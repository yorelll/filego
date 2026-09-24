//! Typed user commands for the search-window ViewModel (M03.2).
//!
//! The Slint UI is intentionally dumb: it forwards gestures/events to Rust via
//! callbacks, and this enum is the adapter's vocabulary. The ViewModel applies
//! these commands against its own state and exposes a fresh [`super::state::ViewState`];
//! the UI never mutates state directly.

use super::state::SelectionMove;

/// A search outcome carried by [`ViewCommand::SearchCompleted`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchOutcome {
    /// Entry ids of the kept rows, in display order.
    pub entry_ids: Vec<uuid::Uuid>,
    /// Empty-state reason computed by the search core.
    pub no_result_reason: Option<crate::search::NoResultReason>,
}

/// A key the user pressed while typing in the search box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKey {
    /// Enter / Return.
    Enter,
    /// Up / Down (with optional shift, which only matters for the underlying
    /// native selection the LineEdit already handles; we ignore it here).
    UpDown(SelectionMove),
    /// Escape while typing.
    Escape,
    /// Alt+H — toggle the keyboard-hint bar.
    ToggleHints,
    /// Any other text/control key (for example Ctrl+A, Ctrl+C, Ctrl+Backspace)
    /// that must keep the built-in text behavior.
    Other,
}

/// A click/context-menu action anchored to a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// Left click or Enter on the row: request an open of the selected entry.
    Open,
    /// Right-click: show the context menu for the row.
    ContextMenu,
    /// Copy the full path to the clipboard (M03/M04 stub: records the command).
    CopyPath,
    /// Copy the display name to the clipboard (M05.5).
    CopyName,
    /// Open the record in the settings add/edit dialog (M05.5).
    Edit,
    /// Toggle the pinned flag (M05.5).
    TogglePin,
    /// Disable/restore the record (M05.5).
    ToggleEnable,
    /// Remove the record from FileGo (never the real folder) (M05.5).
    RemoveFromList,
}

impl RowAction {
    /// Map a numeric context-menu action code (as the Slint `ContextMenuPopup`
    /// forwards it) onto a [`RowAction`].
    ///
    /// M05 review OBS-01 closed (M07.1): these codes mirror the `MenuRow`
    /// order in `ui/app-window.slint` (0 = Open, 1 = CopyPath, 2 = CopyName,
    /// 3 = Edit, 4 = TogglePin, 5 = ToggleEnable, anything else = Remove) and
    /// live here as named/typed state so a reorder on either side is caught at
    /// compile time rather than by a silent mis-dispatch. The ordered variants
    /// also assert the mirror stays correct.
    pub fn from_context_action(action: i32) -> Self {
        match action {
            0 => RowAction::Open,
            1 => RowAction::CopyPath,
            2 => RowAction::CopyName,
            3 => RowAction::Edit,
            4 => RowAction::TogglePin,
            5 => RowAction::ToggleEnable,
            _ => RowAction::RemoveFromList,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_action_codes_map_to_the_slint_menu_order() {
        // OBS-01: the numeric action codes the Slint ContextMenuPopup forwards
        // must map one-to-one onto the typed actions, in order.
        assert_eq!(RowAction::from_context_action(0), RowAction::Open);
        assert_eq!(RowAction::from_context_action(1), RowAction::CopyPath);
        assert_eq!(RowAction::from_context_action(2), RowAction::CopyName);
        assert_eq!(RowAction::from_context_action(3), RowAction::Edit);
        assert_eq!(RowAction::from_context_action(4), RowAction::TogglePin);
        assert_eq!(RowAction::from_context_action(5), RowAction::ToggleEnable);
        assert_eq!(RowAction::from_context_action(6), RowAction::RemoveFromList);
        assert_eq!(
            RowAction::from_context_action(999),
            RowAction::RemoveFromList
        );
    }
}

/// Every user gesture the ViewModel understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewCommand {
    /// The user typed into the search box. `text` is the raw current query.
    QueryEdited(String),
    /// A key was pressed while the search box had focus.
    SearchKey(SearchKey),
    /// The user clicked the clear button.
    ClearQuery,
    /// Move the selection (arrow keys or scroll zoom on the list focus).
    SelectMove(SelectionMove),
    /// Select an explicit row index (usually from a click).
    SelectIndex(usize),
    /// Select the entry with this id (from an overlay/panel callback).
    SelectId(uuid::Uuid),
    /// A row-level action (open / context menu / copy path).
    RowAction(RowAction),
    /// Toggle filter dirty flag / open the filter panel.
    ToggleFilters,
    /// Open/close the filter panel or context menu.
    SetOverlay(bool),
    /// Result of an async search, delivered for the generation it ran under.
    /// The ViewModel ignores a completion whose generation is no longer
    /// current (M02.5 stale rejection).
    SearchCompleted {
        generation: u64,
        result: Result<SearchOutcome, ()>,
    },
    /// Hide the window (Esc path). Suppressed while an overlay is open.
    HideRequested,
    /// The user wants to quit the whole app (wired to tray in M04).
    ExitRequested,
}
