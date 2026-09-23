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
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError, HWND},
    System::{
        LibraryLoader::GetModuleFileNameW,
        Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW},
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

/// Register (or update) the launch-at-login value.
///
/// `enabled == true` writes `HKCU\...\Run\FileGo = "<exe path>"`; `false`
/// removes it. Idempotent.
pub fn set_launch_at_login(enabled: bool) -> Result<(), RegistryError> {
    let subkey = pcwstr(run_subkey_wide());
    let value = pcwstr(run_value_wide());
    if enabled {
        let exe = current_exe_path()?;
        let quoted = format!("\"{}\"", exe);
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
        // RegSetKeyValueW with NULL data removes the value.
        // Safety: same arguments, NULL data + 0 length.
        let status =
            unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, subkey, value, REG_SZ.0, None, 0) };
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            // Removing a value that does not exist is benign for our purposes.
            return Err(RegistryError::WriteFailed);
        }
        Ok(())
    }
}

/// Whether launch-at-login is currently set.
pub fn launch_at_login() -> Result<bool, RegistryError> {
    // REG_SZ is small; 4 KiB is generous. A corrupt/oversized value (size
    // overflow) surfaces as ReadFailed rather than overflowing our buffer.
    let mut buffer = [0u16; 4096];
    let mut size = (buffer.len() * 2) as u32;
    // Safety: buffer is valid for size bytes; RRF_RT_REG_SZ restricts the type.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            pcwstr(run_subkey_wide()),
            pcwstr(run_value_wide()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    match status {
        ERROR_SUCCESS => Ok(true),
        ERROR_FILE_NOT_FOUND => Ok(false),
        _ => Err(RegistryError::ReadFailed),
    }
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

/// Borrow a PCWSTR from a NUL-terminated wide vector.
fn pcwstr(wide: Vec<u16>) -> windows::core::PCWSTR {
    debug_assert_eq!(wide.last(), Some(&0), "wide string must be NUL-terminated");
    windows::core::PCWSTR::from_raw(wide.as_ptr())
}

/// The current executable's absolute path (for the Run value).
fn current_exe_path() -> Result<String, RegistryError> {
    let mut buffer = [0u16; 4096];
    // Safety: buffer is valid; GetModuleFileNameW null-terminates.
    let len = unsafe { GetModuleFileNameW(None, &mut buffer) };
    if len == 0 {
        return Err(RegistryError::ExePathUnavailable);
    }
    let len = (len as usize).min(buffer.len() - 1);
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
}
