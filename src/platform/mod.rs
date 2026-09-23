//! Platform boundary (M04).
//!
//! Pure, testable protocol and policy modules live here (hotkey validation /
//! state machine, single-instance IPC protocol, window-position geometry,
//! shell-open error mapping). The `windows` submodule is the only place that
//! touches the `windows`-crate FFI; everything else is unit-tested without a
//! window station and runs on both GNU and MSVC CI.

#[cfg(target_os = "windows")]
pub mod windows;

pub mod hotkey;
pub mod ipc;
pub mod shell_open;
pub mod window_position;

/// The session scope prefix for the single-instance mutex (`Local\`).
pub const SESSION_SCOPE: &str = "Local";

/// Commands the platform layer can deliver to the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformCommand {
    /// Show the search window (hotkey press, second-instance activation).
    ShowWindow,
    /// Hide the search window.
    HideWindow,
    /// Toggle the search window.
    ToggleWindow,
    /// Exit the application.
    Exit,
}

pub trait PlatformEventSource {
    type Error;

    fn initialize(&mut self) -> Result<(), Self::Error>;
}
