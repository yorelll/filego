use serde::{Deserialize, Serialize};

use super::error::{ValidationError, ValidationErrorKind};

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
