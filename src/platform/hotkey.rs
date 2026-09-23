//! Pure global-hotkey validation and state machine (M04.2).
//!
//! No Win32 imports live here. The `platform::windows` boundary maps
//! [`crate::domain::settings::HotkeySetting`] onto `RegisterHotKey` /
//! `UnregisterHotKey` through the injected [`HotkeyRegistry`] trait, so every
//! rule and every transition below is unit-testable without a window station.
//!
//! # Rules (table-driven, matches the M04.2 acceptance list)
//!
//! 1. The key must be in range: `Vk` in `0x20..=0x5A` (Space..Z) or
//!    `Function` with index `1..=24` (F1..F24).
//! 2. A `Vk` key needs at least one non-Win modifier (Control / Alt / Shift);
//!    a bare letter/digit/Space alone is rejected.
//! 3. "Single Win" (Win as the only modifier) is rejected for any key.
//! 4. Reserved system combinations are rejected outright (table).
//!
//! # State machine
//!
//! [`HotkeyState`] is `Disabled` (never registered / cleared / startup
//! failure), `Active(combo)` (registered, presses dispatch `Show`), or
//! `Paused(combo)` (still registered, presses ignored). Pause keeps the
//! registration so resume needs no re-registration race with other apps.
//!
//! OS lifetime note: `RegisterHotKey` is per-session and auto-cleared when the
//! process exits (crash or not), so "no OS residual after exit" is satisfied by
//! the OS itself; `unregister` on clean exit is still a correctness nicety.
//! We test the machine, never the OS.

use crate::domain::settings::{HotkeyKey, HotkeyModifiers, HotkeySetting};

/// The Windows virtual-key range accepted for `Vk` keys (Space ..= Z).
pub const MIN_VK: u8 = 0x20;
pub const MAX_VK: u8 = 0x5A;
/// F1..=F24.
pub const MIN_FUNCTION: u8 = 1;
pub const MAX_FUNCTION: u8 = 24;

/// Why a combination is invalid, so the UI can show an understandable,
/// localized reason without any raw Win32 detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyValidationError {
    /// Key outside the accepted ranges (only Space..Z or F1..F24 allowed).
    KeyOutOfRange,
    /// A letter/digit/Space key without any non-Win modifier.
    NeedsModifier,
    /// Win is the only modifier.
    WinOnly,
    /// The combination is reserved by the system.
    Reserved,
}

impl HotkeyValidationError {
    /// Privacy-safe, stable diagnostic detail (never carries the combo itself).
    pub const fn as_detail(self) -> &'static str {
        match self {
            HotkeyValidationError::KeyOutOfRange => "the hotkey key is outside the supported range",
            HotkeyValidationError::NeedsModifier => {
                "a non-Win modifier is required for a letter/digit/Space hotkey"
            }
            HotkeyValidationError::WinOnly => {
                "the Windows key alone is not a valid hotkey modifier"
            }
            HotkeyValidationError::Reserved => "the hotkey combination is reserved by the system",
        }
    }
}

/// Modifier bit masks; kept private and mirrored by the boundary.
pub(crate) const MOD_CTRL: u8 = 1;
pub(crate) const MOD_ALT: u8 = 2;
pub(crate) const MOD_SHIFT: u8 = 4;
pub(crate) const MOD_WIN: u8 = 8;

/// Map a hotkey's key onto a Windows virtual-key code (1-based F-index → 0x70+
/// computed in the domain helper; here we reuse it).
fn key_vk(key: HotkeyKey) -> u8 {
    key.vk_code()
}

/// Reserved combinations (currently-registered modifier mask + VK code),
/// table-driven: match on `(mask, vk)` both equal.
const RESERVED: &[(u8, u8)] = &[
    // Ctrl+Alt+Del cannot be claimed (hardware path).
    (MOD_CTRL | MOD_ALT, 0x2E /* VK_DELETE */),
    // Alt+Tab switches applications.
    (MOD_ALT, 0x09 /* VK_TAB */),
    // Alt+Esc cycles windows.
    (MOD_ALT, 0x1B /* VK_ESCAPE */),
    // Ctrl+Esc opens the Start menu.
    (MOD_CTRL, 0x1B /* VK_ESCAPE */),
];

/// Modifier mask used by the validation and matching rules (order-independent).
pub fn modifier_mask(modifiers: HotkeyModifiers) -> u8 {
    let mut mask = 0;
    if modifiers.control {
        mask |= MOD_CTRL;
    }
    if modifiers.alt {
        mask |= MOD_ALT;
    }
    if modifiers.shift {
        mask |= MOD_SHIFT;
    }
    if modifiers.win {
        mask |= MOD_WIN;
    }
    mask
}

/// Exactly one non-Win modifier present.
pub fn has_non_win_modifier(modifiers: HotkeyModifiers) -> bool {
    modifiers.control || modifiers.alt || modifiers.shift
}

/// Validate a combination against the rules.
pub fn validate(modifiers: HotkeyModifiers, key: HotkeyKey) -> Result<(), HotkeyValidationError> {
    let is_vk_in_range = match key {
        HotkeyKey::Vk { vk } => (MIN_VK..=MAX_VK).contains(&vk),
        HotkeyKey::Function { .. } => false,
    };
    let is_function_in_range = match key {
        HotkeyKey::Function { index } => (MIN_FUNCTION..=MAX_FUNCTION).contains(&index),
        HotkeyKey::Vk { .. } => false,
    };

    // Reserved system combinations trump everything else (including the range
    // check: a caller who constructs VK_TAB must get "reserved", not a vague
    // "out of range").
    let mask = modifier_mask(modifiers);
    let vk = key_vk(key);
    if RESERVED
        .iter()
        .any(|(reserved_mask, reserved_vk)| *reserved_mask == mask && *reserved_vk == vk)
    {
        return Err(HotkeyValidationError::Reserved);
    }

    if !is_vk_in_range && !is_function_in_range {
        return Err(HotkeyValidationError::KeyOutOfRange);
    }

    // Win as the only modifier is rejected (more specific than NeedsModifier:
    // "single Win" is exactly what the user pressed).
    if modifiers.win && !has_non_win_modifier(modifiers) {
        return Err(HotkeyValidationError::WinOnly);
    }

    // A non-function key needs a non-Win modifier.
    if !is_function_in_range && !has_non_win_modifier(modifiers) {
        return Err(HotkeyValidationError::NeedsModifier);
    }

    Ok(())
}

/// A hotkey that is currently being tracked by the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyState {
    /// Nothing registered (cleared, paused-from-start, or a failed startup).
    Disabled,
    /// Registered and active; a matching press yields `Show`.
    Active(HotkeySetting),
    /// Still registered, but presses are ignored.
    Paused(HotkeySetting),
}

/// Why a platform registration failed (the boundary maps Win32 errors here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyErrorKind {
    /// Another window owns the combination (`ERROR_HOTKEY_ALREADY_REGISTERED`).
    Conflict,
    /// Any other API failure.
    Unavailable,
}

impl HotkeyErrorKind {
    pub const fn as_detail(self) -> &'static str {
        match self {
            HotkeyErrorKind::Conflict => "the hotkey is already in use by another application",
            HotkeyErrorKind::Unavailable => "the hotkey could not be registered",
        }
    }
}

/// Registry seam the state machine calls. The boundary implements this over
/// the real Windows API; tests inject a fake to force failures.
pub trait HotkeyRegistry {
    /// Register `combo`. Returns `Err` when the platform refused.
    fn register(&mut self, combo: HotkeySetting) -> Result<(), HotkeyErrorKind>;
    /// Unregister the currently-registered hotkey if any.
    fn unregister(&mut self) -> Result<(), HotkeyErrorKind>;
}

/// A resolved notification delivered to the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    /// The search window should be shown (toggled into visibility).
    Show,
}

/// Pure hotkey lifecycle machine. Owns the desired state and drives the
/// injected registry; every mutation that touches the OS is explicit.
pub struct HotkeyMachine<R: HotkeyRegistry> {
    registry: R,
    state: HotkeyState,
    /// The last registration failure (if any) for a "report conflict"
    /// affordance.
    last_error: Option<HotkeyErrorKind>,
}

impl<R: HotkeyRegistry> HotkeyMachine<R> {
    /// Construct from a persisted `setting`. An invalid or unconvertible combo
    /// yields `Disabled` (never registers garbage); a startup registration
    /// failure also yields `Disabled` and records the reason in
    /// [`Self::last_error`].
    pub fn new(registry: R, setting: Option<HotkeySetting>) -> Self {
        let mut machine = HotkeyMachine {
            registry,
            state: HotkeyState::Disabled,
            last_error: None,
        };
        let Some(combo) = setting else {
            return machine;
        };
        if validate(combo.modifiers, combo.key).is_err() {
            return machine;
        }
        match machine.registry.register(combo) {
            Ok(()) => machine.state = HotkeyState::Active(combo),
            Err(kind) => machine.last_error = Some(kind),
        }
        machine
    }

    /// The current hotkey state.
    pub const fn state(&self) -> HotkeyState {
        self.state
    }

    /// The last registration failure (if any) for a conflict affordance.
    pub const fn last_error(&self) -> Option<HotkeyErrorKind> {
        self.last_error
    }

    /// Replace the combination. Validates first; on an invalid combo nothing
    /// changes. On a platform conflict the old registration is re-established
    /// (keep-old semantics) and `Err(Conflict)` is returned.
    pub fn set(&mut self, combo: HotkeySetting) -> Result<(), HotkeyErrorKind> {
        if validate(combo.modifiers, combo.key).is_err() {
            return Err(HotkeyErrorKind::Unavailable);
        }
        if self.state == HotkeyState::Active(combo) {
            return Ok(());
        }

        // Forget any previous registration (also from a paused state).
        if matches!(self.state, HotkeyState::Active(_) | HotkeyState::Paused(_))
            && let Err(kind) = self.registry.unregister()
        {
            self.last_error = Some(kind);
            return Err(kind);
        }

        match self.registry.register(combo) {
            Ok(()) => {
                self.state = HotkeyState::Active(combo);
                self.last_error = None;
                Ok(())
            }
            Err(kind) => {
                // Keep-old: re-establish the previous combo (restored as Active)
                // so the user keeps a working shortcut.
                let previous = match self.state {
                    HotkeyState::Active(prev) | HotkeyState::Paused(prev) => Some(prev),
                    HotkeyState::Disabled => None,
                };
                if let Some(prev) = previous {
                    let _ = self.registry.register(prev);
                    self.state = HotkeyState::Active(prev);
                } else {
                    self.state = HotkeyState::Disabled;
                }
                self.last_error = Some(kind);
                Err(kind)
            }
        }
    }

    /// Disable the hotkey entirely (unregister).
    pub fn clear(&mut self) -> Result<(), HotkeyErrorKind> {
        if matches!(self.state, HotkeyState::Active(_) | HotkeyState::Paused(_)) {
            self.registry.unregister()?;
        }
        self.state = HotkeyState::Disabled;
        self.last_error = None;
        Ok(())
    }

    /// Temporarily ignore presses; the registration is kept so `resume` is
    /// instantaneous and does not race other apps for the combination.
    pub fn pause(&mut self) {
        self.state = match self.state {
            HotkeyState::Active(combo) => HotkeyState::Paused(combo),
            other => other,
        };
    }

    /// Resume after a pause.
    pub fn resume(&mut self) {
        self.state = match self.state {
            HotkeyState::Paused(combo) => HotkeyState::Active(combo),
            other => other,
        };
    }

    /// Deliver a raw hotkey event (already dispatched by the boundary). Only an
    /// `Active` registration with an exact match yields an action; a paused or
    /// disabled one ignores the press.
    pub fn on_event(&self, vk: u8, modifiers: HotkeyModifiers) -> Option<HotkeyAction> {
        let HotkeyState::Active(combo) = self.state else {
            return None;
        };
        if combo.key.vk_code() == vk && modifier_mask(combo.modifiers) == modifier_mask(modifiers) {
            Some(HotkeyAction::Show)
        } else {
            None
        }
    }

    /// Whether two modifier sets are equivalent for matching.
    pub fn modifiers_match(a: HotkeyModifiers, b: HotkeyModifiers) -> bool {
        modifier_mask(a) == modifier_mask(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn combo(control: bool, alt: bool, shift: bool, win: bool, key: HotkeyKey) -> HotkeySetting {
        HotkeySetting {
            modifiers: HotkeyModifiers {
                control,
                alt,
                shift,
                win,
            },
            key,
        }
    }

    fn vk(v: u8) -> HotkeyKey {
        HotkeyKey::Vk { vk: v }
    }
    fn func(i: u8) -> HotkeyKey {
        HotkeyKey::Function { index: i }
    }

    fn ctrl_alt_space() -> HotkeySetting {
        combo(true, true, false, false, vk(0x20))
    }

    const CTRL_ALT: HotkeyModifiers = HotkeyModifiers {
        control: true,
        alt: true,
        shift: false,
        win: false,
    };

    /// A test registry recording calls into a shared log.
    #[derive(Clone, Default)]
    struct FakeRegistry {
        log: Rc<RefCell<Vec<&'static str>>>,
        fail_next: Rc<RefCell<Option<HotkeyErrorKind>>>,
    }

    impl FakeRegistry {
        fn set_registered(&mut self, set: &'static str) {
            self.log.borrow_mut().push(set);
        }
        fn fail(kind: HotkeyErrorKind) -> Self {
            let reg = FakeRegistry::default();
            *reg.fail_next.borrow_mut() = Some(kind);
            reg
        }
    }

    impl HotkeyRegistry for FakeRegistry {
        fn register(&mut self, _combo: HotkeySetting) -> Result<(), HotkeyErrorKind> {
            if let Some(kind) = self.fail_next.borrow_mut().take() {
                return Err(kind);
            }
            self.set_registered("register");
            Ok(())
        }
        fn unregister(&mut self) -> Result<(), HotkeyErrorKind> {
            self.set_registered("unregister");
            Ok(())
        }
    }

    // --- validation table ------------------------------------------------

    #[test]
    fn default_ctrl_alt_space_is_valid() {
        assert!(validate(CTRL_ALT, vk(0x20)).is_ok());
    }

    #[test]
    fn bare_letter_or_digit_is_rejected_without_a_modifier() {
        assert_eq!(
            validate(HotkeyModifiers::default(), vk(b'A')),
            Err(HotkeyValidationError::NeedsModifier)
        );
        assert_eq!(
            validate(HotkeyModifiers::default(), vk(b'1')),
            Err(HotkeyValidationError::NeedsModifier)
        );
        // With a modifier it is fine.
        assert!(
            validate(
                HotkeyModifiers {
                    control: true,
                    ..Default::default()
                },
                vk(b'A')
            )
            .is_ok()
        );
    }

    #[test]
    fn space_alone_is_rejected() {
        assert_eq!(
            validate(HotkeyModifiers::default(), vk(0x20)),
            Err(HotkeyValidationError::NeedsModifier)
        );
    }

    #[test]
    fn single_win_is_rejected_for_any_key() {
        assert_eq!(
            validate(
                HotkeyModifiers {
                    win: true,
                    ..Default::default()
                },
                vk(b'A')
            ),
            Err(HotkeyValidationError::WinOnly)
        );
        assert_eq!(
            validate(
                HotkeyModifiers {
                    win: true,
                    ..Default::default()
                },
                func(5)
            ),
            Err(HotkeyValidationError::WinOnly)
        );
        // Win plus another modifier is allowed.
        assert!(
            validate(
                HotkeyModifiers {
                    win: true,
                    control: true,
                    ..Default::default()
                },
                vk(b'A')
            )
            .is_ok()
        );
    }

    #[test]
    fn function_keys_are_valid_with_any_modifiers_or_none() {
        assert!(validate(HotkeyModifiers::default(), func(1)).is_ok());
        assert!(validate(HotkeyModifiers::default(), func(24)).is_ok());
        assert!(
            validate(
                HotkeyModifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                func(12)
            )
            .is_ok()
        );
        assert_eq!(
            validate(HotkeyModifiers::default(), func(0)),
            Err(HotkeyValidationError::KeyOutOfRange)
        );
        assert_eq!(
            validate(HotkeyModifiers::default(), func(25)),
            Err(HotkeyValidationError::KeyOutOfRange)
        );
    }

    #[test]
    fn out_of_range_vk_is_rejected() {
        assert_eq!(
            validate(
                HotkeyModifiers {
                    control: true,
                    ..Default::default()
                },
                vk(0x1F)
            ),
            Err(HotkeyValidationError::KeyOutOfRange)
        );
        assert_eq!(
            validate(
                HotkeyModifiers {
                    control: true,
                    ..Default::default()
                },
                vk(0x5B)
            ),
            Err(HotkeyValidationError::KeyOutOfRange)
        );
        // VK_DELETE (0x2E) is inside the range; alone with Ctrl it is allowed.
        assert!(
            validate(
                HotkeyModifiers {
                    control: true,
                    ..Default::default()
                },
                vk(0x2E)
            )
            .is_ok()
        );
    }

    #[test]
    fn reserved_system_combinations_are_rejected() {
        let cases = [
            (
                HotkeyModifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                0x2E,
                "ctrl+alt+del",
            ),
            (
                HotkeyModifiers {
                    alt: true,
                    ..Default::default()
                },
                0x09,
                "alt+tab",
            ),
            (
                HotkeyModifiers {
                    alt: true,
                    ..Default::default()
                },
                0x1B,
                "alt+esc",
            ),
            (
                HotkeyModifiers {
                    control: true,
                    ..Default::default()
                },
                0x1B,
                "ctrl+esc",
            ),
        ];
        for (mods, key_vk, label) in cases {
            assert_eq!(
                validate(mods, vk(key_vk)),
                Err(HotkeyValidationError::Reserved),
                "{label} must be reserved"
            );
        }
        // The same modifier set with a DIFFERENT (non-reserved, in-range) key is
        // fine — e.g. Alt+Q.
        assert!(
            validate(
                HotkeyModifiers {
                    alt: true,
                    ..Default::default()
                },
                vk(0x51)
            )
            .is_ok()
        );
        // Shift+Tab is out-of-range for our accepted Vk window — a different,
        // benign error is fine (reserved only fires on an exact table match).
        assert_eq!(
            validate(
                HotkeyModifiers {
                    shift: true,
                    ..Default::default()
                },
                vk(0x09)
            ),
            Err(HotkeyValidationError::KeyOutOfRange)
        );
    }

    // --- state machine ---------------------------------------------------

    #[test]
    fn startup_with_default_registers_and_is_active() {
        let machine = HotkeyMachine::new(FakeRegistry::default(), Some(ctrl_alt_space()));
        assert_eq!(machine.state(), HotkeyState::Active(ctrl_alt_space()));
        assert_eq!(machine.last_error(), None);
        assert_eq!(machine.on_event(0x20, CTRL_ALT), Some(HotkeyAction::Show));
    }

    #[test]
    fn startup_with_no_hotkey_is_disabled() {
        let machine = HotkeyMachine::new(FakeRegistry::default(), None);
        assert_eq!(machine.state(), HotkeyState::Disabled);
        assert_eq!(machine.on_event(0x20, CTRL_ALT), None);
    }

    #[test]
    fn startup_with_invalid_combo_stays_disabled() {
        let machine = HotkeyMachine::new(
            FakeRegistry::default(),
            Some(combo(false, false, false, false, vk(b'A'))),
        );
        assert_eq!(machine.state(), HotkeyState::Disabled);
    }

    #[test]
    fn startup_with_conflict_is_disabled_and_reports() {
        let machine = HotkeyMachine::new(
            FakeRegistry::fail(HotkeyErrorKind::Conflict),
            Some(ctrl_alt_space()),
        );
        assert_eq!(machine.state(), HotkeyState::Disabled);
        assert_eq!(machine.last_error(), Some(HotkeyErrorKind::Conflict));
    }

    #[test]
    fn set_swaps_combo_and_old_press_stops_firing() {
        let machine = HotkeyMachine::new(FakeRegistry::default(), Some(ctrl_alt_space()));
        let mut machine = machine;
        let new_combo = combo(true, false, false, false, vk(0x20)); // Ctrl+Space
        assert!(machine.set(new_combo).is_ok());
        assert_eq!(machine.state(), HotkeyState::Active(new_combo));
        assert_eq!(
            machine.on_event(
                0x20,
                HotkeyModifiers {
                    control: true,
                    alt: false,
                    shift: false,
                    win: false
                }
            ),
            Some(HotkeyAction::Show)
        );
        assert_eq!(machine.on_event(0x20, CTRL_ALT), None);
    }

    #[test]
    fn set_keeps_old_registration_when_new_conflicts() {
        let registry = FakeRegistry::default();
        let mut machine = HotkeyMachine::new(registry, Some(ctrl_alt_space()));
        *machine.registry.fail_next.borrow_mut() = Some(HotkeyErrorKind::Conflict);
        let new_combo = combo(true, false, false, false, vk(0x20));
        assert_eq!(machine.set(new_combo), Err(HotkeyErrorKind::Conflict));
        // Old combo is preserved and back to Active.
        assert_eq!(machine.state(), HotkeyState::Active(ctrl_alt_space()));
        assert_eq!(machine.on_event(0x20, CTRL_ALT), Some(HotkeyAction::Show));
    }

    #[test]
    fn set_rejects_invalid_combo_without_touching_registry() {
        let registry = FakeRegistry::default();
        let mut machine = HotkeyMachine::new(registry, Some(ctrl_alt_space()));
        let calls_before = machine.registry.log.borrow().len();
        assert_eq!(
            machine.set(combo(false, false, false, false, vk(b'A'))),
            Err(HotkeyErrorKind::Unavailable)
        );
        assert_eq!(machine.registry.log.borrow().len(), calls_before);
        assert_eq!(machine.state(), HotkeyState::Active(ctrl_alt_space()));
    }

    #[test]
    fn clear_disables_and_a_press_does_nothing() {
        let mut machine = HotkeyMachine::new(FakeRegistry::default(), Some(ctrl_alt_space()));
        assert!(machine.clear().is_ok());
        assert_eq!(machine.state(), HotkeyState::Disabled);
        assert_eq!(machine.on_event(0x20, CTRL_ALT), None);
    }

    #[test]
    fn pause_suppresses_presses_and_resume_restores() {
        let mut machine = HotkeyMachine::new(FakeRegistry::default(), Some(ctrl_alt_space()));
        machine.pause();
        assert_eq!(machine.state(), HotkeyState::Paused(ctrl_alt_space()));
        assert_eq!(
            machine.on_event(0x20, CTRL_ALT),
            None,
            "paused presses must be ignored"
        );
        machine.resume();
        assert_eq!(machine.state(), HotkeyState::Active(ctrl_alt_space()));
        assert_eq!(machine.on_event(0x20, CTRL_ALT), Some(HotkeyAction::Show));
    }

    #[test]
    fn non_matching_press_is_ignored() {
        let machine = HotkeyMachine::new(FakeRegistry::default(), Some(ctrl_alt_space()));
        assert_eq!(machine.on_event(0x43, CTRL_ALT), None);
        assert_eq!(
            machine.on_event(
                0x20,
                HotkeyModifiers {
                    control: true,
                    alt: false,
                    shift: false,
                    win: false
                }
            ),
            None
        );
    }

    #[test]
    fn clear_and_disabled_are_idempotent() {
        let mut machine = HotkeyMachine::new(FakeRegistry::default(), None);
        assert!(machine.clear().is_ok());
        assert_eq!(machine.state(), HotkeyState::Disabled);
        machine.pause();
        assert_eq!(machine.state(), HotkeyState::Disabled);
        machine.resume();
        assert_eq!(machine.state(), HotkeyState::Disabled);
    }

    // F001 contract: `NativePlatform::start` builds the machine in a pending
    // (no-setting) state and calls `set(combo)` only once the worker has
    // published its HWND. This mirrors that two-step wiring and proves that a
    // registration issued after the "window ready" point wins (Active + fires),
    // whereas the old single-step `new(registry, setting)` upfront raced the
    // worker and could silently stay Disabled forever.
    #[test]
    fn startup_in_two_steps_registers_once_the_hwnd_is_ready() {
        let registry = FakeRegistry::default();
        // Step 1: pending machine, no registration yet (worker still creating
        // the window).
        let mut machine = HotkeyMachine::new(registry, None);
        assert_eq!(machine.state(), HotkeyState::Disabled);
        assert_eq!(machine.last_error(), None);

        // Step 2: the window is up; register the default combo via `set`.
        assert!(machine.set(ctrl_alt_space()).is_ok());
        assert_eq!(machine.state(), HotkeyState::Active(ctrl_alt_space()));
        assert_eq!(machine.last_error(), None);
        assert_eq!(machine.on_event(0x20, CTRL_ALT), Some(HotkeyAction::Show));

        // A conflict at this step surfaces as `Disabled` + `last_error`; never
        // a silent success.
        let conflicting = FakeRegistry::fail(HotkeyErrorKind::Conflict);
        let mut machine = HotkeyMachine::new(conflicting, None);
        assert_eq!(
            machine.set(ctrl_alt_space()),
            Err(HotkeyErrorKind::Conflict)
        );
        assert_eq!(machine.state(), HotkeyState::Disabled);
        assert_eq!(machine.last_error(), Some(HotkeyErrorKind::Conflict));
    }
}
