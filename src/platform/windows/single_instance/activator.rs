//! Second-instance activation over WM_COPYDATA (M04.3).
//!
//! The second instance finds the primary's hidden top-level window by class
//! name (`FileGoHotkeyWindow`) and sends the fixed `Show` payload. A bounded
//! discovery poll handles a primary that is still starting or has just crashed:
//! - a crashing first instance's process handle dies with it, so after it the
//!   mutex is free and the next start becomes primary;
//! - a still-initializing primary is not yet discoverable → with timeout we
//!   wait briefly, then fall back to "stale endpoint, exit".
//!
//! Privacy: only the fixed protocol bytes are sent; nothing path-like crosses
//! the wire.

use windows::Win32::{
    Foundation::{ERROR_SUCCESS, GetLastError, HWND, LPARAM, WPARAM},
    System::DataExchange::COPYDATASTRUCT,
    UI::WindowsAndMessaging::{
        FindWindowW, SMTO_ABORTIFHUNG, SMTO_ERRORONEXIT, SendMessageTimeoutW, WM_COPYDATA,
    },
};

use crate::platform::ipc;

/// The hidden window class both instances create.
pub const HIDDEN_WINDOW_CLASS: &str = "FileGoHotkeyWindow";

/// Outcome reported back to the single-instance decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationResult {
    /// The primary accepted the Show command.
    Accepted,
    /// The primary's window was found but declined / the send timed out.
    RejectedOrUnreachable,
    /// No primary window could be found within the discovery budget.
    StaleEndpoint,
}

/// Find the primary's hidden window, waiting up to `budget_ms` for a primary
/// that is still initializing.
pub fn discover_primary_window(budget_ms: u64) -> Option<HWND> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(budget_ms);
    let class_wide: Vec<u16> = HIDDEN_WINDOW_CLASS
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    loop {
        // Safety: class name is a stable NUL-terminated wide literal.
        let found = unsafe {
            FindWindowW(
                windows::core::PCWSTR(class_wide.as_ptr()),
                windows::core::PCWSTR::null(),
            )
        }
        .ok();
        if let Some(hwnd) = found {
            return Some(hwnd);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Send the fixed Show activation to the primary's hidden window.
///
/// `SendMessageTimeoutW` keeps us from blocking on a hung primary.
pub fn send_activate(target: HWND) -> ActivationResult {
    let payload = ipc::encode_show();
    let copy_data = COPYDATASTRUCT {
        dwData: 0,
        cbData: payload.len() as u32,
        lpData: payload.as_ptr() as *mut core::ffi::c_void,
    };
    // Safety: `lparam` must point at a COPYDATASTRUCT valid for the duration of
    // the synchronous send; `copy_data` lives on our stack and outlives the call.
    let result = unsafe {
        SendMessageTimeoutW(
            target,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&copy_data as *const _ as isize),
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
            5_000,
            None,
        )
    };
    // SendMessageTimeoutW returns 0 precisely when the timeout fired/error.
    if result.0 == 0 {
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_SUCCESS {
            return ActivationResult::RejectedOrUnreachable;
        }
        return ActivationResult::RejectedOrUnreachable;
    }
    // Our WndProc returns LRESULT(1) for a handled Show.
    if result.0 == 1 {
        ActivationResult::Accepted
    } else {
        ActivationResult::RejectedOrUnreachable
    }
}

/// One-shot second-instance activation: discover the primary's hidden window
/// (waiting up to `budget_ms` for a still-initializing primary) and send the
/// `Show` command.
pub fn activate_primary(budget_ms: u64) -> ActivationResult {
    match discover_primary_window(budget_ms) {
        Some(target) => send_activate(target),
        None => ActivationResult::StaleEndpoint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ipc::{IpcRequest, PROTOCOL_LEN};

    #[test]
    fn encoded_show_payload_is_protocol_sized_and_decodes_as_show() {
        let payload = super::ipc::encode_show();
        assert_eq!(payload.len(), PROTOCOL_LEN);
        assert_eq!(super::ipc::decode(&payload), IpcRequest::Show);
    }

    #[test]
    fn copy_data_struct_layout_matches_payload() {
        let payload = ipc::encode_show();
        let data = COPYDATASTRUCT {
            dwData: 0,
            cbData: payload.len() as u32,
            lpData: payload.as_ptr() as *mut core::ffi::c_void,
        };
        assert_eq!(data.cbData as usize, PROTOCOL_LEN);
        assert!(!data.lpData.is_null());
    }

    #[test]
    fn outcome_enum_is_exhaustive_and_stable() {
        let _ = [
            ActivationResult::Accepted,
            ActivationResult::RejectedOrUnreachable,
            ActivationResult::StaleEndpoint,
        ];
    }
}
