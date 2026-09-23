//! Observable window state exchanged between the ViewModel and UI adapters.
//!
//! Every type here is plain data (no Slint, no I/O, no wall clock). The search
//! results are pre-rendered into display rows so the Slint layer only has to
//! map a row index to fields — there is no query/path re-derivation in the UI.
//! Privacy rule: these structs carry user data because they are the *display*
//! state, but they are never serialized and never written to logs.

use super::theme::ResolvedTheme;
use crate::search::NoResultReason;

/// A fully rendered row for the results list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRow {
    /// Entry id that Rust can use to look up the open target.
    pub entry_id: uuid::Uuid,
    pub display_name: String,
    /// The resolved relative-path label (parent folder name).
    pub path_label: String,
    /// The full path used for the path tooltip / Copy-Path; never logged.
    pub full_path: String,
    /// Category display name, if any.
    pub category_name: Option<String>,
    /// Tag display names that still fit the prompt list.
    pub tag_texts: Vec<String>,
    /// True when the path was reported inaccessible.
    pub inaccessible: bool,
}

/// Search failure that is actionable but anonymous: it never carries a raw
/// Win32 code or a full path. The detail stream is optional and privacy-gated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFailure {
    /// A load failed (corrupt/unreadable data, recovery required, ...).
    Load,
    /// A save failed while persisting state.
    Save,
    /// The search itself cannot run against the given data.
    Search,
    /// The shell could not open the selected folder (M04.5). The window stays
    /// up so Retry (Enter) and Copy (Ctrl+C) keep working on the selection.
    Open,
}

impl SearchFailure {
    /// Privacy-safe detail for a diagnostics stream (see `crate::diagnostics`).
    pub fn as_detail(&self) -> &'static str {
        match self {
            SearchFailure::Load => {
                "stored data could not be loaded; a corrupt file is preserved and recoverable"
            }
            SearchFailure::Save => {
                "state could not be saved; the previous stored copy was not overwritten"
            }
            SearchFailure::Search => "the search could not be completed",
            SearchFailure::Open => {
                "the folder could not be opened; the window stays up for retry or copy"
            }
        }
    }
}

/// The full visible state of the search window, in one struct, so an adapter
/// can render a frame from a single snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewState {
    pub query: String,
    pub rows: Vec<ResultRow>,
    pub selected: Option<usize>,
    /// Busy generation (what is currently computing). `None` when idle.
    pub busy: Option<u64>,
    /// A newer query has superseded an in-flight one; its result is discarded.
    pub stale: bool,
    pub no_result_reason: Option<NoResultReason>,
    pub failure: Option<SearchFailure>,
    pub theme: ResolvedTheme,
    pub locale: super::i18n::Locale,
    /// The filter model has been edited but the search not yet refreshed.
    pub filter_dirty: bool,
    /// A popup/panel is open; while set, hide-on-focus-loss must not hide the
    /// root window.
    pub overlay_open: bool,
    /// IME composition state (see
    /// [`super::view_model::SearchViewModel::set_ime_composition`]).
    pub ime_composition_active: bool,
    /// Hideable keyboard-hint bar; default visible.
    pub keyboard_hint_visible: bool,
}

impl ViewState {
    pub fn has_rows(&self) -> bool {
        !self.rows.is_empty()
    }

    /// Whether Enter/Space should be treated as an open action for the current
    /// selection. While an IME composition/candidate window is active, no
    /// commit key may trigger result actions (M03.3).
    pub fn can_open_selected(&self) -> bool {
        self.selected.is_some_and(|index| index < self.rows.len()) && !self.ime_composition_active
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            query: String::new(),
            rows: Vec::new(),
            selected: None,
            busy: None,
            stale: false,
            no_result_reason: None,
            failure: None,
            theme: super::theme::ResolvedTheme::LIGHT,
            locale: super::i18n::Locale::default(),
            filter_dirty: false,
            overlay_open: false,
            ime_composition_active: false,
            keyboard_hint_visible: true,
        }
    }
}

/// A legal move of the selection highlight inside the current rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMove {
    Up,
    Down,
    First,
    Last,
}

impl SelectionMove {
    /// Apply the move to `current` over `row_count` rows. `None` is a legal
    /// "no selection" state (for example before any result arrives) and gets
    /// wrapped deterministically. Wraps at both edges:
    /// - Up from the first row (or from None) → the last row;
    /// - Down from the last row (or from None) → the first row;
    /// - First/Last always jump to the edges.
    pub fn apply(self, current: Option<usize>, row_count: usize) -> Option<usize> {
        if row_count == 0 {
            return None;
        }
        let current = current.filter(|index| *index < row_count);
        Some(match (self, current) {
            (SelectionMove::First, _) | (SelectionMove::Down, None) => 0,
            (SelectionMove::Last, _) => row_count - 1,
            (SelectionMove::Up, None) => row_count - 1,
            (SelectionMove::Up, Some(0)) => row_count - 1,
            (SelectionMove::Up, Some(index)) => index - 1,
            (SelectionMove::Down, Some(index)) if index + 1 == row_count => 0,
            (SelectionMove::Down, Some(index)) => index + 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionMove;

    #[test]
    fn selection_up_wraps_to_last_row() {
        assert_eq!(SelectionMove::Up.apply(None, 3), Some(2));
        assert_eq!(SelectionMove::Up.apply(Some(0), 3), Some(2));
        assert_eq!(SelectionMove::Up.apply(Some(1), 3), Some(0));
    }

    #[test]
    fn selection_down_wraps_to_first_row() {
        assert_eq!(SelectionMove::Down.apply(None, 3), Some(0));
        assert_eq!(SelectionMove::Down.apply(Some(2), 3), Some(0));
        assert_eq!(SelectionMove::Down.apply(Some(0), 3), Some(1));
    }

    #[test]
    fn selection_first_last_are_explicit() {
        assert_eq!(SelectionMove::First.apply(None, 3), Some(0));
        assert_eq!(SelectionMove::First.apply(Some(2), 3), Some(0));
        assert_eq!(SelectionMove::Last.apply(None, 3), Some(2));
        assert_eq!(SelectionMove::Last.apply(Some(0), 3), Some(2));
    }

    #[test]
    fn selection_on_empty_rows_is_none() {
        assert_eq!(SelectionMove::Up.apply(None, 0), None);
        assert_eq!(SelectionMove::Down.apply(Some(0), 0), None);
    }

    #[test]
    fn selection_clamps_to_current_row_count() {
        // A stale index larger than the current rows is treated as None and
        // then wrapped by the requested move.
        assert_eq!(SelectionMove::Up.apply(Some(9), 3), Some(2));
        assert_eq!(SelectionMove::Down.apply(Some(9), 3), Some(0));
    }
}
