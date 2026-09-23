//! Window activation / foreground fallback (M04.4).
//!
//! Slint's `Window::show()` maps the window but does not guarantee foreground
//! focus under Windows foreground-lock rules. This module wraps the native
//! HWND and applies the documented fallback chain:
//!
//! 1. `ShowWindow(SW_SHOW)` + `SetWindowPos(HWND_TOP, SWP_NOACTIVATE|SWP_SHOWWINDOW)`
//!    — safe placement that never steals focus;
//! 2. `SetForegroundWindow` — the standard bring-to-front call; it only
//!    succeeds when our process is allowed to take foreground;
//! 3. If we are NOT foreground: temporarily `AttachThreadInput` to the
//!    foreground thread, `BringWindowToTop` + `SetActiveWindow`, then detach —
//!    the classic launcher-tool sequence that a global-hotkey invocation
//!    legitimately expects. Bounded: if attach fails we just proceed with
//!    `BringWindowToTop`.
//!
//! The result is anonymous ([`FocusResult`]); raw codes never escape.

use windows::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::{GetActiveWindow, SetActiveWindow},
        WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, HWND_TOP,
            IsWindowVisible, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
            SetForegroundWindow, SetWindowPos, ShowWindow,
        },
    },
};

/// Bring a native HWND (passed as raw pointer bits from the UI layer) to the
/// front. `main.rs` cannot reference `windows::Win32` types directly without
/// depending on the FFI crate in the bin, so the boundary converts for it.
pub fn bring_to_front_bits(hwnd_bits: isize, already_visible: bool) -> FocusResult {
    use windows::Win32::Foundation::HWND;
    let hwnd = HWND(hwnd_bits as *mut core::ffi::c_void);
    bring_to_front(hwnd, already_visible)
}

/// Outcome of an activation attempt (anonymous).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusResult {
    /// The window is visible and has foreground focus.
    Foreground,
    /// The window is visible, but Windows refused foreground focus; the
    /// fallback chain already ran. The user may still switch to it (or click).
    VisibleNotForeground,
}

fn is_foreground(hwnd: HWND) -> bool {
    // Safety: stateless HWND comparison.
    unsafe { GetForegroundWindow() == hwnd }
}

/// Whether a window is currently mapped.
pub fn window_is_visible(hwnd: HWND) -> bool {
    // Safety: stateless query.
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// Bring `hwnd` to the front and attempt to focus it.
///
/// `already_visible` skips the redundant `ShowWindow` when the caller just
/// showed the window through Slint.
pub fn bring_to_front(hwnd: HWND, already_visible: bool) -> FocusResult {
    if !already_visible {
        // Safety: `hwnd` is the Slint window's native handle (valid, owned by
        // the winit event loop).
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        // Safety: same; SWP_NOACTIVATE means we never steal focus while placing.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE | SWP_SHOWWINDOW,
            )
        };
    }

    // Standard bring-to-front.
    // Safety: as above.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }

    if is_foreground(hwnd) {
        return FocusResult::Foreground;
    }

    // Fallback: temporary thread-input attach (launcher-tool pattern).
    set_active_and_top(hwnd);

    if is_foreground(hwnd) {
        FocusResult::Foreground
    } else {
        FocusResult::VisibleNotForeground
    }
}

fn foreground_thread_id() -> Option<u32> {
    // Safety: GetForegroundWindow may return NULL (we then yield None);
    // GetWindowThreadProcessId is a stateless query.
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return None;
    }
    let thread_id = unsafe { GetWindowThreadProcessId(fg, None) };
    if thread_id == 0 {
        None
    } else {
        Some(thread_id)
    }
}

fn set_active_and_top(hwnd: HWND) {
    // Safety: GetCurrentThreadId never fails.
    let our_thread = unsafe { GetCurrentThreadId() };
    let fg_thread = foreground_thread_id();

    match fg_thread {
        Some(fg) if fg != our_thread => {
            // Safety: AttachThreadInput is attach-then-detach in the same scope;
            // if either call fails we proceed best-effort (the window stays
            // visible even if it does not reach foreground).
            let attached = unsafe { AttachThreadInput(our_thread, fg, true) };
            if attached.as_bool() {
                let _ = unsafe { BringWindowToTop(hwnd) };
                let _ = unsafe { SetActiveWindow(hwnd) };
                unsafe {
                    let _ = AttachThreadInput(our_thread, fg, false);
                }
            } else {
                let _ = unsafe { BringWindowToTop(hwnd) };
            }
        }
        _ => {
            let _ = unsafe { BringWindowToTop(hwnd) };
            let _ = unsafe { SetActiveWindow(hwnd) };
        }
    }
    // Final attempt after the attach trick gave us foreground permission.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Active-window probe (used by an aggressive retry upstream, not by default).
pub fn active_window() -> Option<HWND> {
    // Safety: stateless query.
    let hwnd = unsafe { GetActiveWindow() };
    if hwnd.is_invalid() { None } else { Some(hwnd) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_results_are_anonymous_and_compareable() {
        assert_eq!(FocusResult::Foreground, FocusResult::Foreground);
        assert_eq!(
            FocusResult::VisibleNotForeground,
            FocusResult::VisibleNotForeground
        );
        assert_ne!(FocusResult::Foreground, FocusResult::VisibleNotForeground);
    }

    #[test]
    fn active_window_probe_never_panics() {
        // On a headless/CI run there may be no active window; the probe must be
        // None or a handle, never panic.
        let _ = active_window();
    }
}
