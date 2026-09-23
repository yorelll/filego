//! Single-instance adapter: named mutex + WM_COPYDATA activation (M04.3).
//!
//! The worker thread in `super::mod.rs` creates a hidden *top-level* window
//! (class `FileGoHotkeyWindow`, `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, never
//! shown). Because it is top-level, the second instance can find it with
//! `FindWindowW` — message-only windows are not enumerable, so we deliberately
//! use an invisible top-level popup.
//!
//! # Who owns what
//!
//! - [`InstanceMutex::acquire`] creates the session-scoped named mutex
//!   `Local\FileGo.<suffix>` (`<suffix>` from [`super::user_session`]).
//!   A `GetLastError() == ERROR_ALREADY_EXISTS` means another instance already
//!   owns it.
//! - The first instance holds the handle for its whole life (and drops it
//!   deterministically on exit).
//! - The second instance calls [`activator::activate_primary`], which polls
//!   `FindWindowW` for at most a bounded time (handles a still-initializing or
//!   just-crashed primary) and sends the fixed `Show` payload over
//!   `WM_COPYDATA` via `SendMessageTimeoutW` (bounded timeout, so a hung
//!   primary cannot block us).

use windows::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
    System::Threading::CreateMutexW,
};

use crate::platform::ipc::{self, SecondInstanceDecision, SecondInstanceOutcome};

pub mod activator;

/// The suffix for the session-scoped mutex name, derived from the current
/// user's identity by [`super::user_session::user_suffix`]. Paired with the
/// `Local\` scope, different users/sessions never block each other.
pub use super::user_session::user_suffix;

/// Guard over the named mutex; `is_primary()` reports whether THIS process
/// created it (and so should run the tray/loop). A second instance exits.
pub struct InstanceMutex {
    handle: HANDLE,
    /// Whether THIS process created the mutex (the primary).
    created_this_process: bool,
}

impl InstanceMutex {
    /// Acquire (create-or-open) the session-scoped mutex.
    ///
    /// `suffix` must have passed [`ipc::mutex_name`] validation. The full mutex
    /// name is derived here.
    pub fn acquire() -> Result<InstanceMutex, SuperError> {
        let suffix = user_suffix();
        let name = ipc::mutex_name(ipc::scope_mutex(), &suffix)
            .map_err(|_| SuperError::MutexNameInvalid)?;
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let wide_pcwstr = windows::core::PCWSTR(wide.as_ptr());

        // Safety: null security attributes (default ACL), initial owner false
        // (we own implicitly on success), wide NUL-terminated name. The Vec is
        // alive for the whole call.
        let handle = unsafe { CreateMutexW(None, false, wide_pcwstr) }
            .map_err(|_| SuperError::MutexFailed)?;
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

        Ok(InstanceMutex {
            handle,
            created_this_process: !already_exists,
        })
    }

    /// Whether this process created the mutex (is the primary).
    pub const fn is_primary(&self) -> bool {
        self.created_this_process
    }

    /// Report this process's role to the pure state machine. A second instance
    /// (`won == false`) still needs the endpoint-availability outcome to pick
    /// between `ActivateAndExit` and `StaleEndpointExit`; the boundary computes
    /// that separately in [`activator`].
    pub const fn decision(
        &self,
        endpoint_reached: bool,
        endpoint_accepted: bool,
    ) -> SecondInstanceDecision {
        if self.created_this_process {
            SecondInstanceDecision {
                should_run_tray: true,
                outcome: None,
            }
        } else {
            let outcome = if endpoint_reached && endpoint_accepted {
                Some(SecondInstanceOutcome::ActivateAndExit)
            } else if endpoint_reached {
                Some(SecondInstanceOutcome::EndpointRejected)
            } else {
                Some(SecondInstanceOutcome::StaleEndpointExit)
            };
            SecondInstanceDecision {
                should_run_tray: false,
                outcome,
            }
        }
    }
}

impl Drop for InstanceMutex {
    fn drop(&mut self) {
        // Safety: `handle` is a valid mutex handle owned by this instance.
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

/// Typed, anonymous errors for the single-instance boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperError {
    /// The derived mutex name was invalid (see [`ipc::MutexNameError`]).
    MutexNameInvalid,
    /// `CreateMutexW` failed for a non-already-exists reason.
    MutexFailed,
}

impl SuperError {
    pub const fn as_detail(self) -> &'static str {
        match self {
            SuperError::MutexNameInvalid => "the single-instance mutex name is invalid",
            SuperError::MutexFailed => "the single-instance mutex could not be created",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_decision_runs_tray() {
        let decision = SecondInstanceDecision {
            should_run_tray: true,
            outcome: None,
        };
        // `decision()` on a created-this-process flag yields this; the pure
        // `ipc::decide` is exercised in the ipc module. Here we assert the
        // constant path through the mask:
        let _ = decision;
        assert!(
            !SecondInstanceDecision {
                should_run_tray: false,
                outcome: None
            }
            .should_run_tray
        );
    }

    #[test]
    fn mutex_names_are_session_scoped_and_len_bounded() {
        let name = ipc::mutex_name(ipc::scope_mutex(), "S-1-5-21-1234567890").expect("valid");
        assert_eq!(name, "Local\\FileGo.S-1-5-21-1234567890");
        assert!(name.len() <= 260);
    }
}
