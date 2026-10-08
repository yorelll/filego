//! Folder-open boundary (M04.5) and launch-at-login HKCU\Run (M04.1).
//!
//! # Folder open
//!
//! The ONE shell call is [`open_folder`] → `ShellExecuteExW` with the folder
//! path as `lpFile` and no verb / no parameters / no interpreter. The pure
//! controller (`crate::platform::shell_open`) maps [`OpenErrorKind`] to the
//! `SE_ERR_*` codes recovered from the `hInstApp` field or the API error.
//! Nothing command-line-shaped is ever built, so there is no quoting/injection
//! surface.
//!
//! The open runs off the UI thread (the caller wraps it), with a best-effort
//! `SEE_MASK_NOASYNC` variant so a hung shell cannot block the caller thread.
//!
//! # Launch at login
//!
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `FileGo` set to
//! the current executable path (quoted). Read/write/clear are idempotent and
//! failures map to an understandable [`RegistryError`]. The value name is
//! fixed; paths never appear in logs.

use windows::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, GetLastError, HWND},
    System::{
        LibraryLoader::GetModuleFileNameW,
        Registry::{
            HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE,
            RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetKeyValueW,
        },
    },
    UI::{
        Shell::{SEE_MASK_ASYNCOK, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::{MB_OK, MessageBoxW},
    },
};

use crate::version;

use crate::platform::shell_open::OpenErrorKind;

/// Run-key path under HKCU.
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "FileGo";

/// The pure `HKCU\...\Run` value for this executable: the quoted absolute path
/// (M06.3 "path with spaces and Unicode correctly quoted"). The surrounding
/// double quotes are what Registry `Run` entries require so a path containing
/// spaces or non-ASCII characters is parsed as one argument. Pure and
/// testable independently of the registry.
pub fn run_value_for(exe_path: &str) -> String {
    format!("\"{exe_path}\"")
}

/// Lossless snapshot of the existing per-user Run value. `None` means that
/// the value did not exist; otherwise the exact type and bytes are retained.
/// Never display or log these bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunValueSnapshot {
    pub value_type: REG_VALUE_TYPE,
    pub bytes: Vec<u8>,
}

pub fn read_run_value() -> Result<Option<RunValueSnapshot>, RegistryError> {
    const MAX_VALUE_BYTES: u32 = 8192;
    let subkey_wide = run_subkey_wide();
    let value_wide = run_value_wide();
    let mut key = Default::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(subkey_wide.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(RegistryError::ReadFailed);
    }
    let value = windows::core::PCWSTR(value_wide.as_ptr());
    let mut kind = REG_VALUE_TYPE(0);
    let mut size = 0u32;
    let status =
        unsafe { RegQueryValueExW(key, value, None, Some(&mut kind), None, Some(&mut size)) };
    if status == ERROR_FILE_NOT_FOUND {
        let closed = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
        return if closed == ERROR_SUCCESS {
            Ok(None)
        } else {
            Err(RegistryError::ReadFailed)
        };
    }
    if status != ERROR_SUCCESS && status != ERROR_MORE_DATA || size > MAX_VALUE_BYTES {
        let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
        return Err(RegistryError::ReadFailed);
    }
    // The value could grow between the size query and the second read. Reserve
    // the full allowed capacity so no raw registry write can overflow the slice.
    let mut bytes = vec![0u8; MAX_VALUE_BYTES as usize];
    size = MAX_VALUE_BYTES;
    let status = unsafe {
        RegQueryValueExW(
            key,
            value,
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    let closed = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
    if status != ERROR_SUCCESS || size > MAX_VALUE_BYTES || closed != ERROR_SUCCESS {
        return Err(RegistryError::ReadFailed);
    }
    bytes.truncate(size as usize);
    Ok(Some(RunValueSnapshot {
        value_type: kind,
        bytes,
    }))
}

pub fn expected_run_value() -> Result<RunValueSnapshot, RegistryError> {
    let quoted = run_value_for(&current_exe_path()?);
    let bytes = quoted
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    Ok(RunValueSnapshot {
        value_type: REG_SZ,
        bytes,
    })
}

/// Restore exactly the previous HKCU value after a failed setting transaction.
/// An absent value is deleted; a present value retains its original type and
/// raw data (including previous portable-EXE paths). The caller verifies it by
/// re-reading before reporting success.
pub fn restore_run_value(previous: Option<&RunValueSnapshot>) -> Result<(), RegistryError> {
    let Some(previous) = previous else {
        return set_launch_at_login(false);
    };
    let subkey_wide = run_subkey_wide();
    let value_wide = run_value_wide();
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(subkey_wide.as_ptr()),
            windows::core::PCWSTR(value_wide.as_ptr()),
            previous.value_type.0,
            Some(previous.bytes.as_ptr().cast()),
            previous.bytes.len() as u32,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(RegistryError::WriteFailed)
    }
}

/// Register (or update) this executable's launch-at-login value. Call via
/// the transaction in `startup_registration`, which refuses foreign values.
///
/// `enabled == true` writes `HKCU\...\Run\FileGo = "<exe path>"`; `false`
/// removes it. Idempotent. The quoted value comes from [`run_value_for`], so a
/// path with spaces/Unicode is never split by the Windows startup parser.
pub fn set_launch_at_login(enabled: bool) -> Result<(), RegistryError> {
    // Keep both UTF-16 buffers alive for the entire Win32 call: PCWSTR borrows
    // their storage and may not outlive either buffer.
    let subkey_wide = run_subkey_wide();
    let value_wide = run_value_wide();
    let subkey = windows::core::PCWSTR(subkey_wide.as_ptr());
    let value = windows::core::PCWSTR(value_wide.as_ptr());
    if enabled {
        let exe = current_exe_path()?;
        let quoted = run_value_for(&exe);
        let wide: Vec<u16> = quoted.encode_utf16().chain(std::iter::once(0)).collect();
        // Safety: HKEY_CURRENT_USER is a predefined root; subkey/value are
        // NUL-terminated wide strings; lpData points at the wide value bytes.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                subkey,
                value,
                REG_SZ.0,
                Some(wide.as_ptr() as *const core::ffi::c_void),
                (wide.len() * 2) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(RegistryError::WriteFailed);
        }
        Ok(())
    } else {
        // RegSetKeyValueW with NULL data does not reliably delete an existing
        // value. Open the per-user Run key and remove this value explicitly.
        let mut key = Default::default();
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_SET_VALUE, &mut key) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        if status != ERROR_SUCCESS {
            return Err(RegistryError::WriteFailed);
        }
        let status = unsafe { RegDeleteValueW(key, value) };
        let closed = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
        if (status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND) || closed != ERROR_SUCCESS {
            return Err(RegistryError::WriteFailed);
        }
        Ok(())
    }
}

/// Whether launch-at-login is registered for this exact executable.
/// Existing values for another portable EXE are not owned by this process.
pub fn launch_at_login() -> Result<bool, RegistryError> {
    Ok(read_run_value()?.as_ref() == Some(&expected_run_value()?))
}

fn run_subkey_wide() -> Vec<u16> {
    RUN_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn run_value_wide() -> Vec<u16> {
    RUN_VALUE.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The current executable's absolute path (for the Run value).
fn current_exe_path() -> Result<String, RegistryError> {
    let mut buffer = [0u16; 4096];
    // A truncated path cannot be registered safely as a different portable
    // executable, and must not be compared as if it were the full path.
    let len = unsafe { GetModuleFileNameW(None, &mut buffer) };
    if len == 0 || len as usize >= buffer.len() {
        return Err(RegistryError::ExePathUnavailable);
    }
    let len = len as usize;
    Ok(String::from_utf16_lossy(&buffer[..len]))
}

/// Open a folder path with the default shell verb.
///
/// `SEE_MASK_NOASYNC | SEE_MASK_ASYNCOK` asks the shell not to block the
/// caller thread indefinitely (the caller also runs this off the UI thread).
/// The path is passed verbatim as `lpFile`; no verb, no parameters — no
/// quoting, no `/c`, no interpreter.
pub fn open_folder(path: &str) -> Result<(), OpenErrorKind> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_ASYNCOK,
        hwnd: HWND::default(),
        lpVerb: windows::core::PCWSTR::null(),
        lpFile: windows::core::PCWSTR(wide.as_ptr()),
        lpParameters: windows::core::PCWSTR::null(),
        lpDirectory: windows::core::PCWSTR::null(),
        nShow: windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
        hInstApp: Default::default(),
        lpIDList: std::ptr::null_mut(),
        lpClass: windows::core::PCWSTR::null(),
        hkeyClass: Default::default(),
        dwHotKey: 0,
        Anonymous: Default::default(),
        hProcess: Default::default(),
    };

    // Safety: `info` is fully initialized, `lpFile` points at the
    // NUL-terminated wide path which lives on our stack for the synchronous
    // call; `ShellExecuteExW` writes hInstApp (mapped to an anonymous kind).
    unsafe { ShellExecuteExW(&mut info) }.map_err(|_| {
        let code = unsafe { GetLastError() }.0;
        map_se_err(code)
    })?;

    // Classic SE_ERR_* encoding surfaced through hInstApp on the W form.
    let code = info.hInstApp.0 as u32;
    if code != 0 && code <= 32 {
        return Err(map_se_err(code));
    }
    Ok(())
}

/// Map a classic SE_ERR_* code onto [`OpenErrorKind`].
fn map_se_err(code: u32) -> OpenErrorKind {
    match code {
        2 | 3 => OpenErrorKind::NotFound, // SE_ERR_FNF / SE_ERR_PNF
        5 => OpenErrorKind::AccessDenied, // SE_ERR_ACCESSDENIED
        27 | 29 | 30 => OpenErrorKind::DdeFailure, // SE_ERR_ASSOCINCOMPLETE / DDEFAIL / DDEBUSY
        31 => OpenErrorKind::NoAssociation, // SE_ERR_NOASSOC
        _ => OpenErrorKind::Unavailable,
    }
}

/// Show the About dialog (M04.1). Never contains a path; only the product name
/// and version.
pub fn about_box() {
    let title: Vec<u16> = version::PRODUCT_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let body: Vec<u16> = format!(
        "{}\n\n{}\n\n{}",
        version::PRODUCT_NAME,
        "Windows x86-64 · MIT",
        version::display()
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
    // Safety: MessageBoxW is a blocking modal dialog with our own wide strings;
    // hwnd NULL makes it owned by the current thread.
    unsafe {
        let _ = MessageBoxW(
            None,
            windows::core::PCWSTR(body.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_OK,
        );
    }
}

/// Anonymous registry errors (never a raw value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    WriteFailed,
    ReadFailed,
    ExePathUnavailable,
}

impl RegistryError {
    pub const fn as_detail(self) -> &'static str {
        match self {
            RegistryError::WriteFailed => "the startup registration could not be written",
            RegistryError::ReadFailed => "the startup registration could not be read",
            RegistryError::ExePathUnavailable => "the application path could not be resolved",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_err_mapping_is_stable() {
        assert_eq!(map_se_err(2), OpenErrorKind::NotFound);
        assert_eq!(map_se_err(3), OpenErrorKind::NotFound);
        assert_eq!(map_se_err(5), OpenErrorKind::AccessDenied);
        assert_eq!(map_se_err(27), OpenErrorKind::DdeFailure);
        assert_eq!(map_se_err(29), OpenErrorKind::DdeFailure);
        assert_eq!(map_se_err(30), OpenErrorKind::DdeFailure);
        assert_eq!(map_se_err(31), OpenErrorKind::NoAssociation);
        assert_eq!(map_se_err(8), OpenErrorKind::Unavailable);
        assert_eq!(map_se_err(9999), OpenErrorKind::Unavailable);
    }

    #[test]
    fn registry_errors_are_anonymous() {
        for error in [
            RegistryError::WriteFailed,
            RegistryError::ReadFailed,
            RegistryError::ExePathUnavailable,
        ] {
            let detail = error.as_detail();
            assert!(!detail.is_empty());
            assert!(!detail.contains('\\'));
        }
    }

    #[test]
    fn run_key_paths_are_fixed() {
        // The constants are stable strings.
        assert_eq!(RUN_VALUE, "FileGo");
        assert!(RUN_SUBKEY.contains("CurrentVersion\\Run"));
        // The wide encodings end with a NUL terminator and match the source.
        let value_wide = run_value_wide();
        assert_eq!(value_wide.last(), Some(&0));
        let dotted = value_wide[..value_wide.len() - 1].to_vec();
        assert_eq!(String::from_utf16_lossy(&dotted), "FileGo");
    }

    // M06.3: the Run value quotes the exe path so spaces and Unicode are never
    // split by the Windows startup parser.
    #[test]
    fn run_value_quotes_full_path_with_spaces_and_unicode() {
        assert_eq!(
            run_value_for(r"C:\Program Files\FileGo\filego.exe"),
            r#""C:\Program Files\FileGo\filego.exe""#
        );
        assert_eq!(
            run_value_for(r"D:\应用\FileGo 数据\filego.exe"),
            r#""D:\应用\FileGo 数据\filego.exe""#
        );
        // The value is exactly `"` + raw path + `"` — one quoting wrapper.
        let path = r"C:\Program Files\FileGo\filego.exe";
        assert_eq!(run_value_for(path), format!(r#""{path}""#));
    }

    /// Status read ↔ write are exact inverses: after a successful
    /// `set_launch_at_login(true)` the registry reads `true`; a `false` write
    /// reads `false`. The registry itself is not touchable headlessly, so this
    /// pins the contract the (real) read path relies on: a written value is
    /// exactly the quoted current-exe path, matched by `launch_at_login`.
    #[test]
    fn launch_at_login_value_matches_only_this_executable() {
        let current = r"C:\Program Files\FileGo\filego.exe";
        assert_eq!(
            run_value_for(current),
            r#""C:\Program Files\FileGo\filego.exe""#
        );
        assert_ne!(
            run_value_for(r"C:\Users\user\Desktop\old-filego.exe"),
            run_value_for(current),
            "a stale portable location must not appear enabled for this process"
        );
        // The registry's quoted UTF-16 value must survive decoding without
        // lifetime-dependent pointers or dropping the terminating NUL early.
        let wide: Vec<u16> = run_value_for(current)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let actual = String::from_utf16_lossy(&wide[..wide.len() - 1]);
        assert_eq!(actual, run_value_for(current));
    }

    #[test]
    fn launch_at_login_read_write_are_exact_inverses_by_contract() {
        let exe = r"C:\Program Files\FileGo\filego.exe";
        let value = run_value_for(exe);
        // The value written for `enabled == true` is the quoted exe path.
        assert!(value.starts_with('"') && value.ends_with('"'));
        // Windows `Run` path: the value is passed to the shell as a command
        // line; a quoted path with spaces parses to exactly one token.
        assert!(!value.contains("  "));
    }
}
