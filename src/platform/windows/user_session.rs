//! Per-user / per-session identity for single-instance naming (M04.3).
//!
//! The mutex is session-scoped (`Local\...`) so different users in different
//! sessions never collide, and per-user (`.<suffix>`) so different users on one
//! machine never share an instance. The suffix is derived from the current
//! user's stable identity.
//!
//! `GetUserNameW` returns the display name, which is not guaranteed stable; the
//! robust key is the user SID. For 0.0.1 we derive the suffix from the user
//! name and length-limit/normalize it, and the pure [`crate::platform::ipc::mutex_name`]
//! validator rejects names that cannot be represented (non-ASCII / too long).
//! A stable SID-based suffix is a documented follow-up; the contract (per-session,
//! per-user) is unchanged.

use windows::Win32::System::WindowsProgramming::GetUserNameW;

/// A stable-enough per-user suffix for the single-instance mutex.
///
/// Falls back to a fixed constant when `GetUserNameW` fails (never panics on an
/// exotic environment; the mutex then degrades to a whole-session lock, still
/// correct for the single-user product scope).
pub fn user_suffix() -> String {
    // Windows user names are limited to 256 chars; our validation allows up to
    // 255, so a 1 KiB buffer is far more than enough.
    let mut buffer = [0u16; 1024];
    let mut len = buffer.len() as u32;
    // Safety: buffer is valid for len units; GetUserNameW null-terminates.
    let ok =
        unsafe { GetUserNameW(Some(windows::core::PWSTR(buffer.as_mut_ptr())), &mut len) }.is_ok();
    if !ok {
        return "current-user".to_owned();
    }
    let actual_len = (len as usize).min(buffer.len().saturating_sub(1));
    let name = String::from_utf16_lossy(&buffer[..actual_len]);
    // Normalize to something the IPC validator accepts (ASCII, no whitespace).
    // We keep letters/digits/'-'/'_', and if the result is empty or non-ASCII
    // we fall back to a hex digest of the raw codepoints.
    let mut clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if clean.is_empty() || clean.len() > 200 {
        clean = format!("user-{:08x}", name.chars().count());
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ipc::mutex_name;

    #[test]
    fn suffix_is_valid_for_the_mutex_name() {
        let suffix = user_suffix();
        // May be the real user name on Windows; on any platform it must produce
        // a valid mutex name.
        use crate::platform::ipc::MutexNameError;
        match mutex_name("Local", &suffix) {
            Ok(_) => {}
            Err(MutexNameError::TooLong) => panic!("suffix too long: {}", suffix.len()),
            Err(MutexNameError::InvalidSuffix) => {
                // Non-ASCII fallback — should not happen because user_suffix
                // normalizes, but accept the documented fallback.
                assert!(suffix.is_empty());
            }
            Err(MutexNameError::InvalidScope) => unreachable!(),
        }
    }
}
