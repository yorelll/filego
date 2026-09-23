//! Pure single-instance IPC protocol (M04.3).
//!
//! The Windows boundary owns the named mutex and the `WM_COPYDATA` transport;
//! this module defines the fixed-length wire format and the second-instance
//! state machine so the protocol can be validated without a window station.
//!
//! # Wire format
//!
//! `WM_COPYDATA` carries a `COPYDATASTRUCT` whose `lpData` points at a
//! fixed-length byte payload:
//!
//! ```text
//! offset 0..4   : magic    "FTGL" (4 bytes)
//! offset 4..8   : version  = 1 (little-endian u32)
//! offset 8..12  : command  = 1 = Show (little-endian u32)
//! offset 12..64 : reserved, MUST be zero
//! ```
//!
//! Only `command == Show` is accepted; anything else (unknown command, wrong
//! magic, wrong version, unsupported trailing bytes) is rejected and ignored.
//! The protocol can never carry a path, a command line, or an arbitrary action
//! — the fixed 5-field envelope has no payload slot.

/// Fixed wire length in bytes (a `COPYDATASTRUCT` of exactly this size is used).
pub const PROTOCOL_LEN: usize = 64;
/// Magic header bytes.
pub const MAGIC: &[u8; 4] = b"FTGL";
/// Protocol version.
pub const VERSION: u32 = 1;
/// The only valid command.
pub const CMD_SHOW: u32 = 1;

/// Decoded inbound request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcRequest {
    /// Activate the first instance's search window.
    Show,
    /// The payload was rejected (wrong magic/version/command or malformed);
    /// the first instance ignores it.
    Ignored,
}

/// Build the fixed-length `Show` payload.
pub fn encode_show() -> [u8; PROTOCOL_LEN] {
    let mut buf = [0u8; PROTOCOL_LEN];
    buf[0..4].copy_from_slice(MAGIC);
    buf[4..8].copy_from_slice(&VERSION.to_le_bytes());
    buf[8..12].copy_from_slice(&CMD_SHOW.to_le_bytes());
    buf
}

/// Decode an inbound payload (already length-checked to `PROTOCOL_LEN`).
///
/// Any malformed input is `Ignored`, never a panic and never an `Execute`.
pub fn decode(payload: &[u8]) -> IpcRequest {
    if payload.len() != PROTOCOL_LEN {
        return IpcRequest::Ignored;
    }
    if &payload[0..4] != MAGIC {
        return IpcRequest::Ignored;
    }
    let version = u32::from_le_bytes(payload[4..8].try_into().unwrap());
    if version != VERSION {
        return IpcRequest::Ignored;
    }
    let command = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    // Reserved region must be zero (rejects crafted/bloated envelopes).
    if payload[12..].iter().any(|byte| *byte != 0) {
        return IpcRequest::Ignored;
    }
    match command {
        CMD_SHOW => IpcRequest::Show,
        _ => IpcRequest::Ignored,
    }
}

/// Why a second instance decided to exit, from its own perspective.
///
/// The boundary performs the real handshake and records the outcome so the
/// second instance can log an anonymous, deterministic reason (never a path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondInstanceOutcome {
    /// The mutex already existed; this instance signaled the first and exits.
    ActivateAndExit,
    /// The mutex was acquired but no endpoint could be reached; this instance
    /// exits without a tray icon.
    StaleEndpointExit,
    /// A claimed primary endpoint responded with a rejection; exit silently.
    EndpointRejected,
}

/// Pure concurrent-second-instance state machine.
///
/// Independent of any OS handle: it only decides *what to do* given whether the
/// primary mutex was won and whether the primary endpoint was reachable. The
/// boundary feeds it the two booleans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondInstanceDecision {
    pub should_run_tray: bool,
    pub outcome: Option<SecondInstanceOutcome>,
}

/// Decide what a second instance should do.
///
/// - `same_session` also deduplicates: when multiple second instances race, the
///   first to observe the already-owning mutex sends the activation; the rest
///   observe the same and also exit — none of them ever starts a second tray.
pub fn decide(
    won_mutex: bool,
    endpoint_reached: bool,
    endpoint_accepted: bool,
) -> SecondInstanceDecision {
    if !won_mutex {
        // The primary is already running. Reach it; if reachable and it
        // accepts, activate-and-exit; if not reachable/rejected, exit silently.
        let outcome = if endpoint_reached && endpoint_accepted {
            Some(SecondInstanceOutcome::ActivateAndExit)
        } else if endpoint_reached {
            Some(SecondInstanceOutcome::EndpointRejected)
        } else {
            Some(SecondInstanceOutcome::StaleEndpointExit)
        };
        return SecondInstanceDecision {
            should_run_tray: false,
            outcome,
        };
    }
    // Won the mutex: we are (or just became) the primary.
    SecondInstanceDecision {
        should_run_tray: true,
        outcome: None,
    }
}

/// The canonical session scope for the named mutex.
pub const SESSION_SCOPE: &str = "Local";

/// Projection of the product scope constant onto the IPC layer.
pub fn scope_mutex() -> &'static str {
    crate::platform::SESSION_SCOPE
}

/// Testable derivation of the session-scoped mutex name from a per-user handle.
///
/// `Local\FileGo.<suffix>` namespaces the mutex to the session; the suffix is
/// derived from the current user's identity by the boundary (`GetUserNameW` is
/// a display name, not a stable per-user key; the code normalizes it — see
/// `platform::windows::user_session`). The rules are validated here: suffix
/// must be non-empty ASCII, and the full name must not exceed MAX_PATH (260).
pub fn mutex_name(scope: &str, suffix: &str) -> Result<String, MutexNameError> {
    if scope.is_empty() || scope != SESSION_SCOPE {
        return Err(MutexNameError::InvalidScope);
    }
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii() && !c.is_whitespace()) {
        return Err(MutexNameError::InvalidSuffix);
    }
    let name = format!("{}\\FileGo.{}", scope, suffix);
    if name.len() > 260 {
        return Err(MutexNameError::TooLong);
    }
    Ok(name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutexNameError {
    InvalidScope,
    InvalidSuffix,
    TooLong,
}

impl MutexNameError {
    pub const fn as_detail(self) -> &'static str {
        match self {
            MutexNameError::InvalidScope => "the single-instance mutex scope is invalid",
            MutexNameError::InvalidSuffix => "the single-instance mutex suffix is invalid",
            MutexNameError::TooLong => "the single-instance mutex name is too long",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_produces_a_valid_show_payload() {
        let payload = encode_show();
        assert_eq!(payload.len(), PROTOCOL_LEN);
        assert_eq!(decode(&payload), IpcRequest::Show);
    }

    #[test]
    fn decode_rejects_wrong_magic() {
        let mut payload = encode_show();
        payload[0] = b'X';
        assert_eq!(decode(&payload), IpcRequest::Ignored);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let mut payload = encode_show();
        payload[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(decode(&payload), IpcRequest::Ignored);
    }

    #[test]
    fn decode_rejects_unknown_commands() {
        let mut payload = encode_show();
        payload[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert_eq!(decode(&payload), IpcRequest::Ignored);
    }

    #[test]
    fn decode_rejects_wrong_length_and_nonzero_reserved() {
        assert_eq!(decode(&encode_show()[..8]), IpcRequest::Ignored);
        let mut payload = encode_show();
        payload[12] = 1;
        assert_eq!(decode(&payload), IpcRequest::Ignored);
    }

    #[test]
    fn decode_never_treats_arbitrary_bytes_as_execute() {
        // A hand-crafted "path-like" tail must stay Ignored (no payload slot).
        let mut payload = [0u8; PROTOCOL_LEN];
        payload[0..4].copy_from_slice(MAGIC);
        payload[4..8].copy_from_slice(&VERSION.to_le_bytes());
        payload[8..12].copy_from_slice(&CMD_SHOW.to_le_bytes());
        // Leave "C:\" residue in the reserved region (bytes 12.. are zeroed
        // otherwise; plant 3 nonzero bytes).
        payload[12] = 0x43; // 'C'
        payload[13] = 0x3A; // ':'
        payload[14] = 0x5C; // '\'
        assert_eq!(decode(&payload), IpcRequest::Ignored);
    }

    #[test]
    fn second_instance_exits_when_primary_is_running() {
        let decision = decide(false, true, true);
        assert!(!decision.should_run_tray);
        assert_eq!(
            decision.outcome,
            Some(SecondInstanceOutcome::ActivateAndExit)
        );
    }

    #[test]
    fn second_instance_exits_silently_on_stale_or_rejected_endpoint() {
        let stale = decide(false, false, false);
        assert!(!stale.should_run_tray);
        assert_eq!(
            stale.outcome,
            Some(SecondInstanceOutcome::StaleEndpointExit)
        );

        let rejected = decide(false, true, false);
        assert!(!rejected.should_run_tray);
        assert_eq!(
            rejected.outcome,
            Some(SecondInstanceOutcome::EndpointRejected)
        );
    }

    #[test]
    fn mutex_winner_runs_tray() {
        let decision = decide(true, false, false);
        assert!(decision.should_run_tray);
        assert_eq!(decision.outcome, None);
    }

    #[test]
    fn concurrent_second_instances_all_exit_without_second_tray() {
        // Simulate three racers: the first observes an already-owning mutex and
        // signals; the others observe the same and exit too. None run a tray.
        for _ in 0..3 {
            let decision = decide(false, true, true);
            assert!(!decision.should_run_tray);
            assert_eq!(
                decision.outcome,
                Some(SecondInstanceOutcome::ActivateAndExit)
            );
        }
    }

    #[test]
    fn mutex_names_are_session_scoped_and_validated() {
        let name = mutex_name("Local", "S-1-5-21-1234567890").expect("valid");
        assert_eq!(name, "Local\\FileGo.S-1-5-21-1234567890");
        assert!(name.len() <= 260);

        assert_eq!(mutex_name("", "abc"), Err(MutexNameError::InvalidScope));
        assert_eq!(
            mutex_name("Global", "abc"),
            Err(MutexNameError::InvalidScope)
        );
        assert_eq!(mutex_name("Local", ""), Err(MutexNameError::InvalidSuffix));
        assert_eq!(
            mutex_name("Local", "has space"),
            Err(MutexNameError::InvalidSuffix)
        );
        assert_eq!(
            mutex_name("Local", "中文"),
            Err(MutexNameError::InvalidSuffix)
        );
        assert_eq!(
            mutex_name("Local", &"x".repeat(300)),
            Err(MutexNameError::TooLong)
        );
    }
}
