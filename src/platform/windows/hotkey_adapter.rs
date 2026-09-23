//! Hotkey adapter: maps the pure [`HotkeyMachine`] onto `RegisterHotKey`.
//!
//! The hidden window's HWND is created by the worker thread, so the registry
//! holds a shared slot (`Arc<Mutex<Option<HWND>>>`) that the worker fills the
//! moment the window exists; registrations before that return `Unavailable`.
//! Raw Win32 codes never escape this module.

use crate::domain::settings::{HotkeyModifiers, HotkeySetting};
use crate::platform::hotkey::{HotkeyErrorKind, HotkeyRegistry};

use windows::Win32::{
    Foundation::HWND,
    UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
        UnregisterHotKey,
    },
};

/// A fixed, process-global hotkey id bound to the hidden window.
pub const HOTKEY_ID: i32 = 1;

/// Map a [`HotkeySetting`] onto Win32 modifier bits. `MOD_NOREPEAT` is always
/// added so holding the combo fires once, not a key-repeat storm.
pub fn hotkey_modifiers(mods: HotkeyModifiers) -> HOT_KEY_MODIFIERS {
    let mut mask = 0u32;
    if mods.control {
        mask |= MOD_CONTROL.0;
    }
    if mods.alt {
        mask |= MOD_ALT.0;
    }
    if mods.shift {
        mask |= MOD_SHIFT.0;
    }
    if mods.win {
        mask |= MOD_WIN.0;
    }
    HOT_KEY_MODIFIERS(mask | MOD_NOREPEAT.0)
}

/// Shared, thread-safe slot that the worker thread fills with the hidden
/// window's HWND once it is created.
///
/// `HWND` is `*mut c_void` and therefore not `Send`, so the slot stores the
/// raw pointer bits as `isize` (the canonical way to move a pointer handle
/// across a channel/mutex) and re-wraps them on use.
pub type HwndSlot = std::sync::Arc<std::sync::Mutex<Option<isize>>>;

pub fn shared_hwnd_slot() -> HwndSlot {
    std::sync::Arc::new(std::sync::Mutex::new(None))
}

fn hwnd_to_isize(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn isize_to_hwnd(bits: isize) -> HWND {
    HWND(bits as *mut core::ffi::c_void)
}

/// The `HotkeyRegistry` implementation over `RegisterHotKey`/`UnregisterHotKey`.
pub struct Win32HotkeyRegistry {
    hwnd: HwndSlot,
}

impl Win32HotkeyRegistry {
    pub fn new(hwnd: HwndSlot) -> Self {
        Self { hwnd }
    }

    /// Record the worker's hidden window in the shared slot.
    pub fn publish_hwnd(&self, hwnd: HWND) {
        *self.hwnd.lock().expect("hwnd slot lock") = Some(hwnd_to_isize(hwnd));
    }

    /// The current hidden window, if the worker has published it.
    pub fn current_hwnd(&self) -> Option<HWND> {
        self.hwnd.lock().expect("hwnd slot lock").map(isize_to_hwnd)
    }
}

impl HotkeyRegistry for Win32HotkeyRegistry {
    fn register(&mut self, combo: HotkeySetting) -> Result<(), HotkeyErrorKind> {
        let hwnd = self.current_hwnd().ok_or(HotkeyErrorKind::Unavailable)?;
        let vk = u32::from(combo.key.vk_code());
        // Safety: `hwnd` is the live hidden window on the worker thread; the id
        // is process-global and never reused while registered.
        unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, hotkey_modifiers(combo.modifiers), vk) }
            .map_err(|error| {
                if error.code().0 == HRESULT_FROM_WIN32_ERRORHOTKEY {
                    HotkeyErrorKind::Conflict
                } else {
                    HotkeyErrorKind::Unavailable
                }
            })
    }

    fn unregister(&mut self) -> Result<(), HotkeyErrorKind> {
        let hwnd = self.current_hwnd().ok_or(HotkeyErrorKind::Unavailable)?;
        // Safety: same rationale as register.
        unsafe { UnregisterHotKey(Some(hwnd), HOTKEY_ID) }.map_err(|_| HotkeyErrorKind::Unavailable)
    }
}

/// HRESULT of `ERROR_HOTKEY_ALREADY_REGISTERED` (1409): `0x80070581`
/// (negative as an `i32`).
const HRESULT_FROM_WIN32_ERRORHOTKEY: i32 = -2_147_023_487;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_mapping_is_exact_and_always_norepeat() {
        let mods = HotkeyModifiers {
            control: true,
            alt: true,
            shift: false,
            win: false,
        };
        let mapped = hotkey_modifiers(mods);
        assert_ne!(mapped.0 & MOD_CONTROL.0, 0);
        assert_ne!(mapped.0 & MOD_ALT.0, 0);
        assert_eq!(mapped.0 & MOD_SHIFT.0, 0);
        assert_eq!(mapped.0 & MOD_WIN.0, 0);
        assert_ne!(mapped.0 & MOD_NOREPEAT.0, 0);
    }

    #[test]
    fn registry_without_hwnd_reports_unavailable() {
        let slot = shared_hwnd_slot();
        let mut registry = Win32HotkeyRegistry::new(slot);
        let combo = HotkeySetting {
            modifiers: HotkeyModifiers {
                control: true,
                alt: true,
                shift: false,
                win: false,
            },
            key: crate::domain::settings::HotkeyKey::Vk { vk: 0x20 },
        };
        assert_eq!(registry.register(combo), Err(HotkeyErrorKind::Unavailable));
        assert_eq!(registry.unregister(), Err(HotkeyErrorKind::Unavailable));
    }

    #[test]
    fn conflict_hrresult_is_stable() {
        // ERROR_HOTKEY_ALREADY_REGISTERED = 1409 → HRESULT 0x80070581 as i32.
        assert_eq!(HRESULT_FROM_WIN32_ERRORHOTKEY, -2_147_023_487);
        // The From<HRESULT> for Error mapping keeps the code; assert the code
        // round-trips through HRESULT::from_win32.
        let error: windows::core::Error = windows::core::HRESULT::from_win32(1409u32).into();
        assert_eq!(error.code().0, HRESULT_FROM_WIN32_ERRORHOTKEY);
    }

    #[test]
    fn slot_starts_empty_and_can_be_filled() {
        let slot = shared_hwnd_slot();
        assert_eq!(*slot.lock().unwrap(), None);
        *slot.lock().unwrap() = Some(hwnd_to_isize(HWND::default()));
        assert!(slot.lock().unwrap().is_some());
        // Round-trip the handle bits.
        let round = slot.lock().unwrap().map(isize_to_hwnd).unwrap();
        assert_eq!(round, HWND::default());
    }
}
