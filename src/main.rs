#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{cell::RefCell, rc::Rc};

use filego::{
    AppTray, AppWindow, SettingsWindow, UiStrings, UiTheme,
    app::{LifecycleCommand, LifecycleController, WindowPort},
    domain::settings::AppSettings,
    presentation::{
        commands::{RowAction, SearchKey, ViewCommand},
        i18n::Msg,
        manager::{MCommand, ManagementController, ManagementStore},
        state::SelectionMove,
        view_model::{ExternalEffect, NoopEffects, ResolvedEntry, SearchViewModel, default_runner},
    },
    version,
};
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, VecModel};

struct SlintWindowPort {
    window: slint::Weak<AppWindow>,
}

impl SlintWindowPort {
    fn app(&self) -> Result<AppWindow, slint::PlatformError> {
        self.window
            .upgrade()
            .ok_or_else(|| slint::PlatformError::from("application window is no longer available"))
    }
}

impl WindowPort for SlintWindowPort {
    type Error = slint::PlatformError;

    fn show(&mut self) -> Result<(), Self::Error> {
        let app = self.app()?;
        place_window(&app);
        app.show()?;
        bring_search_to_front(&app);
        Ok(())
    }

    fn hide(&mut self) -> Result<(), Self::Error> {
        self.app()?.hide()
    }

    fn quit(&mut self) -> Result<(), Self::Error> {
        slint::quit_event_loop().map_err(event_loop_error_as_platform_error)
    }
}

/// Compute and apply the M04.4 window placement (cursor monitor, centered,
/// top ≈ 10%, clamped to the work area) before showing.
///
/// F003: the physical size is derived from the TARGET (cursor) monitor's DPI
/// scale — never the window's current `scale_factor()`, which may describe a
/// different monitor in a mixed-DPI layout. `placement_rect_for_cursor` queries
/// the cursor position, the target monitor's work area, and its `GetDpiForMonitor`
/// scale, then computes the physical rect. Re-computed on every show so monitor
/// count/DPI changes are picked up deterministically.
fn place_window(app: &AppWindow) {
    let window = app.window();
    // Slint's `window.size()` is already physical for the window's own monitor;
    // converting to logical and re-scaling by the TARGET monitor's DPI is what
    // F003 fixes (the old code scaled the logical size by the window's current
    // scale, mis-sizing on a different-DPI target monitor).
    let logical = window.size().to_logical(window.scale_factor());
    let (logical_w, logical_h) = if logical.width > 0.0 && logical.height > 0.0 {
        (logical.width, logical.height)
    } else {
        // Default first-show size (600x140 logical).
        (600.0, 140.0)
    };
    let rect = filego::platform::windows::window_placement::placement_rect_for_cursor(
        logical_w, logical_h,
    );
    // Apply through Slint's public physical-position API.
    window.set_position(slint::WindowPosition::Physical(
        slint::PhysicalPosition::new(rect.x, rect.y),
    ));
}

/// Bring the search window to the foreground after showing (M04.4).
///
/// Uses the raw-window-handle 0.6 accessor Slint exposes; falls back to a no-op
/// (the window is still visible) when the native handle is not yet available.
fn bring_search_to_front(app: &AppWindow) {
    use raw_window_handle::HasWindowHandle as _;
    let window = app.window();
    // Bind the winit window handle so its owned data outlives the
    // `HasWindowHandle` borrow (E0716: the accessor returns a handle borrowing
    // from this temporary).
    let winit_window = window.window_handle();
    let Ok(handle) = winit_window.window_handle() else {
        return;
    };
    let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd_bits = win32.hwnd.get();
    let _ = filego::platform::windows::window_focus::bring_to_front_bits(hwnd_bits, true);
}

fn event_loop_error_as_platform_error(error: slint::EventLoopError) -> slint::PlatformError {
    slint::PlatformError::from(error.to_string())
}

/// Result of an async shell open, posted back to the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenResult {
    Succeeded,
    Failed(filego::platform::shell_open::OpenErrorKind),
}

/// The shell-open function signature the presenter invokes for M04.5.
type OpenFolderFn =
    dyn Fn(&str) -> Result<(), filego::platform::shell_open::OpenErrorKind> + Send + Sync;

/// The `&self`-able, thread-safe shell opener the presenter uses (initially
/// backed by `windows::tray_open::open_folder`). The open runs on a worker
/// thread, so the handle must be `Send + Sync` (Arc, not Rc).
#[derive(Clone)]
struct ShellHandle(std::sync::Arc<OpenFolderFn>);

impl ShellHandle {
    fn open(&self, path: &str) -> Result<(), filego::platform::shell_open::OpenErrorKind> {
        (self.0)(path)
    }

    #[cfg(windows)]
    fn real() -> Self {
        ShellHandle(std::sync::Arc::new(|path| {
            filego::platform::windows::tray_open::open_folder(path)
        }))
    }
}

/// The 0.0.1 ViewModel-driven adapter for the M03 search window.
///
/// M05 moves search onto the real repository-backed data path: the adapter is
/// handed the resolved entries (built by `run()` from the shared repository)
/// instead of demo data. The management side (settings window) shares the same
/// repository. Context-menu actions (M05.5) are routed to the settings window
/// through `context_sender`.
struct MainWindowController {
    view_model: SearchViewModel,
    window: slint::Weak<AppWindow>,
    /// Receiver for async open results (the shell open runs off the UI thread;
    /// a UI-thread timer drains this and applies the result).
    open_results: std::sync::mpsc::Receiver<OpenResult>,
    /// Sender handed to spawned open-threads (kept alive for the controller's
    /// life; the receiver lives in this struct).
    open_sender: std::sync::mpsc::Sender<OpenResult>,
    /// Settings snapshot for hide-after-open / clear-after-open (M04.5).
    settings: AppSettings,
    /// The shell opener used for M04.5 (injected for tests to observe it runs
    /// only `open_folder`).
    shell: ShellHandle,
    /// Sender for context-menu effects (M05.5) drained by the settings window
    /// controller on a UI-thread timer.
    context_sender: std::sync::mpsc::Sender<ContextEffect>,
    /// The full set of resolved entries (rebuilt from the repository after a
    /// management change; pushed into the ViewModel).
    resolved: Vec<ResolvedEntry>,
}

impl MainWindowController {
    fn new(
        window: slint::Weak<AppWindow>,
        open_results: std::sync::mpsc::Receiver<OpenResult>,
        open_sender: std::sync::mpsc::Sender<OpenResult>,
        shell: ShellHandle,
        context_sender: std::sync::mpsc::Sender<ContextEffect>,
        resolved: Vec<ResolvedEntry>,
        settings: AppSettings,
    ) -> Self {
        let view_model = SearchViewModel::new(
            settings.clone(),
            resolved.clone(),
            Box::new(default_runner),
            Box::new(NoopEffects),
        );
        Self {
            view_model,
            window: window.clone(),
            open_results,
            open_sender,
            settings,
            shell,
            context_sender,
            resolved,
        }
    }

    /// Rebuild the resolved entries from the shared repository and refresh the
    /// search window (called after a settings-window change).
    fn refresh_from_repository(
        &mut self,
        repo: &std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>>,
    ) {
        self.resolved = resolved_from_repository(repo);
        let entries = self.resolved.clone();
        self.view_model.set_resolved_entries(entries);
        self.sync_ui();
    }

    /// Drain any pending open results (called by the UI-thread timer).
    fn drain_open_results(&mut self) {
        while let Ok(result) = self.open_results.try_recv() {
            self.apply_open_result(result);
        }
    }

    fn handle(&mut self, command: ViewCommand) {
        let effects = self.view_model.handle(command);
        self.sync_ui();
        for effect in effects {
            self.apply_effect(effect);
        }
    }

    fn apply_effect(&mut self, effect: ExternalEffect) {
        match effect {
            ExternalEffect::HideWindow => {
                // The lifecycle controller already routes hide through the
                // window port; here we hide directly because this is the
                // presenter's own hide (Esc etc.). State stays consistent with
                // the LifecycleController via the on_close_requested handler.
                if let Some(window) = self.window.upgrade() {
                    let _ = window.hide();
                }
            }
            // M04.5: open the selected entry via the shell verb. The shell call
            // is async off the UI thread; the result is posted to a channel and
            // drained by a repeating UI-thread polling timer (see `open_drain`)
            // into `apply_open_result`.
            ExternalEffect::OpenEntry => self.open_selected(),
            // Copy the full selection path to the system clipboard. M04 keeps
            // the ViewModel's effect; the wiring is clipboard via Slint's
            // `Slint.Clipboard` text copying through platform. Slint 1.18 does
            // not expose clipboard on `Window`, so we surface the selected
            // path into the OS clipboard with a UTF-16 write in the clipboard
            // adapter (part of M04 native wiring, but only best-effort).
            ExternalEffect::CopyPath => self.copy_selected_path(),
            // M05.5: copy the selected display name to the clipboard.
            ExternalEffect::CopyName => {
                if let Some(name) = self.selected_display_name() {
                    filego::platform::windows::clipboard::set_text(&name);
                }
            }
            // M05.5: open the selected record in the settings edit dialog. The
            // adapter routes this to the settings window controller through a
            // channel (see `SettingsBridge`).
            ExternalEffect::EditSelected => self.request_context_action(ContextAction::Edit),
            ExternalEffect::TogglePinSelected => {
                self.request_context_action(ContextAction::TogglePin)
            }
            ExternalEffect::ToggleEnableSelected => {
                self.request_context_action(ContextAction::ToggleEnable)
            }
            ExternalEffect::RemoveSelectedFromList => {
                self.request_context_action(ContextAction::Remove)
            }
            ExternalEffect::ClearInput => {
                // The LineEdit already clears via state-query; keep focus on it.
            }
        }
    }

    fn selected_display_name(&self) -> Option<String> {
        let state = self.view_model.state();
        let index = state.selected?;
        state.rows.get(index).map(|row| row.display_name.clone())
    }

    /// Route a context-menu action for the selected row (M05.5). The settings
    /// window owns the repository; the action is forwarded through a channel
    /// drained by the settings timer. The overlay stays open until the adapter
    /// closes it (popup-safe).
    fn request_context_action(&mut self, action: ContextAction) {
        let state = self.view_model.state();
        let Some(index) = state.selected else {
            return;
        };
        let Some(row) = state.rows.get(index) else {
            return;
        };
        let effect = ContextEffect::from_row(action, row.entry_id);
        self.context_sender.send(effect).ok();
    }

    fn selected_full_path(&self) -> Option<String> {
        let state = self.view_model.state();
        let index = state.selected?;
        state.rows.get(index).map(|row| row.full_path.clone())
    }

    /// Kick off an async shell open of the selected row (M04.5).
    ///
    /// The shell call runs on a worker thread via `shell_open::open_async` and
    /// posts [`OpenResult`] to `self.open_results`. A UI-thread timer drains the
    /// channel and calls [`Self::apply_open_result`].
    fn open_selected(&mut self) {
        let Some(path) = self.selected_full_path() else {
            return;
        };
        // Clear a previous open-failure banner so a retry is visible.
        self.view_model.clear_failure();
        self.sync_ui();
        let opener = self.shell.clone();
        let results = self.open_sender.clone();
        std::thread::spawn(move || {
            let result = opener.open(&path);
            let _ = results.send(match result {
                Ok(()) => OpenResult::Succeeded,
                Err(kind) => OpenResult::Failed(kind),
            });
        });
    }

    /// Copy the selected entry's full path to the clipboard (M04 native best
    /// effort; Slint 1.18 has no public clipboard API, so we use the Windows
    /// clipboard through the adapter).
    fn copy_selected_path(&self) {
        let Some(path) = self.selected_full_path() else {
            return;
        };
        filego::platform::windows::clipboard::set_text(&path);
    }

    /// Apply an async open result on the UI thread.
    fn apply_open_result(&mut self, result: OpenResult) {
        match result {
            OpenResult::Succeeded => {
                self.view_model.clear_failure();
                // hide/clear per settings (M04.5).
                if self.settings.hide_after_open
                    && let Some(window) = self.window.upgrade()
                {
                    let _ = window.hide();
                }
                if self.settings.clear_after_open {
                    self.view_model.handle(ViewCommand::ClearQuery);
                }
                self.sync_ui();
            }
            OpenResult::Failed(kind) => {
                // Keep the window; surface an anonymous failure (kind only, no
                // path); the selection stays so Retry (Enter) / Copy (Ctrl+C)
                // keep working.
                self.view_model.set_open_failure(kind);
                self.sync_ui();
            }
        }
    }

    /// Push the full ViewState snapshot into the Slint properties.
    fn sync_ui(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let state = self.view_model.state();

        window.set_state_query(state.query.clone().into());
        window.set_state_selected(state.selected.map_or(-1, |index| index as i32));
        window.set_state_busy(state.busy.is_some());
        window.set_state_stale(state.stale);
        window.set_hints_visible(state.keyboard_hint_visible);
        window.set_ime_composing(state.ime_composition_active);

        let rows = state.rows.clone();
        let row_count = rows.len();
        window.set_rows_count(row_count as i32);
        window.set_rows_name(string_model(
            rows.iter().map(|row| row.display_name.clone()),
        ));
        window.set_rows_path(string_model(rows.iter().map(|row| row.path_label.clone())));
        // F002: the full path is carried on each row for the tooltip / Copy-Path
        // only; the visible row line-2 keeps the short label.
        window.set_rows_full_path(string_model(rows.iter().map(|row| row.full_path.clone())));
        // F001: re-derive the results-count label from the actual rows on every
        // push (never a constant), so the rendered count matches the results.
        window
            .global::<UiStrings>()
            .set_count(results_count_label(&rows, state.locale));
        window.set_rows_category(string_model(
            rows.iter()
                .map(|row| row.category_name.clone().unwrap_or_default()),
        ));
        window.set_rows_tags(string_model(
            rows.iter().flat_map(|row| row.tag_texts.clone()),
        ));
        window.set_rows_inaccessible(ModelRc::new(VecModel::from(
            rows.iter()
                .map(|row| row.inaccessible)
                .collect::<Vec<bool>>(),
        )));

        // Empty/error state: map NoResultReason to the i18n title + body.
        let show_empty = state.no_result_reason.is_some() || state.failure.is_some();
        let (title, body) = match state.failure {
            // F006: pick the per-kind localized body (still anonymous — the
            // kind never carries a path); a kind not in the table falls back to
            // the generic open body.
            Some(filego::presentation::state::SearchFailure::Open(kind)) => {
                let body = match kind {
                    filego::platform::shell_open::OpenErrorKind::NotFound => {
                        filego::presentation::i18n::Msg::ErrorOpenNotFound
                    }
                    filego::platform::shell_open::OpenErrorKind::AccessDenied => {
                        filego::presentation::i18n::Msg::ErrorOpenAccessDenied
                    }
                    filego::platform::shell_open::OpenErrorKind::NoAssociation => {
                        filego::presentation::i18n::Msg::ErrorOpenNoAssociation
                    }
                    filego::platform::shell_open::OpenErrorKind::DdeFailure => {
                        filego::presentation::i18n::Msg::ErrorOpenDde
                    }
                    filego::platform::shell_open::OpenErrorKind::ShellRejected => {
                        filego::presentation::i18n::Msg::ErrorOpenShellRejected
                    }
                    filego::platform::shell_open::OpenErrorKind::Unavailable => {
                        filego::presentation::i18n::Msg::ErrorOpenUnavailable
                    }
                };
                (
                    filego::presentation::i18n::Msg::ErrorOpenTitle.tr(state.locale),
                    body.tr(state.locale),
                )
            }
            Some(_failure) => (
                filego::presentation::i18n::Msg::ErrorSearchTitle.tr(state.locale),
                filego::presentation::i18n::Msg::ErrorSearchBody.tr(state.locale),
            ),
            None => match state.no_result_reason {
                Some(filego::search::NoResultReason::NoData) => (
                    filego::presentation::i18n::Msg::NoDataTitle.tr(state.locale),
                    filego::presentation::i18n::Msg::NoDataBody.tr(state.locale),
                ),
                Some(filego::search::NoResultReason::FilteredOut) => (
                    filego::presentation::i18n::Msg::FilteredOutTitle.tr(state.locale),
                    filego::presentation::i18n::Msg::FilteredOutBody.tr(state.locale),
                ),
                Some(filego::search::NoResultReason::NoMatch) => (
                    filego::presentation::i18n::Msg::NoMatchTitle.tr(state.locale),
                    filego::presentation::i18n::Msg::NoMatchBody.tr(state.locale),
                ),
                None => (String::new(), String::new()),
            },
        };
        window.set_state_shows_empty(show_empty);
        window.set_state_empty_title(title.into());
        window.set_state_empty_body(body.into());
    }
}

fn string_model(values: impl Iterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        values.map(slint::SharedString::from).collect::<Vec<_>>(),
    ))
}

/// A context-menu action routed from the search window to the settings window
/// (M05.5). The settings controller resolves the targe folder by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Edit,
    TogglePin,
    ToggleEnable,
    Remove,
}

/// A context effect pushed to the settings controller through a channel.
#[derive(Debug, Clone)]
pub enum ContextEffect {
    OpenEdit { folder_id: uuid::Uuid },
    TogglePin { folder_id: uuid::Uuid },
    ToggleEnable { folder_id: uuid::Uuid },
    Remove { folder_id: uuid::Uuid },
}

impl ContextEffect {
    fn from_row(action: ContextAction, folder_id: uuid::Uuid) -> Self {
        match action {
            ContextAction::Edit => ContextEffect::OpenEdit { folder_id },
            ContextAction::TogglePin => ContextEffect::TogglePin { folder_id },
            ContextAction::ToggleEnable => ContextEffect::ToggleEnable { folder_id },
            ContextAction::Remove => ContextEffect::Remove { folder_id },
        }
    }
}

/// The 0.0.1 adapter that owns the settings window, drives the manager with ONE
/// shared repository, forwards tray/picker/context commands, and pushes the
/// manager's [`MView`] into the Slint `SettingsWindow`.
struct SettingsWindowController {
    manager: ManagementController<filego::presentation::manager::SharedStore>,
    window: slint::Weak<SettingsWindow>,
    /// Receiver for context-menu effects from the search window (drained by a
    /// UI-thread timer).
    context_rx: std::sync::mpsc::Receiver<ContextEffect>,
    /// The folder-picker entry point; injected so the bin can be tested.
    picker: fn() -> filego::platform::windows::folder_picker::PickOutcome,
    /// Shared repository (the search window rebuilds its entries from it).
    repo: std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>>,
}

impl SettingsWindowController {
    fn new(
        window: slint::Weak<SettingsWindow>,
        store: filego::presentation::manager::SharedStore,
        context_rx: std::sync::mpsc::Receiver<ContextEffect>,
    ) -> Self {
        let repo = store.repo().clone();
        let one_level_import_setting = store
            .document()
            .map(|document| document.data.settings.one_level_import)
            .unwrap_or(false);
        let manager = ManagementController::new(store, one_level_import_setting);
        Self {
            manager,
            window,
            context_rx,
            picker: filego::platform::windows::folder_picker::pick_folder_dialog,
            repo,
        }
    }

    fn handle(&mut self, command: MCommand) {
        self.manager.handle(command);
        self.sync_ui();
    }

    /// The shared repository (used by the search window to rebuild entries
    /// after a management change).
    fn manager_repo(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>> {
        self.repo.clone()
    }

    /// Drain context-menu effects from the search window.
    fn drain_context(&mut self) {
        while let Ok(effect) = self.context_rx.try_recv() {
            match effect {
                ContextEffect::OpenEdit { folder_id } => {
                    let id = filego::domain::ids::FolderId::from_uuid(folder_id);
                    self.manager.handle(MCommand::EditFolder(id));
                    self.show();
                }
                ContextEffect::TogglePin { folder_id } => {
                    let id = filego::domain::ids::FolderId::from_uuid(folder_id);
                    self.manager.handle(MCommand::TogglePin(id));
                }
                ContextEffect::ToggleEnable { folder_id } => {
                    let id = filego::domain::ids::FolderId::from_uuid(folder_id);
                    self.manager.handle(MCommand::ToggleEnable(id));
                }
                ContextEffect::Remove { folder_id } => {
                    let id = filego::domain::ids::FolderId::from_uuid(folder_id);
                    self.manager.handle(MCommand::StartRemove(id));
                    self.show();
                }
            }
            self.sync_ui();
        }
    }

    fn show(&self) {
        if let Some(window) = self.window.upgrade() {
            let _ = window.show();
        }
    }

    /// Open the native folder picker and forward results (add flow).
    fn browse(&mut self) {
        if let filego::platform::windows::folder_picker::PickOutcome::Picked(paths) =
            (self.picker)()
        {
            self.manager.handle(MCommand::OpenPickPaths(paths));
        }
        self.sync_ui();
    }

    /// Paste the clipboard text as a manual path (add/edit field).
    fn paste_into_path(&mut self) {
        let path = filego::platform::windows::clipboard::read_text();
        if !path.trim().is_empty() {
            self.manager.handle(MCommand::EditPath(path));
        }
    }

    /// Push the full manager view into the Slint window.
    fn sync_ui(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let view = self.manager.view();

        window.set_page(match view.page {
            filego::presentation::manager::Page::Folders => 0,
            filego::presentation::manager::Page::Categories => 1,
            filego::presentation::manager::Page::Tags => 2,
        });

        window.set_rows_count(view.rows.len() as i32);
        window.set_rows_name(string_model(
            view.rows.iter().map(|r| r.display_name.clone()),
        ));
        window.set_rows_path(string_model(view.rows.iter().map(|r| r.path.clone())));
        window.set_rows_category(string_model(
            view.rows
                .iter()
                .map(|r| r.category_name.clone().unwrap_or_default()),
        ));
        window.set_rows_tags(string_model(
            view.rows.iter().flat_map(|r| r.tag_names.clone()),
        ));
        let (enabled, pinned, favorite, ids): (Vec<bool>, Vec<bool>, Vec<bool>, Vec<i32>) = view
            .rows
            .iter()
            .map(|r| {
                (
                    r.enabled,
                    r.pinned,
                    r.favorite,
                    r.id.as_uuid().as_u128() as i32,
                )
            })
            .collect();
        window.set_rows_enabled(ModelRc::new(VecModel::from(enabled)));
        window.set_rows_pinned(ModelRc::new(VecModel::from(pinned)));
        window.set_rows_favorite(ModelRc::new(VecModel::from(favorite)));
        window.set_rows_ids(ModelRc::new(VecModel::from(ids)));

        window.set_categories_count(view.categories.len() as i32);
        window.set_categories_name(string_model(view.categories.iter().map(|c| c.name.clone())));
        window.set_categories_counts(ModelRc::new(VecModel::from(
            view.categories
                .iter()
                .map(|c| c.count as i32)
                .collect::<Vec<_>>(),
        )));

        window.set_tags_count(view.tags.len() as i32);
        window.set_tags_name(string_model(view.tags.iter().map(|t| t.name.clone())));
        window.set_tags_usage(ModelRc::new(VecModel::from(
            view.tags.iter().map(|t| t.usage as i32).collect::<Vec<_>>(),
        )));

        // Import offer.
        if let Some(offer) = &view.import_offer {
            window.set_import_offer_visible(true);
            window.set_import_offer_parent(offer.parent_path.clone().into());
            window.set_import_offer_children(offer.child_count as i32);
            window.set_import_offer_hover(offer.hover.clone().into());
        } else {
            window.set_import_offer_visible(false);
        }

        // Notice.
        let locale = filego::presentation::i18n::Locale::default();
        window.set_notice_text(
            view.notice
                .map(|notice| settings_notice_text(notice, locale))
                .unwrap_or_default()
                .into(),
        );

        // Pending remove banner.
        if let Some(pending) = &view.pending_remove {
            window.set_has_pending_remove(true);
            window.set_pending_remove_name(pending.folder_name.clone().into());
            window.set_pending_remove_seconds(i32::from(pending.seconds_left));
        } else {
            window.set_has_pending_remove(false);
            window.set_pending_remove_name("".into());
            window.set_pending_remove_seconds(0);
        }

        // Draft dialog.
        match &view.add_flow {
            filego::presentation::manager::AddFlowView::Draft(draft) => {
                window.set_draft_visible(true);
                window.set_draft_name(draft.display_name.clone().into());
                window.set_draft_path(draft.path.clone().into());
                window.set_draft_note(draft.note.clone().into());
                let title = if draft.id.is_some() {
                    Msg::EditDialogTitle
                } else {
                    Msg::AddDialogTitle
                };
                window.set_draft_title(title.tr(locale).into());
                let error = if !draft.valid {
                    Msg::NoticeInvalidPath.tr(locale)
                } else if let Some(existing) = &draft.duplicate_existing {
                    format!("{}: {existing}", Msg::NoticeDuplicateBlocked.tr(locale))
                } else {
                    String::new()
                };
                window.set_draft_error(error.into());
                window.set_draft_color(slint::Color::from_rgb_u8(0x25, 0x63, 0xEB));
            }
            filego::presentation::manager::AddFlowView::Preview(batch) => {
                window.set_draft_visible(false);
                window.set_batch_visible(true);
                window.set_batch_count(batch.items.len() as i32);
                window.set_batch_name(string_model(batch.items.iter().map(|i| i.name.clone())));
                window.set_batch_path(string_model(batch.items.iter().map(|i| i.path.clone())));
                window.set_batch_dup(ModelRc::new(VecModel::from(
                    batch
                        .items
                        .iter()
                        .map(|i| i.duplicate_existing.is_some())
                        .collect::<Vec<_>>(),
                )));
                window.set_batch_invalid(ModelRc::new(VecModel::from(
                    batch.items.iter().map(|i| i.invalid).collect::<Vec<_>>(),
                )));
            }
            filego::presentation::manager::AddFlowView::Closed => {
                window.set_draft_visible(false);
                window.set_batch_visible(false);
            }
        }
    }
}

/// Localize a manager notice (privacy-safe; never a path).
fn settings_notice_text(
    notice: filego::presentation::manager::Notice,
    locale: filego::presentation::i18n::Locale,
) -> String {
    let msg = match notice {
        filego::presentation::manager::Notice::Saved => Msg::NoticeSaved,
        filego::presentation::manager::Notice::SaveFailed => Msg::NoticeSaveFailed,
        filego::presentation::manager::Notice::Removed => Msg::NoticeRemoved,
        filego::presentation::manager::Notice::Restored => Msg::NoticeRestored,
        filego::presentation::manager::Notice::UndoExpired => Msg::NoticeUndoExpired,
        filego::presentation::manager::Notice::CannotUndo => Msg::NoticeCannotUndo,
        filego::presentation::manager::Notice::DuplicateBlocked => Msg::NoticeDuplicateBlocked,
        filego::presentation::manager::Notice::InvalidPath => Msg::NoticeInvalidPath,
        filego::presentation::manager::Notice::InvalidName => Msg::NoticeInvalidName,
        filego::presentation::manager::Notice::NotFound => Msg::NoticeNotFound,
        filego::presentation::manager::Notice::PathAccessible => Msg::NoticePathAccessible,
        filego::presentation::manager::Notice::PathInaccessible => Msg::NoticePathInaccessible,
        filego::presentation::manager::Notice::ImportLimited => Msg::NoticeImportLimited,
    };
    msg.tr(locale)
}

/// The results-count label for the current rows, through the i18n catalog.
/// F001 fix: the count is derived from `rows.len()` on every `sync_ui`, never a
/// constant, so the window never renders a stale "0 results".
fn results_count_label(
    rows: &[filego::presentation::state::ResultRow],
    locale: filego::presentation::i18n::Locale,
) -> slint::SharedString {
    filego::presentation::i18n::Msg::ResultsCount(u16::try_from(rows.len()).unwrap_or(u16::MAX))
        .tr(locale)
        .into()
}

/// The FileGo data directory: `%LOCALAPPDATA%\FileGo`. Pure helper; the app
/// falls back to a portable path when the env var is missing (M05 keeps data
/// local and private).
fn data_dir() -> std::path::PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("FileGo-portable"));
    base.join("FileGo")
}

/// Open (or create) the shared repository at `data_dir()`. A first run has no
/// document: we seed an empty (but valid) one so the whole flow (add/edit/
/// category/tag) is available immediately. Corruption/pending-recovery is kept
/// explicit: the repository reports it and the app starts with an empty
/// working copy rather than destroying the corrupt file.
fn open_repository()
-> std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>> {
    let base = data_dir();
    let paths = filego::storage::location::DocumentPaths::from_base_dir(&base);
    let mut repo = filego::storage::repository::DocumentRepository::new(paths);
    let result = repo.load();
    // On first run (`NotFound`) we seed a valid empty document through the
    // public `save` so the revision/lock machinery is exercised exactly like a
    // real first save. On corruption we keep the file untouched and start with
    // an empty working copy the user can repair via the normal recovery path.
    if matches!(
        result,
        Err(filego::storage::repository::RepositoryError::NotFound)
    ) {
        let empty = empty_document();
        if repo.save(&empty).is_err() {
            eprintln!("FileGo could not create its data file");
        }
    } else if result.is_err() {
        // Corrupt/unreadable: keep the file, start with no working copy. The
        // repository recovery paths (backup/repair) remain the only writers.
        eprintln!("FileGo could not load stored data; recovery is pending");
    }
    let _ = base;
    std::rc::Rc::new(std::cell::RefCell::new(repo))
}

/// Push the settings-window i18n strings into the shared `UiStrings` global.
/// The settings window reuses the app window's `UiStrings` global (`in-out`
/// properties are shared across components in one compiled module).
fn apply_settings_localization(
    window: &SettingsWindow,
    locale: filego::presentation::i18n::Locale,
) {
    let entries: &[(&str, Msg)] = &[
        ("settings_title", Msg::SettingsTitle),
        ("page_folders", Msg::SettingsFolders),
        ("page_categories", Msg::SettingsCategories),
        ("page_tags", Msg::SettingsTags),
        ("col_name", Msg::ColName),
        ("col_path", Msg::ColPath),
        ("col_status", Msg::ColStatus),
        ("col_actions", Msg::ColActions),
        ("action_add", Msg::ActionAdd),
        ("action_edit", Msg::ActionEdit),
        ("action_remove", Msg::ActionRemoveRecord),
        ("action_toggle_enabled", Msg::ActionAdd),
        ("action_toggle_pin", Msg::PinnedLabel),
        ("action_check", Msg::ActionCheck),
        ("filter_name_placeholder", Msg::FilterNamePlaceholder),
        ("uncategorized_label", Msg::Uncategorized),
        ("enabled_label", Msg::EnabledLabel),
        ("disabled_label", Msg::DisabledLabel),
        ("pinned_label", Msg::PinnedLabel),
        ("notice_saved", Msg::NoticeSaved),
        ("notice_save_failed", Msg::NoticeSaveFailed),
        ("notice_removed", Msg::NoticeRemoved),
        ("notice_restored", Msg::NoticeRestored),
        ("notice_undo_expired", Msg::NoticeUndoExpired),
        ("notice_cannot_undo", Msg::NoticeCannotUndo),
        ("notice_duplicate_blocked", Msg::NoticeDuplicateBlocked),
        ("notice_invalid_path", Msg::NoticeInvalidPath),
        ("notice_invalid_name", Msg::NoticeInvalidName),
        ("notice_not_found", Msg::NoticeNotFound),
        ("notice_path_accessible", Msg::NoticePathAccessible),
        ("notice_path_inaccessible", Msg::NoticePathInaccessible),
        ("notice_import_limited", Msg::NoticeImportLimited),
        ("remove_confirm", Msg::RemoveConfirm),
        ("remove_never_deletes", Msg::RemoveNeverDeletes),
        ("undo", Msg::Undo),
        ("add_dialog_title", Msg::AddDialogTitle),
        ("edit_dialog_title", Msg::EditDialogTitle),
        ("field_name", Msg::FieldName),
        ("field_path", Msg::FieldPath),
        ("save", Msg::Save),
        ("cancel", Msg::Cancel),
        ("paste", Msg::ActionPaste),
        ("browse", Msg::ActionBrowse),
        ("import_offer_title", Msg::ImportOfferTitle),
        ("import_parent_only", Msg::ImportParentOnly),
        ("import_children", Msg::ImportChildren),
        ("import_second_confirm", Msg::ImportSecondConfirm),
        ("import_max_hint", Msg::ImportChildren),
        ("batch_preview_title", Msg::BatchPreviewTitle),
        ("batch_add", Msg::BatchAdd),
        ("batch_status_duplicate", Msg::BatchStatusDuplicate),
        ("batch_status_invalid", Msg::BatchStatusInvalid),
        ("category_create", Msg::CategoryCreate),
        ("category_rename", Msg::CategoryCreate),
        ("category_delete", Msg::CategoryDelete),
        ("tag_create", Msg::TagCreate),
        ("tag_rename", Msg::TagCreate),
        ("tag_merge_to", Msg::TagMerge),
        ("tag_delete", Msg::TagDelete),
        ("usage_suffix", Msg::UsageSuffix),
    ];
    for (field, msg) in entries {
        let value = msg.tr(locale);
        // The generated setters are named `set_<field>`.
        call_string_setter(window, field, value);
    }
}

/// Route a string property value to the right generated setter. The settings
/// window's properties were generated from the `UiStrings` global, so the
/// setter lives on the global accessor, not the window.
fn call_string_setter(window: &SettingsWindow, field: &str, value: String) {
    let strings = window.global::<UiStrings>();
    let value = slint::SharedString::from(&value);
    match field {
        "settings_title" => strings.set_settings_title(value),
        "page_folders" => strings.set_page_folders(value),
        "page_categories" => strings.set_page_categories(value),
        "page_tags" => strings.set_page_tags(value),
        "col_name" => strings.set_col_name(value),
        "col_path" => strings.set_col_path(value),
        "col_status" => strings.set_col_status(value),
        "col_actions" => strings.set_col_actions(value),
        "action_add" => strings.set_action_add(value),
        "action_edit" => strings.set_action_edit(value),
        "action_remove" => strings.set_action_remove(value),
        "action_toggle_enabled" => strings.set_action_toggle_enabled(value),
        "action_toggle_pin" => strings.set_action_toggle_pin(value),
        "action_check" => strings.set_action_check(value),
        "filter_name_placeholder" => strings.set_filter_name_placeholder(value),
        "uncategorized_label" => strings.set_uncategorized_label(value),
        "enabled_label" => strings.set_enabled_label(value),
        "disabled_label" => strings.set_disabled_label(value),
        "pinned_label" => strings.set_pinned_label(value),
        "notice_saved" => strings.set_notice_saved(value),
        "notice_save_failed" => strings.set_notice_save_failed(value),
        "notice_removed" => strings.set_notice_removed(value),
        "notice_restored" => strings.set_notice_restored(value),
        "notice_undo_expired" => strings.set_notice_undo_expired(value),
        "notice_cannot_undo" => strings.set_notice_cannot_undo(value),
        "notice_duplicate_blocked" => strings.set_notice_duplicate_blocked(value),
        "notice_invalid_path" => strings.set_notice_invalid_path(value),
        "notice_invalid_name" => strings.set_notice_invalid_name(value),
        "notice_not_found" => strings.set_notice_not_found(value),
        "notice_path_accessible" => strings.set_notice_path_accessible(value),
        "notice_path_inaccessible" => strings.set_notice_path_inaccessible(value),
        "notice_import_limited" => strings.set_notice_import_limited(value),
        "remove_confirm" => strings.set_remove_confirm(value),
        "remove_never_deletes" => strings.set_remove_never_deletes(value),
        "undo" => strings.set_undo(value),
        "add_dialog_title" => strings.set_add_dialog_title(value),
        "edit_dialog_title" => strings.set_edit_dialog_title(value),
        "field_name" => strings.set_field_name(value),
        "field_path" => strings.set_field_path(value),
        "save" => strings.set_save(value),
        "cancel" => strings.set_cancel(value),
        "paste" => strings.set_paste(value),
        "browse" => strings.set_browse(value),
        "import_offer_title" => strings.set_import_offer_title(value),
        "import_parent_only" => strings.set_import_parent_only(value),
        "import_children" => strings.set_import_children(value),
        "import_second_confirm" => strings.set_import_second_confirm(value),
        "import_max_hint" => strings.set_import_max_hint(value),
        "batch_preview_title" => strings.set_batch_preview_title(value),
        "batch_add" => strings.set_batch_add(value),
        "batch_status_duplicate" => strings.set_batch_status_duplicate(value),
        "batch_status_invalid" => strings.set_batch_status_invalid(value),
        "category_create" => strings.set_category_create(value),
        "category_rename" => strings.set_category_rename(value),
        "category_delete" => strings.set_category_delete(value),
        "tag_create" => strings.set_tag_create(value),
        "tag_rename" => strings.set_tag_rename(value),
        "tag_merge_to" => strings.set_tag_merge_to(value),
        "tag_delete" => strings.set_tag_delete(value),
        "usage_suffix" => strings.set_usage_suffix(value),
        _ => {}
    }
}

fn empty_document() -> filego::storage::schema::StoredDocumentV1 {
    use filego::storage::schema::StoredDocumentV1;
    StoredDocumentV1::new(filego::domain::document::AppData {
        settings: AppSettings::default(),
        folders: Vec::new(),
        categories: Vec::new(),
        tags: Vec::new(),
        revision: 1,
    })
}

/// Build the search-resolved entries from the repository document (M05).
fn resolved_from_repository(
    repo: &std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>>,
) -> Vec<ResolvedEntry> {
    let Some(document) = repo.borrow().document().cloned() else {
        return Vec::new();
    };
    let categories = &document.data.categories;
    let tags = &document.data.tags;
    let category_name = |id: filego::domain::ids::CategoryId| -> Option<String> {
        categories
            .iter()
            .find(|category| category.id == id)
            .map(|category| category.name.clone())
    };
    let tag_names = |ids: &[filego::domain::ids::TagId]| -> Vec<String> {
        tags.iter()
            .filter(|tag| ids.contains(&tag.id))
            .map(|tag| tag.name.clone())
            .collect()
    };

    let searchable: Vec<filego::search::SearchEntry> = document
        .data
        .folders
        .iter()
        .filter(|folder| folder.enabled)
        .map(|folder| filego::search::SearchEntry {
            id: folder.id,
            display_name: folder.display_name.clone(),
            aliases: folder.aliases.clone(),
            path: folder.path.clone(),
            category_name: folder.category_id.and_then(category_name),
            tag_names: tag_names(&folder.tag_ids),
            note: folder.note.clone(),
            pinned: folder.pinned,
            favorite: folder.favorite,
            manual_weight: folder.manual_weight,
            open_count: folder.open_count,
            last_opened_at: folder.last_opened_at,
            accessibility: filego::search::Accessibility::Unknown,
            origin: filego::search::Origin::Unknown,
        })
        .collect();
    filego::presentation::view_model::resolved_from_search(&searchable, |entry| {
        entry
            .path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&entry.path)
            .to_owned()
    })
}

fn color_from_hex(hex: &str) -> slint::Color {
    let rgb = u32::from_str_radix(&hex[1..], 16).expect("theme token must be #RRGGBB");
    slint::Color::from_rgb_u8(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Push resolved i18n strings and theme tokens into the UI globals.
fn apply_localization_and_theme(window: &AppWindow, locale: filego::presentation::i18n::Locale) {
    let strings = window.global::<UiStrings>();
    strings.set_search_placeholder(
        filego::presentation::i18n::Msg::SearchPlaceholder
            .tr(locale)
            .into(),
    );
    strings.set_clear(filego::presentation::i18n::Msg::Clear.tr(locale).into());
    strings.set_empty_hint(filego::presentation::i18n::Msg::EmptyHint.tr(locale).into());
    strings.set_no_data_title(
        filego::presentation::i18n::Msg::NoDataTitle
            .tr(locale)
            .into(),
    );
    strings.set_no_data_body(
        filego::presentation::i18n::Msg::NoDataBody
            .tr(locale)
            .into(),
    );
    strings.set_filtered_title(
        filego::presentation::i18n::Msg::FilteredOutTitle
            .tr(locale)
            .into(),
    );
    strings.set_filtered_body(
        filego::presentation::i18n::Msg::FilteredOutBody
            .tr(locale)
            .into(),
    );
    strings.set_no_match_title(
        filego::presentation::i18n::Msg::NoMatchTitle
            .tr(locale)
            .into(),
    );
    strings.set_no_match_body(
        filego::presentation::i18n::Msg::NoMatchBody
            .tr(locale)
            .into(),
    );
    strings.set_error_load_title(
        filego::presentation::i18n::Msg::ErrorLoadTitle
            .tr(locale)
            .into(),
    );
    strings.set_error_load_body(
        filego::presentation::i18n::Msg::ErrorLoadBody
            .tr(locale)
            .into(),
    );
    strings.set_error_save_title(
        filego::presentation::i18n::Msg::ErrorSaveTitle
            .tr(locale)
            .into(),
    );
    strings.set_error_save_body(
        filego::presentation::i18n::Msg::ErrorSaveBody
            .tr(locale)
            .into(),
    );
    strings.set_error_search_title(
        filego::presentation::i18n::Msg::ErrorSearchTitle
            .tr(locale)
            .into(),
    );
    strings.set_error_search_body(
        filego::presentation::i18n::Msg::ErrorSearchBody
            .tr(locale)
            .into(),
    );
    strings.set_hint_select(
        filego::presentation::i18n::Msg::HintNavigate
            .tr(locale)
            .into(),
    );
    strings.set_hint_open(filego::presentation::i18n::Msg::HintOpen.tr(locale).into());
    strings.set_hint_close(filego::presentation::i18n::Msg::HintClose.tr(locale).into());
    strings.set_hint_toggle(
        filego::presentation::i18n::Msg::HintToggleHints
            .tr(locale)
            .into(),
    );
    // NOTE(F001): the results-count label is NOT set here — it is re-derived
    // from `ViewState::rows` on every `sync_ui` push (see
    // `results_count_label`), so the rendered count always matches the rows.
    // Setting it once with a constant (the old `ResultsCount(0)`) made the
    // label stay "0 个结果" forever.
    strings.set_inaccessible(
        filego::presentation::i18n::Msg::Inaccessible
            .tr(locale)
            .into(),
    );
    strings.set_inaccessible_tooltip(
        filego::presentation::i18n::Msg::InaccessibleTooltip
            .tr(locale)
            .into(),
    );

    let theme = filego::presentation::theme::ResolvedTheme::resolve(
        filego::domain::settings::ThemePreference::System,
        filego::presentation::theme::ResolvedColorScheme::Light,
    );
    ui_set_theme(window, theme);
}

fn ui_set_theme(window: &AppWindow, theme: filego::presentation::theme::ResolvedTheme) {
    let theme_global = window.global::<UiTheme>();
    theme_global.set_background(color_from_hex(theme.background));
    theme_global.set_surface(color_from_hex(theme.surface));
    theme_global.set_border(color_from_hex(theme.border));
    theme_global.set_text(color_from_hex(theme.text));
    theme_global.set_text_secondary(color_from_hex(theme.text_secondary));
    theme_global.set_accent(color_from_hex(theme.accent));
    theme_global.set_on_accent(color_from_hex(theme.on_accent));
    theme_global.set_warning(color_from_hex(theme.warning));
    theme_global.set_selection(color_from_hex(theme.selection));
}

fn run() -> Result<(), slint::PlatformError> {
    // ---- M04.3 single-instance guard --------------------------------
    // A second instance signals the primary and exits BEFORE any window or
    // tray is created (so no second tray icon can ever appear). The primary
    // MUST keep the mutex handle alive for the whole process life (dropping it
    // would let a later instance wrongly become primary), so we bind it to a
    // guard that lives until `run()` returns.
    let _instance_guard = match filego::platform::windows::single_instance::InstanceMutex::acquire()
    {
        Ok(inst) => {
            if !inst.is_primary() {
                // Second instance: activate the primary and exit.
                let _outcome =
                    filego::platform::windows::single_instance::activator::activate_primary(2_000);
                return Ok(());
            }
            Some(inst)
        }
        Err(_) => {
            // Could not verify single-instance; proceed as primary (a second
            // instance is still prevented by the OS mutex semantics for the
            // owned case; a failure here should not brick startup).
            eprintln!("FileGo could not verify single-instance status");
            None
        }
    };

    let app = AppWindow::new()?;
    let tray = AppTray::new()?;

    // ---- M04.2/M04.3 native platform --------------------------------
    let settings = AppSettings::default();
    let native =
        filego::platform::windows::NativePlatform::start(settings.hotkey).unwrap_or_else(|_| {
            // A failed native worker must not prevent the tray shell from
            // running; hotkey/IPC degrade gracefully.
            filego::platform::windows::NativePlatform::disabled()
        });
    // The native platform is shared (UI-thread only) between the tray callbacks
    // and the event-drain timer; Slint timers/callbacks are FnMut on the UI
    // thread so `Rc<RefCell<_>>` needs no Send.
    let native = Rc::new(RefCell::new(native));

    let controller = Rc::new(RefCell::new(LifecycleController::new()));
    let port = Rc::new(RefCell::new(SlintWindowPort {
        window: app.as_weak(),
    }));

    apply_localization_and_theme(&app, filego::presentation::i18n::Locale::default());

    {
        let controller = Rc::clone(&controller);
        app.window().on_close_requested(move || {
            controller.borrow_mut().accept_window_close();
            CloseRequestResponse::HideWindow
        });
    }

    {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        app.on_hide_requested(move || {
            report_platform_error(
                controller
                    .borrow_mut()
                    .handle(LifecycleCommand::Hide, &mut *port.borrow_mut()),
            );
        });
    }

    {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        tray.on_toggle_window(move || {
            report_platform_error(
                controller
                    .borrow_mut()
                    .handle(LifecycleCommand::Toggle, &mut *port.borrow_mut()),
            );
        });
    }

    {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        tray.on_open_window(move || {
            report_platform_error(
                controller
                    .borrow_mut()
                    .handle(LifecycleCommand::Show, &mut *port.borrow_mut()),
            );
        });
    }

    {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        tray.on_quit_requested(move || {
            // Clean exit: unregister the hotkey, drop the mutex, quit.
            report_platform_error(
                controller
                    .borrow_mut()
                    .handle(LifecycleCommand::ExitFromTray, &mut *port.borrow_mut()),
            );
        });
    }

    // ---- M04.1 native tray actions -----------------------------------
    // (Slint's SystemTrayIcon delivers these on the UI thread.)
    // Launch-at-login and pause-hotkeys are checkbox-style menu items whose
    // glyph state is a bound Slint property; we refresh both at startup so the
    // menu reflects the persisted registry state / current pause state, and on
    // each toggle.
    tray.set_launch_at_login_glyph(
        if filego::platform::windows::tray_open::launch_at_login().unwrap_or(false) {
            "✓ "
        } else {
            ""
        }
        .into(),
    );
    tray.set_pause_hotkeys_glyph(if native.borrow().paused() { "✓ " } else { "" }.into());

    // ---- M05: shared repository + settings window + shared adapter -----
    let repo = open_repository();
    let (context_tx, context_rx) = std::sync::mpsc::channel();
    let settings_window = SettingsWindow::new()?;
    apply_settings_localization(
        &settings_window,
        filego::presentation::i18n::Locale::default(),
    );
    let store = filego::presentation::manager::SharedStore::new(Rc::clone(&repo));
    let settings_adapter = Rc::new(RefCell::new(SettingsWindowController::new(
        settings_window.as_weak(),
        store,
        context_rx,
    )));

    {
        let settings = Rc::clone(&settings_adapter);
        tray.on_add_folder(move || {
            // M05.2: tray "添加文件夹" opens the settings window and runs the
            // native folder picker directly (user-initiated, no scan).
            settings.borrow_mut().browse();
            if let Some(window) = settings.borrow().window.upgrade() {
                let _ = window.show();
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        tray.on_open_settings(move || {
            // M05/M06: tray "设置" opens the settings window.
            if let Some(window) = settings.borrow().window.upgrade() {
                let _ = window.show();
            }
        });
    }
    {
        let tray_weak = tray.as_weak();
        tray.on_toggle_launch_at_login(move || {
            let enabled = !filego::platform::windows::tray_open::launch_at_login().unwrap_or(false);
            let _ = filego::platform::windows::tray_open::set_launch_at_login(enabled);
            if let Some(tray) = tray_weak.upgrade() {
                tray.set_launch_at_login_glyph(if enabled { "✓ " } else { "" }.into());
            }
        });
    }
    {
        let tray_weak = tray.as_weak();
        let native = Rc::clone(&native);
        tray.on_toggle_pause_hotkeys(move || {
            let paused = native.borrow().toggle_pause();
            if let Some(tray) = tray_weak.upgrade() {
                tray.set_pause_hotkeys_glyph(if paused { "✓ " } else { "" }.into());
            }
        });
    }
    {
        tray.on_show_about(move || {
            filego::platform::windows::tray_open::about_box();
        });
    }

    // ---- M04.2 native drain timer (hotkey + second-instance) ----------
    // The worker thread forwards resolved events over the channel; a repeating
    // UI-thread timer drains them and shows/focuses the search window through
    // the lifecycle controller (keeps state consistent).
    let native_drain = {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        let native = Rc::clone(&native);
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || {
                while let Some(event) = native.borrow().try_recv_event() {
                    match event {
                        filego::platform::windows::NativeEvent::HotkeyShow
                        | filego::platform::windows::NativeEvent::ActivateFromSecondInstance => {
                            report_platform_error(
                                controller
                                    .borrow_mut()
                                    .handle(LifecycleCommand::Show, &mut *port.borrow_mut()),
                            );
                        }
                    }
                }
            },
        );
        timer
    };
    let _native_drain = native_drain; // the timer lives for the whole event loop

    // ---- M03/M05: shared repository-backed ViewModel adapter wiring -----
    let resolved = resolved_from_repository(&repo);
    let settings = AppSettings {
        one_level_import: repo
            .borrow()
            .document()
            .map(|document| document.data.settings.one_level_import)
            .unwrap_or(false),
        ..AppSettings::default()
    };
    let (open_tx, open_rx) = std::sync::mpsc::channel();
    let shell = ShellHandle::real();
    let main = Rc::new(RefCell::new(MainWindowController::new(
        app.as_weak(),
        open_rx,
        open_tx,
        shell,
        context_tx,
        resolved,
        settings,
    )));

    let main_ui = Rc::clone(&main);
    app.on_command_query_edited(move |text| {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::QueryEdited(text.to_string()));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_clear_query(move || {
        main_ui.borrow_mut().handle(ViewCommand::ClearQuery);
    });
    let main_ui = Rc::clone(&main);
    app.on_command_navigate_up(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::SelectMove(SelectionMove::Up));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_navigate_down(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::SelectMove(SelectionMove::Down));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_escape(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::SearchKey(SearchKey::Escape));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_toggle_hints(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::SearchKey(SearchKey::ToggleHints));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_select_index(move |index| {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::SelectIndex(index as usize));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_open_selected(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::RowAction(RowAction::Open));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_open_index(move |index| {
        let mut main = main_ui.borrow_mut();
        main.handle(ViewCommand::SelectIndex(index as usize));
        main.handle(ViewCommand::RowAction(RowAction::Open));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_copy_path(move || {
        main_ui
            .borrow_mut()
            .handle(ViewCommand::RowAction(RowAction::CopyPath));
    });
    let main_ui = Rc::clone(&main);
    app.on_command_set_overlay(move |open| {
        main_ui.borrow_mut().handle(ViewCommand::SetOverlay(open));
    });

    // IME gate: while composing, the UI's key handler already rejects
    // Enter/arrows (see app-window.slint). The logic-level gate is driven
    // through `SearchViewModel::set_ime_composition`; final native IME wiring is
    // a manual-acceptance item (M03 task list).

    // ---- M04.5 async open-result drain ---------------------------------
    // Shell opens run on a worker thread; a repeating UI-thread timer drains
    // the result channel and applies hide/clear/failure per settings. The main
    // controller is `Rc<RefCell<_>>` (UI thread only), so this closure needs no
    // Send.
    let open_drain = {
        let main = Rc::clone(&main);
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || main.borrow_mut().drain_open_results(),
        );
        timer
    };
    let _open_drain = open_drain; // the timer lives for the whole event loop

    // ---- M05: settings window callback wiring ---------------------------
    // The settings Slint window forwards gestures as MCommand; the adapter
    // applies them against the manager and re-pushes the observable state. All
    // closures are UI-thread FnMut (Rc<RefCell<_>>, no Send needed).
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_close_settings(move || {
            if let Some(window) = settings.borrow().window.upgrade() {
                let _ = window.hide();
            }
        });
    }
    // Wire the settings callbacks. Each forwards an MCommand to the adapter
    // and re-pushes the observable state (the adapter's `handle` already
    // syncs_ui; a refresh of the search window happens via a shared repo only
    // when the manager mutated data, which the settings adapter performs
    // through `refresh_search`).
    {
        let settings = Rc::clone(&settings_adapter);
        let main = Rc::clone(&main);
        settings_window.on_command_show_page(move |page| {
            let cmd = match page {
                1 => MCommand::ShowPage(filego::presentation::manager::Page::Categories),
                2 => MCommand::ShowPage(filego::presentation::manager::Page::Tags),
                _ => MCommand::ShowPage(filego::presentation::manager::Page::Folders),
            };
            settings.borrow_mut().handle(cmd);
            let repo = settings.borrow().manager_repo();
            main.borrow_mut().refresh_from_repository(&repo);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_add(move || {
            settings.borrow_mut().handle(MCommand::OpenManual);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_paste(move || {
            settings.borrow_mut().paste_into_path();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_browse(move || {
            settings.borrow_mut().browse();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_edit(move |id| {
            let folder_id =
                filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id as u128));
            settings
                .borrow_mut()
                .handle(MCommand::EditFolder(folder_id));
            if let Some(window) = settings.borrow().window.upgrade() {
                let _ = window.show();
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_remove(move |id| {
            let folder_id =
                filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id as u128));
            settings
                .borrow_mut()
                .handle(MCommand::StartRemove(folder_id));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_enabled(move |id| {
            let folder_id =
                filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id as u128));
            settings
                .borrow_mut()
                .handle(MCommand::ToggleEnable(folder_id));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_pin(move |id| {
            let folder_id =
                filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id as u128));
            settings.borrow_mut().handle(MCommand::TogglePin(folder_id));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_check(move |id| {
            let folder_id =
                filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id as u128));
            settings.borrow_mut().handle(MCommand::CheckPath(folder_id));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_filter_name(move |text| {
            settings
                .borrow_mut()
                .handle(MCommand::SetFilterName(text.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_create_category(move |name| {
            settings
                .borrow_mut()
                .handle(MCommand::CreateCategory(name.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_delete_category(move |index| {
            let category_id = {
                let s = settings.borrow();
                s.manager
                    .view()
                    .categories
                    .get(index as usize)
                    .map(|c| c.id)
            };
            if let Some(category_id) = category_id {
                settings
                    .borrow_mut()
                    .handle(MCommand::DeleteCategory(category_id));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_create_tag(move |name| {
            settings
                .borrow_mut()
                .handle(MCommand::CreateTag(name.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_delete_tag(move |index| {
            let tag_id = {
                let s = settings.borrow();
                s.manager.view().tags.get(index as usize).map(|t| t.id)
            };
            if let Some(tag_id) = tag_id {
                settings.borrow_mut().handle(MCommand::DeleteTag(tag_id));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_confirm_remove(move || {
            settings.borrow_mut().handle(MCommand::ConfirmRemove);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_cancel_remove(move || {
            settings.borrow_mut().handle(MCommand::CancelRemove);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_undo_remove(move || {
            settings.borrow_mut().handle(MCommand::UndoRemove);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_save_draft(move || {
            settings.borrow_mut().handle(MCommand::SaveDraft);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_cancel_draft(move || {
            settings.borrow_mut().handle(MCommand::CancelDraft);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_draft_name(move |name| {
            settings
                .borrow_mut()
                .handle(MCommand::EditName(name.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_draft_path(move |path| {
            settings
                .borrow_mut()
                .handle(MCommand::EditPath(path.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_pinned(move || {
            settings.borrow_mut().handle(MCommand::ToggleDraftPinned);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_enabled(move |enabled| {
            settings
                .borrow_mut()
                .handle(MCommand::SetDraftEnabled(enabled));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_import_parent_only(move || {
            settings.borrow_mut().handle(MCommand::ChooseImport(
                filego::presentation::management::ChildImportChoice::ParentOnly,
            ));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_import_children(move || {
            settings.borrow_mut().handle(MCommand::ChooseImport(
                filego::presentation::management::ChildImportChoice::DirectChildren,
            ));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_import_dismiss(move || {
            settings.borrow_mut().handle(MCommand::DismissImport);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_apply_batch(move || {
            settings.borrow_mut().handle(MCommand::ApplyBatch);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_cancel_batch(move || {
            settings.borrow_mut().handle(MCommand::CancelBatch);
        });
    }

    // Push the initial settings view once.
    settings_adapter.borrow_mut().sync_ui();

    // ---- M05: context-effect drain (search → settings) -----------------
    let context_drain = {
        let settings = Rc::clone(&settings_adapter);
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || settings.borrow_mut().drain_context(),
        );
        timer
    };
    let _context_drain = context_drain; // the timer lives for the whole event loop

    // M00 shell starts in the tray. The visible SystemTrayIcon keeps the Slint
    // event loop alive until the explicit tray Exit command is handled.
    slint::run_event_loop()
}

fn report_platform_error(result: Result<filego::app::LifecycleState, slint::PlatformError>) {
    if result.is_err() {
        // Do not print native details: later platform errors can contain user paths.
        eprintln!("FileGo could not complete a window operation");
    }
}

fn main() {
    if std::env::args_os().skip(1).any(|argument| {
        argument == std::ffi::OsStr::new("--version") || argument == std::ffi::OsStr::new("-V")
    }) {
        println!("{}", version::display());
        return;
    }

    if run().is_err() {
        eprintln!("FileGo could not initialize its user interface");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_loop_errors_are_converted_for_the_shared_window_port() {
        let platform_error =
            event_loop_error_as_platform_error(slint::EventLoopError::EventLoopTerminated);

        assert_eq!(
            platform_error.to_string(),
            "The event loop was already terminated"
        );
    }

    #[test]
    fn theme_hex_colors_parse_to_rgb() {
        assert_eq!(
            color_from_hex("#005FB8"),
            slint::Color::from_rgb_u8(0, 0x5F, 0xB8)
        );
    }

    #[test]
    fn results_count_label_is_derived_from_the_row_count() {
        // F001: the count label must reflect the actual rows, never a hardcoded
        // 0. Both locales format the number into the {count} placeholder.
        let locale = filego::presentation::i18n::Locale::ZhCN;
        assert!(results_count_label(&[], locale).contains('0'));

        let rows = vec![
            filego::presentation::state::ResultRow {
                entry_id: uuid::Uuid::from_u128(1),
                display_name: "Documents".to_owned(),
                path_label: "Documents".to_owned(),
                full_path: r"C:\Users\me\Documents".to_owned(),
                category_name: None,
                tag_texts: Vec::new(),
                inaccessible: false,
            },
            filego::presentation::state::ResultRow {
                entry_id: uuid::Uuid::from_u128(2),
                display_name: "Photos".to_owned(),
                path_label: "Photos".to_owned(),
                full_path: r"C:\Users\me\Pictures".to_owned(),
                category_name: None,
                tag_texts: Vec::new(),
                inaccessible: false,
            },
        ];
        let zh = results_count_label(&rows, filego::presentation::i18n::Locale::ZhCN).to_string();
        let en = results_count_label(&rows, filego::presentation::i18n::Locale::EnUS).to_string();
        assert_eq!(zh, "2 个结果");
        assert_eq!(en, "2 results");
    }
}
