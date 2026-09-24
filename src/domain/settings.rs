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
/// Minimum window width for the settings window (logical px).
pub const MIN_SETTINGS_WIDTH: u16 = 760;
/// Default settings-window width (logical px).
pub const DEFAULT_SETTINGS_WIDTH: u16 = 900;
/// Default settings-window height (logical px).
pub const DEFAULT_SETTINGS_HEIGHT: u16 = 640;
/// Row-height compaction modes for the management list (M06.4).
pub const DEFAULT_ROW_HEIGHT_COMPACT: u16 = 34;
pub const DEFAULT_ROW_HEIGHT_STANDARD: u16 = 44;
/// Font scale multiplier percent (100 = 1.0x; 80..=150).
pub const DEFAULT_FONT_SCALE_PERCENT: u16 = 100;
pub const MIN_FONT_SCALE_PERCENT: u16 = 80;
pub const MAX_FONT_SCALE_PERCENT: u16 = 150;

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

/// Row-height compaction mode for the management list (M06.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowHeightPreference {
    /// Compact rows (default).
    #[default]
    Compact,
    /// Standard-height rows.
    Standard,
}

/// Which monitor the search window favors for placement (M06.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorStrategy {
    /// Place on the monitor containing the cursor (default; current M04.4
    /// behavior).
    #[default]
    Mouse,
    /// Place on the monitor containing the foreground (active) window. The
    /// placement is genuinely implemented in
    /// `platform::windows::window_placement::placement_rect_for_active_window`
    /// (`GetWindowRect` → monitor work area, DPI-aware); when the active-window
    /// handle is invalid/unavailable the placement falls back to the cursor
    /// monitor (same as [`MonitorStrategy::Mouse`]). M06 review M2: corrected
    /// docstring to match the real wiring.
    ActiveWindow,
}

/// The explicitly chosen UI language (M06.2). `System` follows the OS locale;
/// zh-CN / en-US override it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    ZhCN,
    EnUS,
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

    #[test]
    fn one_level_import_defaults_to_off_and_round_trips_forward_compatibly() {
        // M05.2: default OFF.
        assert!(!AppSettings::default().one_level_import);
        // A document without the field must decode with `false` (forward-compat).
        #[derive(Debug, Deserialize)]
        struct Legacy {
            #[serde(default)]
            one_level_import: bool,
        }
        let legacy: Legacy = serde_json::from_str(r#"{}"#).expect("decode legacy");
        assert!(!legacy.one_level_import);
        // A document with the field on round-trips.
        let settings = AppSettings {
            one_level_import: true,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).expect("encode");
        let decoded: AppSettings = serde_json::from_str(&json).expect("decode");
        assert!(decoded.one_level_import);
    }

    #[test]
    fn old_schema_v1_json_without_m06_fields_decodes_with_defaults() {
        // Forward-compat (M06 constraint): an old schema-v1 settings object
        // that predates every M06 field must decode with the M06 defaults —
        // never an error and never a lost default. The pre-M06 field set was
        // the complete `AppSettings` of M01–M05 (theme..launch_at_login);
        // adding the M06 fields with `#[serde(default)]` keeps old documents
        // valid while the new fields fall back to their defaults.
        let pre_m06 = r#"{
            "theme":"system",
            "max_results":8,
            "window_width":600,
            "search_paths":true,
            "search_categories":true,
            "search_tags":true,
            "search_notes":false,
            "fuzzy_matching":true,
            "search_pinyin":true,
            "search_english_initials":true,
            "max_edit_distance":1,
            "hide_after_open":true,
            "clear_after_open":true,
            "hide_on_focus_loss":true,
            "launch_at_login":false
        }"#;
        let settings: AppSettings = serde_json::from_str(pre_m06).expect("decode");
        assert!(!settings.show_main_window_at_startup);
        assert!(settings.silent_start);
        assert!(!settings.launch_at_login_wired);
        assert_eq!(settings.monitor_strategy, MonitorStrategy::Mouse);
        assert_eq!(settings.language_preference, LanguagePreference::System);
        assert_eq!(settings.row_height_preference, RowHeightPreference::Compact);
        assert_eq!(settings.search_window_width, None);
        assert_eq!(settings.settings_window_width, None);
        assert_eq!(settings.font_scale_percent, DEFAULT_FONT_SCALE_PERCENT);
        assert!(settings.show_path_in_results);
        assert!(settings.show_category_tag_in_results);
        assert!(settings.highlight_results);
    }

    #[test]
    fn m06_ranges_validate_or_reject() {
        // Valid defaults pass.
        assert!(AppSettings::default().validate().is_ok());
        // Out-of-range font scale rejects.
        let bad_font = AppSettings {
            font_scale_percent: MAX_FONT_SCALE_PERCENT + 1,
            ..AppSettings::default()
        };
        assert!(bad_font.validate().is_err());
        let low_font = AppSettings {
            font_scale_percent: MIN_FONT_SCALE_PERCENT - 1,
            ..AppSettings::default()
        };
        assert!(low_font.validate().is_err());
        // A settings-window width below the minimum rejects; equal is fine.
        let narrow = AppSettings {
            settings_window_width: Some(MIN_SETTINGS_WIDTH - 1),
            ..AppSettings::default()
        };
        assert!(narrow.validate().is_err());
        let min = AppSettings {
            settings_window_width: Some(MIN_SETTINGS_WIDTH),
            ..AppSettings::default()
        };
        assert!(min.validate().is_ok());
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
    /// Whether the search core indexes folder aliases (M06.4; the alias keys
    /// already exist, this gates them like the other field toggles).
    #[serde(default)]
    pub search_aliases: bool,
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
    /// M05.2: whether picking a parent directory offers a controlled one-level
    /// import of its direct children (max 100, user-initiated and bounded).
    /// Default OFF; forward-compatible with `#[serde(default)]`.
    #[serde(default)]
    pub one_level_import: bool,

    // ---- M06 new settings (forward-compatible: #[serde(default)]) -------
    /// Show the main search window at startup, in addition to the tray. When
    /// false the app starts silently in the tray (default; see
    /// [`Self::silent_start`]).
    #[serde(default)]
    pub show_main_window_at_startup: bool,
    /// Whether to launch fully minimal (tray only, no window, no window
    /// placement). Default true. `silent_start == false` implies "start
    /// visible" unless the platform otherwise suppresses it.
    #[serde(default = "default_true")]
    pub silent_start: bool,
    /// M06 review L1: LEGACY forward-compat field. The HKCU Run value is the
    /// single source of truth (tracked live by
    /// `SettingsWindowController::set_launch_at_login_os` and read from the
    /// registry at startup/toggle), so this persisted flag is never read or
    /// written by any logic. It is KEPT because `AppSettings` decodes with
    /// `#[serde(deny_unknown_fields)]`: every M06-era on-disk document
    /// serializes this key, so dropping the field would make existing data
    /// files fail to load. Retain as a defaulted, always-false legacy field.
    #[serde(default)]
    pub launch_at_login_wired: bool,
    /// Which monitor the search window targets (M06.2).
    #[serde(default)]
    pub monitor_strategy: MonitorStrategy,
    /// Explicit UI language override (M06.2). `System` follows the OS locale.
    #[serde(default)]
    pub language_preference: LanguagePreference,
    /// Row-height compaction for the management list (M06.4).
    #[serde(default)]
    pub row_height_preference: RowHeightPreference,
    /// Search-window width override (logical px); `MIN_WINDOW_WIDTH..`
    /// clamp. `None` keeps the window component's default width.
    #[serde(default)]
    pub search_window_width: Option<u16>,
    /// Settings-window width override (logical px); `MIN_SETTINGS_WIDTH..`
    /// clamp. `None` keeps the component default.
    #[serde(default)]
    pub settings_window_width: Option<u16>,
    /// Font scale percent (80..=150) applied to the default font size (M06.4).
    #[serde(default = "default_font_scale")]
    pub font_scale_percent: u16,
    /// Whether row-2 of each result shows the path label (M06.4).
    #[serde(default = "default_true")]
    pub show_path_in_results: bool,
    /// Whether row-2 appends the category/tag line (M06.4).
    #[serde(default = "default_true")]
    pub show_category_tag_in_results: bool,
    /// Enable result highlight ranges (M06.4; Slint render wiring is a manual
    /// acceptance item, the flag persists and drives
    /// [`crate::search::HighlightOptions`]).
    #[serde(default = "default_true")]
    pub highlight_results: bool,
    /// Prefer recently-used entries within equal relevance (M06.4 "最近时间排
    /// 序"). When on, `last_opened_at` descending becomes the primary
    /// same-relevance tie-break (after manual weight and pinned) in the search
    /// core's deterministic ordering.
    #[serde(default)]
    pub recent_sort_first: bool,
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
            search_aliases: true,
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
            one_level_import: false,
            show_main_window_at_startup: false,
            silent_start: true,
            launch_at_login_wired: false,
            monitor_strategy: MonitorStrategy::Mouse,
            language_preference: LanguagePreference::System,
            row_height_preference: RowHeightPreference::Compact,
            search_window_width: None,
            settings_window_width: None,
            font_scale_percent: DEFAULT_FONT_SCALE_PERCENT,
            show_path_in_results: true,
            show_category_tag_in_results: true,
            highlight_results: true,
            recent_sort_first: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_font_scale() -> u16 {
    DEFAULT_FONT_SCALE_PERCENT
}

impl AppSettings {
    /// The effective search-window width: the persisted override, or the
    /// window component's default (`DEFAULT_WINDOW_WIDTH`).
    pub fn search_window_width_or_default(&self) -> u16 {
        self.search_window_width
            .unwrap_or(DEFAULT_WINDOW_WIDTH)
            .clamp(MIN_WINDOW_WIDTH, MAX_WINDOW_WIDTH)
    }

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

        if !(MIN_FONT_SCALE_PERCENT..=MAX_FONT_SCALE_PERCENT).contains(&self.font_scale_percent) {
            return Err(ValidationError::new(ValidationErrorKind::InvalidSetting));
        }

        if self
            .settings_window_width
            .is_some_and(|width| width < MIN_SETTINGS_WIDTH)
        {
            return Err(ValidationError::new(ValidationErrorKind::InvalidSetting));
        }

        Ok(())
    }
}
