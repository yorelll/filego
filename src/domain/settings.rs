use serde::{Deserialize, Serialize};

use super::error::{ValidationError, ValidationErrorKind};

/// The default global hotkey combination: Ctrl+Alt+Space.
pub const DEFAULT_HOTKEY_MODIFIERS: HotkeyModifiers = HotkeyModifiers {
    control: true,
    alt: true,
    shift: false,
    win: false,
};
pub const DEFAULT_HOTKEY_KEY: HotkeyKey = HotkeyKey::Vk { vk: 0x20 }; // VK_SPACE

pub const DEFAULT_MAX_RESULTS: u16 = 8;
pub const MIN_MAX_RESULTS: u16 = 1;
pub const MAX_MAX_RESULTS: u16 = 100;
pub const DEFAULT_WINDOW_WIDTH: u16 = 600;
pub const MIN_WINDOW_WIDTH: u16 = 480;
pub const MAX_WINDOW_WIDTH: u16 = 760;
pub const DEFAULT_MAX_EDIT_DISTANCE: u8 = 1;
pub const MAX_EDIT_DISTANCE: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

/// What the search window shows while the query is blank (M02.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyQueryStrategy {
    /// Show up to `MAX_FAVORITES` favorites first, then pinned and recently
    /// opened entries. Default.
    #[default]
    FavoritesFirst,
    /// Show every entry in the deterministic rank order.
    All,
    /// Show pinned entries only.
    PinnedOnly,
    /// Keep the list blank until the user types.
    Blank,
}

/// Modifier keys of a global hotkey combination (M04.2).
///
/// This struct is deliberately plain booleans so the ViewModel/validation layer
/// stays free of Win32 constants; the boundary module maps it onto
/// `HOT_KEY_MODIFIERS` (MOD_CONTROL / MOD_ALT / MOD_SHIFT / MOD_WIN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// The non-modifier key of a global hotkey combination (M04.2).
///
/// Validated combinations are either a non-Win modifier plus an alphanumeric
/// key, a function key (F1–F24), or at minimum one modifier with any key that
/// is not a bare Win combination (rules table in
/// [`crate::platform::hotkey::validate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyKey {
    /// A printable / main-section key represented by its `u8` Windows virtual
    /// key code. Serde encodes it as `{"vk": 0x20}` (space). The validation
    /// rules accept `0x20`..=`0x5A` (Space..=Z) with a non-Win modifier.
    Vk { vk: u8 },
    /// A function key F1..=F24 (1-based). Serde encodes it as
    /// `{"function": 1}`.
    Function { index: u8 },
}

impl HotkeyKey {
    pub const fn vk_code(self) -> u8 {
        match self {
            HotkeyKey::Vk { vk } => vk,
            HotkeyKey::Function { index } => 0x70_u8.saturating_add(index.saturating_sub(1)),
        }
    }

    pub const fn is_function(self) -> bool {
        matches!(self, HotkeyKey::Function { .. })
    }
}

/// A validated global-hotkey setting (M04.2). `None` (in `AppSettings::hotkey`)
/// means "no hotkey".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeySetting {
    pub modifiers: HotkeyModifiers,
    pub key: HotkeyKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HotkeySetting {
        HotkeySetting {
            modifiers: HotkeyModifiers {
                control: true,
                alt: true,
                shift: false,
                win: false,
            },
            key: HotkeyKey::Vk { vk: 0x20 },
        }
    }

    #[test]
    fn hotkey_round_trips_through_serde_json() {
        let json = serde_json::to_string(&sample()).expect("encode");
        assert_eq!(
            json,
            r#"{"modifiers":{"control":true,"alt":true,"shift":false,"win":false},"key":{"Vk":{"vk":32}}}"#
        );
        let decoded: HotkeySetting = serde_json::from_str(&json).expect("decode");
        assert_eq!(decoded, sample());
    }

    #[test]
    fn missing_hotkey_decodes_as_absent_in_settings() {
        // Forward-compat: an old document without the `hotkey` field must decode
        // with `hotkey: None`, and `default()` still yields the default combo.
        #[derive(Debug, Deserialize)]
        struct Legacy {
            #[serde(default)]
            hotkey: Option<HotkeySetting>,
        }
        let legacy: Legacy = serde_json::from_str(r#"{}"#).expect("decode legacy");
        assert_eq!(legacy.hotkey, None);

        let settings = AppSettings::default();
        assert_eq!(
            settings.hotkey,
            Some(HotkeySetting {
                modifiers: DEFAULT_HOTKEY_MODIFIERS,
                key: DEFAULT_HOTKEY_KEY,
            })
        );
    }

    #[test]
    fn function_key_maps_to_the_correct_vk_range() {
        assert_eq!(HotkeyKey::Function { index: 1 }.vk_code(), 0x70);
        assert_eq!(HotkeyKey::Function { index: 12 }.vk_code(), 0x7B);
        assert_eq!(HotkeyKey::Function { index: 24 }.vk_code(), 0x87);
        assert!(HotkeyKey::Function { index: 5 }.is_function());
        assert!(!HotkeyKey::Vk { vk: 0x43 }.is_function());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub theme: ThemePreference,
    pub max_results: u16,
    pub window_width: u16,
    pub search_paths: bool,
    pub search_categories: bool,
    pub search_tags: bool,
    pub search_notes: bool,
    pub fuzzy_matching: bool,
    /// M02 must derive pinyin keys from names and aliases; it must not persist a scan index.
    pub search_pinyin: bool,
    pub search_english_initials: bool,
    pub max_edit_distance: u8,
    pub hide_after_open: bool,
    pub clear_after_open: bool,
    pub hide_on_focus_loss: bool,
    pub launch_at_login: bool,
    /// Empty-query display strategy (M02.4); default
    /// [`EmptyQueryStrategy::FavoritesFirst`].
    #[serde(default)]
    pub empty_query_strategy: EmptyQueryStrategy,
    /// Whether to remember the last active filter (M02.3). Only persisted
    /// forward-compatibly here; not wired to any UI yet.
    #[serde(default)]
    pub remember_last_filter: bool,
    /// The global search hotkey (M04.2). `None` disables the hotkey. Persisted
    /// forward-compatibly with `#[serde(default)]` so documents written before
    /// M04 stay valid; `None` means old documents keep the default via the
    /// effective-hotkey projection in the adapter, not by mutating the stored
    /// document.
    #[serde(default)]
    pub hotkey: Option<HotkeySetting>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            max_results: DEFAULT_MAX_RESULTS,
            window_width: DEFAULT_WINDOW_WIDTH,
            search_paths: true,
            search_categories: true,
            search_tags: true,
            search_notes: false,
            fuzzy_matching: true,
            search_pinyin: true,
            search_english_initials: true,
            max_edit_distance: DEFAULT_MAX_EDIT_DISTANCE,
            hide_after_open: true,
            clear_after_open: true,
            hide_on_focus_loss: true,
            launch_at_login: false,
            empty_query_strategy: EmptyQueryStrategy::FavoritesFirst,
            remember_last_filter: false,
            hotkey: Some(HotkeySetting {
                modifiers: DEFAULT_HOTKEY_MODIFIERS,
                key: DEFAULT_HOTKEY_KEY,
            }),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !(MIN_MAX_RESULTS..=MAX_MAX_RESULTS).contains(&self.max_results) {
            return Err(ValidationError::new(ValidationErrorKind::InvalidSetting));
        }

        if !(MIN_WINDOW_WIDTH..=MAX_WINDOW_WIDTH).contains(&self.window_width) {
            return Err(ValidationError::new(ValidationErrorKind::InvalidSetting));
        }

        if self.max_edit_distance > MAX_EDIT_DISTANCE {
            return Err(ValidationError::new(ValidationErrorKind::InvalidSetting));
        }

        Ok(())
    }
}
