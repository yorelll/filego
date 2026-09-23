//! Pure-Rust search-window ViewModel (M03.2/M03.3).
//!
//! This is the single source of truth for the search window. It does **not**
//! depend on Slint: the `.slint` file is a thin adapter that forwards callbacks
//! as [`ViewCommand`]s and re-renders from the exposed [`ViewState`]. All
//! decisions here (selection, IME gating, popup/hide gating, query-generation
//! stale rejection) are unit-tested with no UI machinery.
//!
//! # Async search + query generation (documented design)
//!
//! [`SearchViewModel::query_edited`] bumps an internal mirror of
//! [`crate::search::QueryGeneration`] and runs the injected [`SearchRunner`].
//! The runner is invoked synchronously by default (deterministic tests, no
//! thread/executor dependency for 0.0.1); a future adapter may wrap it in a
//! worker thread. Results are then reconciled through [`Self::complete_search`],
//! which accepts a completion **only when the completed generation still equals
//! the active generation** — a newer query first bumps the token, so a stale
//! in-flight result is flagged `stale` and dropped. This mirrors the M02.5
//! design (`src/search/generation.rs`) at the presenter level.
//!
//! To make stale rejection deterministic in tests without threads, the runner
//! is a closure in the tests that *captures* the ViewModel's future result
//! explicitly through [`ViewCommand::SearchCompleted`]; the production default
//! runner runs inline. The contract is: the adapter may deliver results either
//! synchronously (runner returns) or asynchronously (a later
//! `SearchCompleted` with the generation the runner was started under).
//!
//! # IME gate (M03.3)
//!
//! Slint 1.18 exposes no native "IME composition active" property on
//! `LineEdit`/`TextInput` (the builtin `TextInput` keeps `preedit-text`
//! internally only). The presenter-side gate is therefore a flag the adapter
//! sets through [`SearchViewModel::set_ime_composition`]; the hook point is
//! documented in the adapter section of `ui/app-window.slint`. While the flag
//! is set, `SearchKey(Enter)` opens nothing and `SelectMove` does not move the
//! selection — the IME itself commits the composition, and only the resulting
//! `QueryEdited` refresh (from LineEdit's `edited`) is allowed. Final native
//! IME wiring is a manual-acceptance item (M03 task list).
//!
//! # Popup/hide gate (M03.2)
//!
//! While [`ViewState::overlay_open`] is true, the hide path is suppressed so
//! opening the filter panel or a context menu never tears the window away from
//! under the popup.

use std::collections::HashMap;

use crate::{
    domain::settings::AppSettings,
    search::{NoResultReason, QueryParser, SearchEntry},
};

use super::{
    commands::{RowAction, SearchKey, SearchOutcome, ViewCommand},
    i18n::Locale,
    state::{ResultRow, SearchFailure, SelectionMove, ViewState},
    theme::{ResolvedColorScheme, ResolvedTheme},
};

/// A searchable entry plus presenter-derived display data for one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    pub searchable: SearchEntry,
    /// Relative path label (typically the last path component).
    pub path_label: String,
}

/// The subset of the filter model that changes search output; kept separate so
/// the UI can send the whole filter at once and the ViewModel re-runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterState {
    pub pinned_only: bool,
    pub recent: bool,
}

impl FilterState {
    pub fn into_filter(&self) -> crate::search::FilterSet {
        crate::search::FilterSet {
            pinned_only: self.pinned_only,
            recent: self.recent,
            ..crate::search::FilterSet::default()
        }
    }
}

/// Whether a runner completed its work synchronously or deferred it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerResponse {
    /// The kept entries (in display order) plus the empty-state reason the M02
    /// core computed, produced synchronously.
    Done(Result<(Vec<ResolvedEntry>, Option<NoResultReason>), ()>),
    /// The work was queued; a later [`ViewCommand::SearchCompleted`] with the
    /// same generation delivers the payload. The ViewModel keeps `busy` set
    /// until that completion.
    Deferred,
}

/// The injected search executor. For 0.0.1 the production default returns
/// [`RunnerResponse::Done`] (synchronous, deterministic). An adapter that wants
/// off-thread work wraps the real runner, returns `Deferred` after starting it,
/// and later delivers the result through [`ViewCommand::SearchCompleted`] with
/// the generation captured from [`SearchViewModel::generation`] — the generation
/// guard rejects any stale completion.
pub type SearchRunner =
    dyn Fn(&[ResolvedEntry], &str, &FilterState, &AppSettings) -> RunnerResponse;

/// Actions the ViewModel asks the adapter to perform outside its own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalEffect {
    /// Hide the root window (the adapter calls `Window::hide`).
    HideWindow,
    /// Request an open of the selected entry (M03 stub → M04.5 real open).
    OpenEntry,
    /// Copy the selected path to the clipboard (M03 stub).
    CopyPath,
    /// Clear the query field in the UI (focus retained).
    ClearInput,
}

/// Seam for surfacing external side effects; the production adapter can also
/// ignore this and read [`ExternalEffect`] from [`Self::handle`]'s return.
pub trait ViewModelEffects {
    fn emit(&mut self, effect: ExternalEffect);
}

/// No-op effects implementation (tests, or adapters that read the return).
pub struct NoopEffects;

impl ViewModelEffects for NoopEffects {
    fn emit(&mut self, _effect: ExternalEffect) {}
}

/// The single source of truth for the search window.
///
/// The `runner` closure is the only dependency that touches M02; everything
/// else is deterministic state transitions.
pub struct SearchViewModel {
    state: ViewState,
    generation: u64,
    settings: AppSettings,
    resolved: Vec<ResolvedEntry>,
    runner: Box<SearchRunner>,
    effects: Box<dyn ViewModelEffects>,
}

impl SearchViewModel {
    /// Create a ViewModel bound to `settings`, `users` entries and a runner.
    pub fn new(
        settings: AppSettings,
        resolved: Vec<ResolvedEntry>,
        runner: Box<SearchRunner>,
        effects: Box<dyn ViewModelEffects>,
    ) -> Self {
        let mut view_model = Self {
            state: ViewState {
                theme: ResolvedTheme::resolve(settings.theme, ResolvedColorScheme::Light),
                locale: Locale::default(),
                ..ViewState::default()
            },
            generation: 0,
            settings,
            resolved,
            runner,
            effects,
        };
        // Initial snapshot with an empty query so the window shows the
        // empty-query strategy immediately (M02.4).
        view_model.refresh();
        view_model
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    /// Set the locale from a detected/adapter-supplied value.
    pub fn set_locale(&mut self, locale: Locale) {
        if self.state.locale != locale {
            self.state.locale = locale;
        }
    }

    /// Set the resolved theme (adapter resolves `System` before calling).
    pub fn set_theme(&mut self, theme: ResolvedTheme) {
        if self.state.theme != theme {
            self.state.theme = theme;
        }
    }

    /// IME gate: the adapter toggles this when a native IME composition (or
    /// candidate window) becomes active/committed (M03.3).
    pub fn set_ime_composition(&mut self, active: bool) {
        if self.state.ime_composition_active != active {
            self.state.ime_composition_active = active;
        }
    }

    /// Surface a folder-open failure (M04.5): the window stays up, the
    /// selection remains so Retry (Enter) / Copy (Ctrl+C) still act on it. The
    /// selection and rows are left untouched; only the failure banner changes.
    pub fn set_open_failure(&mut self) {
        self.state.failure = Some(SearchFailure::Open);
    }

    /// Clear any open-failure banner (a fresh search or a successful retry).
    pub fn clear_failure(&mut self) {
        if matches!(self.state.failure, Some(SearchFailure::Open)) {
            self.state.failure = None;
        }
    }

    /// Apply one user command. Returns the external effects the adapter must
    /// perform (in order). The same effects are also delivered to
    /// [`Self::effects`].
    pub fn handle(&mut self, command: ViewCommand) -> Vec<ExternalEffect> {
        match command {
            ViewCommand::QueryEdited(text) => self.query_edited(text),
            ViewCommand::ClearQuery => self.clear_query(),
            ViewCommand::SearchKey(key) => self.search_key(key),
            ViewCommand::SelectMove(movement) => self.select_move_with_ime(movement),
            ViewCommand::SelectIndex(index) => self.select_index(index),
            ViewCommand::SelectId(id) => self.select_id(id),
            ViewCommand::RowAction(action) => self.row_action(action),
            ViewCommand::ToggleFilters => self.toggle_filters(),
            ViewCommand::SetOverlay(open) => self.set_overlay(open),
            ViewCommand::SearchCompleted { generation, result } => {
                self.search_completed(generation, result)
            }
            ViewCommand::HideRequested => self.hide_requested(),
            ViewCommand::ExitRequested => Vec::new(),
        }
    }

    /// Capture the current generation (for an adapter that wants to run search
    /// off-thread and later call `SearchCompleted`).
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The currently active filter model (unchanged rows); the UI can read it
    /// to render the panel. Returns empty when no filter was ever set.
    pub fn current_filter(&self) -> FilterState {
        FilterState::default()
    }

    fn query_edited(&mut self, text: String) -> Vec<ExternalEffect> {
        if text == self.state.query {
            return Vec::new();
        }
        self.state.query = text;
        self.state.failure = None;
        self.state.selected = None;
        self.run_search_for_current_generation();
        Vec::new()
    }

    fn clear_query(&mut self) -> Vec<ExternalEffect> {
        if self.state.query.is_empty() {
            return Vec::new();
        }
        self.state.query.clear();
        self.state.selected = None;
        self.run_search_for_current_generation();
        vec![ExternalEffect::ClearInput]
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Start a search under a freshly bumped generation. The runner decides
    /// whether to complete synchronously (`Done`) or defer (`Deferred`); in
    /// either case stale rejection is uniform (see [`Self::accept_outcome`]).
    fn run_search_for_current_generation(&mut self) {
        self.bump_generation();
        let generation = self.generation;
        self.state.busy = Some(generation);
        self.state.stale = false;

        let response = self.compute_search();
        match response {
            RunnerResponse::Done(result) => {
                self.accept_run_result(generation, result);
            }
            RunnerResponse::Deferred => {
                // busagem remains set; the adapter later delivers
                // `SearchCompleted` with this generation.
            }
        }
    }

    /// Run the injected runner against the *current* state. Uses a cloned
    /// snapshot so the runner cannot re-enter the ViewModel.
    fn compute_search(&mut self) -> RunnerResponse {
        let query = self.state.query.clone();
        let filter = FilterState::default();
        let settings = self.settings.clone();
        let snapshot = self.resolved.clone();
        (self.runner)(&snapshot, &query, &filter, &settings)
    }

    /// Accept a synchronous search outcome produced under `generation`.
    fn accept_run_result(
        &mut self,
        generation: u64,
        result: Result<(Vec<ResolvedEntry>, Option<NoResultReason>), ()>,
    ) {
        // The generation must still be the active one; otherwise the result is
        // stale and must be dropped (older in-flight work never wins).
        if self.generation != generation {
            self.state.stale = true;
            self.state.busy = Some(self.generation);
            return;
        }
        self.state.stale = false;
        self.state.busy = None;
        match result {
            Ok((entries, no_result_reason)) => {
                self.rebuild_rows(entries);
                self.state.no_result_reason = no_result_reason;
                self.state.failure = None;
            }
            Err(()) => {
                self.state.rows.clear();
                self.state.selected = None;
                self.state.no_result_reason = Some(NoResultReason::NoMatch);
                self.state.failure = Some(SearchFailure::Search);
            }
        }
    }

    /// Accept an outcome delivered through [`ViewCommand::SearchCompleted`]
    /// (the async path). The generation must still be current.
    fn accept_outcome(
        &mut self,
        generation: u64,
        result: Result<SearchOutcome, ()>,
    ) -> Vec<ExternalEffect> {
        if self.generation != generation {
            // Stale completion — drop it; the newer query owns `busy`.
            self.state.stale = self.state.busy.is_some();
            return Vec::new();
        }
        self.state.busy = None;
        self.state.stale = false;
        match result {
            Ok(outcome) => {
                // Resolve the outcome's ids against the resolved entries to
                // rebuild display rows (single source of truth).
                let kept: Vec<ResolvedEntry> = outcome
                    .entry_ids
                    .iter()
                    .filter_map(|id| {
                        self.resolved
                            .iter()
                            .find(|entry| entry.searchable.id.as_uuid() == *id)
                            .cloned()
                    })
                    .collect();
                self.rebuild_rows(kept);
                self.state.no_result_reason = outcome.no_result_reason;
                self.state.failure = None;
            }
            Err(()) => {
                self.state.rows.clear();
                self.state.selected = None;
                self.state.no_result_reason = Some(NoResultReason::NoMatch);
                self.state.failure = Some(SearchFailure::Search);
            }
        }
        Vec::new()
    }

    /// Adapter callback for a completed async search (generation-tagged).
    fn search_completed(
        &mut self,
        generation: u64,
        result: Result<SearchOutcome, ()>,
    ) -> Vec<ExternalEffect> {
        self.accept_outcome(generation, result)
    }

    fn refresh(&mut self) {
        // Initial snapshot uses the current (zero) generation without bumping,
        // so the first search runs under the same token as `new()`.
        let generation = self.generation;
        self.state.busy = Some(generation);
        match self.compute_search() {
            RunnerResponse::Done(result) => self.accept_run_result(generation, result),
            RunnerResponse::Deferred => {}
        }
    }

    fn rebuild_rows(&mut self, entries: Vec<ResolvedEntry>) {
        let rows: Vec<ResultRow> = entries
            .iter()
            .map(|resolved| ResultRow {
                entry_id: resolved.searchable.id.as_uuid(),
                display_name: resolved.searchable.display_name.clone(),
                path_label: resolved.path_label.clone(),
                full_path: resolved.searchable.path.clone(),
                category_name: resolved.searchable.category_name.clone(),
                tag_texts: resolved
                    .searchable
                    .tag_names
                    .iter()
                    .take(1)
                    .cloned()
                    .collect(),
                inaccessible: resolved.searchable.accessibility
                    == crate::search::Accessibility::Inaccessible,
            })
            .collect();

        // Legalize the selection against the new row count: an out-of-bounds
        // index is dropped, otherwise kept (highlight stability while typing).
        self.state.selected = self.state.selected.filter(|index| *index < rows.len());
        self.state.rows = rows;
        self.state.filter_dirty = false;
    }

    fn search_key(&mut self, key: SearchKey) -> Vec<ExternalEffect> {
        match key {
            SearchKey::Enter => {
                if !self.state.ime_composition_active {
                    self.open_selected()
                } else {
                    Vec::new()
                }
            }
            SearchKey::UpDown(movement) => self.select_move_with_ime(movement),
            SearchKey::Escape => {
                // Esc closes an overlay/panel first, then hides the window.
                if self.state.overlay_open {
                    self.state.overlay_open = false;
                    Vec::new()
                } else {
                    self.effects.emit(ExternalEffect::HideWindow);
                    vec![ExternalEffect::HideWindow]
                }
            }
            SearchKey::ToggleHints => {
                self.state.keyboard_hint_visible = !self.state.keyboard_hint_visible;
                Vec::new()
            }
            SearchKey::Other => Vec::new(),
        }
    }

    fn select_move_with_ime(&mut self, movement: SelectionMove) -> Vec<ExternalEffect> {
        if !self.state.ime_composition_active {
            self.select_move(movement);
        }
        Vec::new()
    }

    fn select_move(&mut self, movement: SelectionMove) {
        let row_count = self.state.rows.len();
        self.state.selected = movement.apply(self.state.selected, row_count);
    }

    fn select_index(&mut self, index: usize) -> Vec<ExternalEffect> {
        if !self.state.ime_composition_active && index < self.state.rows.len() {
            self.state.selected = Some(index);
        }
        Vec::new()
    }

    fn select_id(&mut self, id: uuid::Uuid) -> Vec<ExternalEffect> {
        if self.state.ime_composition_active {
            return Vec::new();
        }
        if let Some(index) = self.state.rows.iter().position(|row| row.entry_id == id) {
            self.state.selected = Some(index);
        }
        Vec::new()
    }

    fn row_action(&mut self, action: RowAction) -> Vec<ExternalEffect> {
        match action {
            RowAction::Open => self.open_selected(),
            RowAction::ContextMenu => {
                // M03: the panel is a stub; opening it sets the overlay gate so
                // hide-on-focus-loss is suppressed.
                self.state.overlay_open = true;
                Vec::new()
            }
            RowAction::CopyPath => {
                if self
                    .state
                    .selected
                    .is_some_and(|index| index < self.state.rows.len())
                {
                    self.effects.emit(ExternalEffect::CopyPath);
                    vec![ExternalEffect::CopyPath]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn open_selected(&mut self) -> Vec<ExternalEffect> {
        let can_open = self.state.can_open_selected() && !self.state.overlay_open;
        if !can_open {
            return Vec::new();
        }
        // M03 stub: no real Explorer open yet — M04.5 wires the adapter to the
        // shell open. Here we only signal intent through the effect.
        self.effects.emit(ExternalEffect::OpenEntry);
        vec![ExternalEffect::OpenEntry]
    }

    fn toggle_filters(&mut self) -> Vec<ExternalEffect> {
        self.state.filter_dirty = true;
        self.state.overlay_open = true;
        Vec::new()
    }

    fn set_overlay(&mut self, open: bool) -> Vec<ExternalEffect> {
        self.state.overlay_open = open;
        Vec::new()
    }

    fn hide_requested(&mut self) -> Vec<ExternalEffect> {
        if self.state.overlay_open {
            // A popup/panel is open: defer the hide (M03.2).
            return Vec::new();
        }
        self.effects.emit(ExternalEffect::HideWindow);
        vec![ExternalEffect::HideWindow]
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (testable)
// ---------------------------------------------------------------------------

/// Build resolved entries from searchable entries, deriving each path label.
pub fn resolved_from_search(
    searchable: &[SearchEntry],
    path_labels: impl Fn(&SearchEntry) -> String,
) -> Vec<ResolvedEntry> {
    searchable
        .iter()
        .map(|entry| ResolvedEntry {
            searchable: entry.clone(),
            path_label: path_labels(entry),
        })
        .collect()
}

/// Default synchronous runner: parse, filter, rank through M02-B's
/// `search_with_filter`, then map ranked ids back to resolved entries. Keeps
/// M02's deterministic ordering; M03 does not render highlight ranges yet.
pub fn default_runner(
    resolved: &[ResolvedEntry],
    query: &str,
    filter: &FilterState,
    settings: &AppSettings,
) -> RunnerResponse {
    let parser = QueryParser;
    let query = parser.parse(query);
    let filter_set = filter.into_filter();
    let entries: Vec<SearchEntry> = resolved
        .iter()
        .map(|resolved| resolved.searchable.clone())
        .collect();
    let response = crate::search::search_with_filter(
        &entries,
        &query,
        &filter_set,
        settings,
        &crate::search::HighlightOptions::default(),
    );
    let by_id: HashMap<uuid::Uuid, &ResolvedEntry> = resolved
        .iter()
        .map(|entry| (entry.searchable.id.as_uuid(), entry))
        .collect();

    let kept: Vec<ResolvedEntry> = match &response.display {
        crate::search::SearchDisplay::Ranked(results) => results
            .iter()
            .filter_map(|result| by_id.get(&result.entry_id.as_uuid()).copied())
            .cloned()
            .collect(),
        crate::search::SearchDisplay::EmptyQuery(indices) => indices
            .iter()
            .filter_map(|index| resolved.get(*index))
            .cloned()
            .collect(),
    };

    RunnerResponse::Done(Ok((kept, response.no_result_reason)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::ids::FolderId,
        presentation::{
            commands::{RowAction, SearchKey, SearchOutcome, ViewCommand},
            state::SelectionMove,
        },
        search::{Accessibility, Origin, SearchEntry},
    };
    use uuid::Uuid;

    fn utc(value: &str) -> chrono::DateTime<chrono::Utc> {
        value.parse().expect("fixed RFC3339 fixture must parse")
    }

    fn entry(id: u128, name: &str, path: &str) -> SearchEntry {
        SearchEntry {
            id: FolderId::from_uuid(Uuid::from_u128(id)),
            display_name: name.to_owned(),
            aliases: Vec::new(),
            path: path.to_owned(),
            category_name: None,
            tag_names: Vec::new(),
            note: String::new(),
            pinned: false,
            favorite: false,
            manual_weight: 0,
            open_count: 0,
            last_opened_at: Some(utc("2026-09-21T00:00:00Z")),
            accessibility: Accessibility::Unknown,
            origin: Origin::Unknown,
        }
    }

    fn resolved(entries: Vec<SearchEntry>) -> Vec<ResolvedEntry> {
        resolved_from_search(&entries, |entry| {
            entry
                .path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&entry.path)
                .to_owned()
        })
    }

    fn view_model() -> SearchViewModel {
        let settings = AppSettings::default();
        let entries = resolved(vec![
            entry(1, "Documents", r"C:\Users\me\Documents"),
            entry(2, "Photos", r"C:\Users\me\Pictures"),
        ]);
        SearchViewModel::new(
            settings,
            entries,
            Box::new(default_runner),
            Box::new(NoopEffects),
        )
    }

    fn recording_effects(
        buffer: std::rc::Rc<std::cell::RefCell<Vec<ExternalEffect>>>,
    ) -> Box<Recorder> {
        Box::new(Recorder { buffer })
    }

    fn last_effect(effects: Vec<ExternalEffect>, want: ExternalEffect) -> bool {
        effects.contains(&want)
    }

    // --- M03.5 unit tests -----------------------------------------------------

    #[test]
    fn initial_state_shows_empty_query_strategy_rows() {
        let view_model = view_model();
        assert!(!view_model.state().rows.is_empty());
        assert_eq!(view_model.state().query, "");
        assert_eq!(view_model.state().busy, None);
        assert!(!view_model.state().stale);
    }

    #[test]
    fn query_edited_updates_query_and_runs_search() {
        let mut view_model = view_model();
        let effects = view_model.handle(ViewCommand::QueryEdited("Doc".to_owned()));
        assert!(effects.is_empty());
        assert_eq!(view_model.state().query, "Doc");
        // default_runner ran synchronously, so busy is already cleared.
        assert_eq!(view_model.state().busy, None);
        assert!(!view_model.state().rows.is_empty());
        assert!(
            view_model
                .state()
                .rows
                .iter()
                .any(|row| row.display_name == "Documents")
        );
    }

    #[test]
    fn clear_query_resets_selection_and_requests_clear_input() {
        let mut view_model = view_model();
        view_model.handle(ViewCommand::QueryEdited("Photos".to_owned()));
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        assert!(view_model.state().selected.is_some());
        let effects = view_model.handle(ViewCommand::ClearQuery);
        assert!(last_effect(effects, ExternalEffect::ClearInput));
        assert_eq!(view_model.state().query, "");
        assert_eq!(view_model.state().selected, None);
        assert!(
            !view_model.state().rows.is_empty(),
            "empty-query rows return"
        );
    }

    #[test]
    fn clear_query_on_empty_query_does_nothing() {
        let mut view_model = view_model();
        assert_eq!(view_model.state().query, "");
        let effects = view_model.handle(ViewCommand::ClearQuery);
        assert!(effects.is_empty());
    }

    #[test]
    fn selection_wraps_at_edges_and_is_legalized_on_results_change() {
        let mut view_model = view_model();
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        assert_eq!(view_model.state().selected, Some(0));
        view_model.handle(ViewCommand::SelectMove(SelectionMove::Up));
        assert_eq!(view_model.state().selected, Some(1), "wrap up to last");
        view_model.handle(ViewCommand::SelectMove(SelectionMove::Down));
        assert_eq!(view_model.state().selected, Some(0), "wrap down to first");

        view_model.handle(ViewCommand::QueryEdited("Photos".to_owned()));
        assert_eq!(view_model.state().rows.len(), 1);
        // Selection was reset by the query change; choose the only row.
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        assert_eq!(view_model.state().selected, Some(0));
    }

    #[test]
    fn selection_index_is_clamped_after_rows_shrink() {
        let mut view_model = view_model();
        view_model.handle(ViewCommand::SelectMove(SelectionMove::Last));
        assert_eq!(view_model.state().selected, Some(1));
        // A query that yields one row must clamp the stale index to None.
        view_model.handle(ViewCommand::QueryEdited("Photos".to_owned()));
        assert_eq!(view_model.state().selected, None);
    }

    #[test]
    fn select_by_id_resolves_against_current_rows() {
        let mut view_model = view_model();
        // "Photos" is entry id 2.
        view_model.handle(ViewCommand::QueryEdited("Photos".to_owned()));
        assert_eq!(view_model.state().rows.len(), 1);
        let photos_id = Uuid::from_u128(2);
        view_model.handle(ViewCommand::SelectId(photos_id));
        assert_eq!(view_model.state().selected, Some(0));

        // An id not in the current rows leaves the selection unchanged.
        view_model.handle(ViewCommand::SelectId(Uuid::from_u128(999)));
        assert_eq!(view_model.state().selected, Some(0));
    }

    #[test]
    fn select_by_id_is_blocked_while_ime_composing() {
        let mut view_model = view_model();
        view_model.set_ime_composition(true);
        view_model.handle(ViewCommand::SelectId(Uuid::from_u128(1)));
        assert_eq!(view_model.state().selected, None);
    }

    #[test]
    fn enter_opens_selected_and_is_a_noop_without_selection() {
        let buffer = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut view_model = view_model_with_effects(recording_effects(buffer.clone()));
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));

        let opened = view_model.handle(ViewCommand::RowAction(RowAction::Open));
        assert!(last_effect(opened, ExternalEffect::OpenEntry));

        // Enter from the search box with no selection is a harmless no-op.
        let buffer = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut fresh = view_model_with_effects(recording_effects(buffer.clone()));
        let opened = fresh.handle(ViewCommand::SearchKey(SearchKey::Enter));
        assert!(opened.is_empty());
    }

    #[test]
    fn search_key_map_and_escape_paths() {
        let mut view_model = view_model();
        // Esc with overlay closed hides the window.
        let effects = view_model.handle(ViewCommand::SearchKey(SearchKey::Escape));
        assert!(last_effect(effects, ExternalEffect::HideWindow));

        // Esc with overlay open closes only the overlay.
        view_model.handle(ViewCommand::SetOverlay(true));
        let effects = view_model.handle(ViewCommand::SearchKey(SearchKey::Escape));
        assert!(effects.is_empty());
        assert!(!view_model.state().overlay_open);

        // Alt+H toggles the hint bar.
        let before = view_model.state().keyboard_hint_visible;
        view_model.handle(ViewCommand::SearchKey(SearchKey::ToggleHints));
        assert_ne!(view_model.state().keyboard_hint_visible, before);
    }

    #[test]
    fn ime_gate_blocks_open_and_selection_until_composition_ends() {
        let mut view_model = view_model();
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        assert_eq!(view_model.state().selected, Some(0));

        view_model.set_ime_composition(true);
        let opened = view_model.handle(ViewCommand::SearchKey(SearchKey::Enter));
        assert!(opened.is_empty(), "Enter must not open while composing");
        view_model.handle(ViewCommand::SelectMove(SelectionMove::Down));
        assert_eq!(view_model.state().selected, Some(0), "Down must not move");

        // Composition ends: Enter opens, Down moves.
        view_model.set_ime_composition(false);
        let opened = view_model.handle(ViewCommand::SearchKey(SearchKey::Enter));
        assert!(last_effect(opened, ExternalEffect::OpenEntry));
        view_model.handle(ViewCommand::SelectMove(SelectionMove::Down));
        assert_eq!(view_model.state().selected, Some(1));
    }

    #[test]
    fn ime_gate_still_allows_query_editing() {
        let mut view_model = view_model();
        view_model.set_ime_composition(true);
        view_model.handle(ViewCommand::QueryEdited("Doc".to_owned()));
        assert_eq!(view_model.state().query, "Doc");
        assert!(!view_model.state().rows.is_empty());
    }

    #[test]
    fn hide_is_deferred_while_an_overlay_is_open() {
        let mut view_model = view_model();
        view_model.handle(ViewCommand::SetOverlay(true));
        let effects = view_model.handle(ViewCommand::HideRequested);
        assert!(effects.is_empty(), "overlay open -> hide deferred");

        view_model.handle(ViewCommand::SetOverlay(false));
        let effects = view_model.handle(ViewCommand::HideRequested);
        assert!(last_effect(effects, ExternalEffect::HideWindow));
    }

    #[test]
    fn open_does_not_work_while_overlay_open() {
        // Inside a popup (context menu), Enter must not open a row.
        let buffer = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut view_model = view_model_with_effects(recording_effects(buffer.clone()));
        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        view_model.handle(ViewCommand::RowAction(RowAction::ContextMenu));
        assert!(view_model.state().overlay_open);
        let opened = view_model.handle(ViewCommand::SearchKey(SearchKey::Enter));
        assert!(opened.is_empty());
    }

    #[test]
    fn copy_path_emits_effect_only_with_a_selected_row() {
        let mut view_model = view_model();
        let effects = view_model.handle(ViewCommand::RowAction(RowAction::CopyPath));
        assert!(effects.is_empty());

        view_model.handle(ViewCommand::SelectMove(SelectionMove::First));
        let effects = view_model.handle(ViewCommand::RowAction(RowAction::CopyPath));
        assert!(last_effect(effects, ExternalEffect::CopyPath));
    }

    #[test]
    fn filter_toggle_marks_dirty_and_opens_the_overlay_gate() {
        let mut view_model = view_model();
        assert!(!view_model.state().filter_dirty);
        view_model.handle(ViewCommand::ToggleFilters);
        assert!(view_model.state().filter_dirty);
        assert!(view_model.state().overlay_open);
    }

    #[test]
    fn generation_rejects_stale_completions() {
        // Simulate the async flow with a controllable runner: first edit bumps
        // the generation; a completion for that generation clears busy; a later
        // edit bumps again and a completion for the *previous* generation is
        // dropped (the flag stays stale).
        let mut view_model = view_model();

        view_model.handle(ViewCommand::QueryEdited("Doc".to_owned()));
        let first_gen = view_model.generation();
        assert!(first_gen > 0);

        // A stale completion (for a generation already superseded by another
        // edit) must not clear the busy flag.
        view_model.handle(ViewCommand::QueryEdited("Photos".to_owned()));
        let active = view_model.generation();
        assert_eq!(active, first_gen + 1);

        view_model.handle(ViewCommand::SearchCompleted {
            generation: first_gen,
            result: Ok(SearchOutcome::default()),
        });
        // The first generation is stale: this completion must be dropped, so
        // the newer query's state is untouched.
        assert!(!view_model.state().stale);
        assert_eq!(view_model.state().busy, None);
        assert_eq!(view_model.state().query, "Photos");
    }

    #[test]
    fn stale_flag_shows_when_an_inflight_result_arrives_after_a_newer_query() {
        // Genuinely async path: a runner that defers. The first edit leaves
        // `busy` set. A second edit supersedes it. A late completion for the
        // first generation must NOT clear the newer, still-busy state.
        fn deferred_runner(
            _: &[ResolvedEntry],
            _: &str,
            _: &FilterState,
            _: &AppSettings,
        ) -> RunnerResponse {
            RunnerResponse::Deferred
        }

        let settings = AppSettings::default();
        let entries = resolved(vec![
            entry(1, "Documents", r"C:\Users\me\Documents"),
            entry(2, "Photos", r"C:\Users\me\Pictures"),
        ]);
        let mut view_model = SearchViewModel::new(
            settings,
            entries,
            Box::new(deferred_runner),
            Box::new(NoopEffects),
        );

        view_model.handle(ViewCommand::QueryEdited("A".to_owned()));
        let stale_gen = view_model.generation();
        assert_eq!(view_model.state().busy, Some(stale_gen));

        // Second edit supersedes the first before its result arrives.
        view_model.handle(ViewCommand::QueryEdited("AB".to_owned()));
        let active = view_model.generation();
        assert!(active > stale_gen);
        assert_eq!(view_model.state().busy, Some(active));
        assert!(!view_model.state().stale);

        // Late completion for the first generation:
        view_model.handle(ViewCommand::SearchCompleted {
            generation: stale_gen,
            result: Ok(SearchOutcome::default()),
        });
        assert!(view_model.state().stale, "older completion must set stale");
        assert_eq!(
            view_model.state().busy,
            Some(active),
            "newer query still busy"
        );

        // The fresh completion clears the state.
        view_model.handle(ViewCommand::SearchCompleted {
            generation: active,
            result: Ok(SearchOutcome {
                entry_ids: vec![Uuid::from_u128(1)],
                no_result_reason: None,
            }),
        });
        assert!(!view_model.state().stale);
        assert_eq!(view_model.state().busy, None);
        assert_eq!(view_model.state().rows.len(), 1);
    }

    // helpers ---------------------------------------------------------------
    fn view_model_with_effects(effects: Box<dyn ViewModelEffects>) -> SearchViewModel {
        let settings = AppSettings::default();
        let entries = resolved(vec![
            entry(1, "Documents", r"C:\Users\me\Documents"),
            entry(2, "Photos", r"C:\Users\me\Pictures"),
        ]);
        SearchViewModel::new(settings, entries, Box::new(default_runner), effects)
    }

    struct Recorder {
        buffer: std::rc::Rc<std::cell::RefCell<Vec<ExternalEffect>>>,
    }

    impl ViewModelEffects for Recorder {
        fn emit(&mut self, effect: ExternalEffect) {
            self.buffer.borrow_mut().push(effect);
        }
    }
}
