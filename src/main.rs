#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{cell::RefCell, rc::Rc};

use filego::{
    AppTray, AppWindow, SettingsWindow, UiStrings, UiTheme,
    app::{LifecycleCommand, LifecycleController, WindowPort},
    domain::settings::AppSettings,
    presentation::{
        commands::{RowAction, SearchKey, ViewCommand},
        i18n::{Locale, Msg},
        manager::{MCommand, ManagementController, ManagementStore},
        settings_controller::{
            CategoryNameCommand, SCommand, SNotice, SettingsController, SettingsSharedStore,
            TagNameCommand,
        },
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

/// The monitor-placement strategy for the search window (M06.2). Written by the
/// settings adapter when the user changes the 常规 page; read on every show by
/// `place_window`. 0 = Mouse (default), 1 = ActiveWindow. A plain atomic cell is
/// enough: the UI thread is the only writer and all readers are on the same
/// thread.
static MONITOR_STRATEGY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Apply the persisted monitor strategy (set from the settings adapter).
fn set_monitor_strategy(strategy: filego::domain::settings::MonitorStrategy) {
    let value = match strategy {
        filego::domain::settings::MonitorStrategy::Mouse => 0,
        filego::domain::settings::MonitorStrategy::ActiveWindow => 1,
    };
    MONITOR_STRATEGY.store(value, std::sync::atomic::Ordering::Relaxed);
}

/// The persisted search-window width override (logical px; M06.4). 0 = use the
/// window component default (600). Written by the settings adapter on change
/// and at startup; read by `place_window` so the width applies on every show.
static SEARCH_WINDOW_WIDTH: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Apply the persisted search-window width override (0 = component default).
fn set_search_window_width(width: Option<u16>) {
    SEARCH_WINDOW_WIDTH.store(width.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
}

/// Compute and apply the M04.4 window placement (monitor, centered,
/// top ≈ 10%, clamped to the work area) before showing.
///
/// M06.2: which monitor is targeted follows the persisted `MonitorStrategy` —
/// the cursor monitor (default) or the active window's monitor. F003: the
/// physical size is derived from the TARGET monitor's DPI scale — never the
/// window's current `scale_factor()`, which may describe a different monitor in
/// a mixed-DPI layout. Re-computed on every show so monitor count/DPI changes
/// are picked up deterministically.
fn place_window(app: &AppWindow) {
    let window = app.window();
    // M06.4: the width override (if set) replaces the window's own width on
    // every placement; the height stays dynamic (140 → results expand).
    let width_override = SEARCH_WINDOW_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
    if (filego::domain::settings::MIN_WINDOW_WIDTH..=filego::domain::settings::MAX_WINDOW_WIDTH)
        .contains(&width_override)
    {
        let scale = window.scale_factor();
        let physical_width = (f32::from(width_override) * scale).round() as u32;
        let physical_height = window.size().height;
        window.set_size(slint::PhysicalSize::new(physical_width, physical_height));
    }
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
    let rect =
        match MONITOR_STRATEGY.load(std::sync::atomic::Ordering::Relaxed) {
            1 => {
                let active_bits = filego::platform::windows::window_focus::active_window()
                    .map(|hwnd| hwnd.0 as isize);
                match active_bits {
                Some(bits) => filego::platform::windows::window_placement::
                    placement_rect_for_active_window(bits, logical_w, logical_h),
                None => filego::platform::windows::window_placement::placement_rect_for_cursor(
                    logical_w,
                    logical_h,
                ),
            }
            }
            _ => filego::platform::windows::window_placement::placement_rect_for_cursor(
                logical_w, logical_h,
            ),
        };
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

/// Show the settings window and bring it to the foreground (tray "设置" and any
/// other direct settings-open path). Mirror of `bring_search_to_front`: the
/// same native bring-to-front fallback chain, so the window is not merely
/// mapped but foreground-focused.
///
/// M06 review H1: the SettingsWindow has no focus-loss hide wiring
/// (`hide_on_focus_loss` only affects the main search window; the native focus
/// hook is an M07 manual item), so the settings window stays open while the
/// user works in it.
fn open_settings_window(settings: &std::rc::Rc<std::cell::RefCell<SettingsWindowController>>) {
    let Some(window) = settings.borrow().window.upgrade() else {
        return;
    };
    let _ = window.show();
    use raw_window_handle::HasWindowHandle as _;
    // Bind the winit window handle so its owned data outlives the
    // `HasWindowHandle` borrow (E0716, same as `bring_search_to_front`).
    let winit_window = window.window().window_handle();
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

    /// Replace the search settings and re-run the current query (M06: a
    /// search/appearance toggle takes effect immediately on the main window).
    fn apply_settings(&mut self, settings: AppSettings) {
        self.settings = settings.clone();
        let query = self.view_model.state().query.clone();
        self.view_model = SearchViewModel::new(
            settings.clone(),
            self.resolved.clone(),
            Box::new(default_runner),
            Box::new(NoopEffects),
        );
        self.view_model.handle(ViewCommand::QueryEdited(query));
        self.sync_ui();
    }

    /// Set the UI locale on the running ViewModel (re-read strings on next sync).
    fn set_locale(&mut self, locale: Locale) {
        self.view_model.set_locale(locale);
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
        // M05.5: per-row pin/enable state for the context-menu labels. Search
        // only shows enabled records (the repository filters on `enabled`), so
        // `rows-enabled` is `true` for every row; the disabled state is only
        // ever reached from the settings page. The pinned flag comes from the
        // resolved SearchEntry (rows are always built from `self.resolved`, so
        // the look-up succeeds for every row).
        let (rows_pinned, rows_enabled): (Vec<bool>, Vec<bool>) = rows
            .iter()
            .map(|row| {
                let pinned = self
                    .resolved
                    .iter()
                    .find(|entry| entry.searchable.id.as_uuid() == row.entry_id)
                    .map(|entry| entry.searchable.pinned)
                    .unwrap_or(false);
                (pinned, true)
            })
            .unzip();
        window.set_rows_pinned(ModelRc::new(VecModel::from(rows_pinned)));
        window.set_rows_enabled(ModelRc::new(VecModel::from(rows_enabled)));

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
    /// M06 settings controller (decision layer for the settings pages), over
    /// the same shared repository.
    settings: SettingsController<SettingsSharedStore>,
    window: slint::Weak<SettingsWindow>,
    /// Receiver for context-menu effects from the search window (drained by a
    /// UI-thread timer).
    context_rx: std::sync::mpsc::Receiver<ContextEffect>,
    /// The folder-picker entry point; injected so the bin can be tested.
    picker: fn() -> filego::platform::windows::folder_picker::PickOutcome,
    /// Shared repository (the search window rebuilds its entries from it).
    repo: std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>>,
    /// The native platform for hotkey record/pause/clear side effects.
    native: std::rc::Rc<std::cell::RefCell<filego::platform::windows::NativePlatform>>,
    /// The FileGo data directory (shown on the Data page + opened on request).
    data_dir: std::path::PathBuf,
    /// A parsed import document held between the preview and the apply step.
    pending_import: Option<filego::domain::document::AppData>,
    /// The main-window controller clone used to refresh the search results
    /// after a settings/management change (rebuilt settings make search take
    /// effect immediately).
    main: std::rc::Rc<std::cell::RefCell<MainWindowController>>,
}

impl SettingsWindowController {
    #[allow(clippy::too_many_arguments)]
    fn new(
        window: slint::Weak<SettingsWindow>,
        store: filego::presentation::manager::SharedStore,
        context_rx: std::sync::mpsc::Receiver<ContextEffect>,
        native: std::rc::Rc<std::cell::RefCell<filego::platform::windows::NativePlatform>>,
        data_dir: std::path::PathBuf,
        main: std::rc::Rc<std::cell::RefCell<MainWindowController>>,
    ) -> Self {
        let repo = store.repo().clone();
        let one_level_import_setting = store
            .document()
            .map(|document| document.data.settings.one_level_import)
            .unwrap_or(false);
        let manager = ManagementController::new(store, one_level_import_setting);
        let settings = SettingsController::new(SettingsSharedStore::new(repo.clone()));
        Self {
            manager,
            settings,
            window,
            context_rx,
            picker: filego::platform::windows::folder_picker::pick_folder_dialog,
            repo,
            native,
            data_dir,
            pending_import: None,
            main,
        }
    }

    fn handle(&mut self, command: MCommand) {
        self.manager.handle(command);
        self.sync_ui();
    }

    /// Apply one M06 settings command; the pure controller persists, then the
    /// adapter performs platform side effects (registry/hotkey/theme/locale).
    /// Side effects for a given command run AFTER the controller accepted it
    /// so a rejected/clamped value never triggers a fake platform action.
    fn handle_settings(&mut self, command: SCommand) {
        self.settings.handle(command.clone());
        self.apply_settings_side_effects(&command);
        self.sync_settings_ui();
        self.sync_ui();
    }

    /// Platform side effects that must happen after the controller persisted.
    fn apply_settings_side_effects(&mut self, command: &SCommand) {
        match command {
            SCommand::SetTheme(_) => {
                let theme = self.settings.view().settings.theme;
                let resolved = filego::presentation::theme::ResolvedTheme::resolve(
                    theme,
                    filego::presentation::theme::ResolvedColorScheme::Light,
                );
                if let Some(window) = self.window.upgrade() {
                    ui_set_theme_for_settings(&window, resolved);
                }
                // The main window shares the UiTheme global.
                self.settings_clear_notice_after_apply();
            }
            SCommand::SetMonitorStrategy(strategy) => {
                // The next `place_window` call targets the chosen monitor.
                set_monitor_strategy(*strategy);
            }
            SCommand::SetLanguage(language) => {
                let locale = locale_for(*language);
                if let Some(window) = self.window.upgrade() {
                    apply_settings_localization(&window, locale);
                }
                // The search window re-localizes too (shared UiStrings global is
                // set separately from run()'s main window; push only the locale).
                if let Some(app) = self.main.borrow().window.upgrade() {
                    apply_localization_and_theme(&app, locale);
                }
                self.main.borrow_mut().set_locale(locale);
            }
            SCommand::SetSearchPaths(_)
            | SCommand::SetSearchCategories(_)
            | SCommand::SetSearchTags(_)
            | SCommand::SetSearchNotes(_)
            | SCommand::SetSearchAliases(_)
            | SCommand::SetFuzzyMatching(_)
            | SCommand::SetSearchPinyin(_)
            | SCommand::SetSearchEnglishInitials(_)
            | SCommand::SetMaxEditDistance(_)
            | SCommand::SetMaxResults(_)
            | SCommand::SetEmptyQueryStrategy(_)
            | SCommand::SetHighlightResults(_)
            | SCommand::SetRecentSort(_) => {
                // The search settings feed the ViewModel rebuild.
                self.refresh_search_settings();
            }
            SCommand::SetShowPathInResults(_) | SCommand::SetShowCategoryTagInResults(_) => {
                if let Some(app) = self.main.borrow().window.upgrade() {
                    app.set_show_path(self.settings.view().settings.show_path_in_results);
                    app.set_show_cat_tag(
                        self.settings.view().settings.show_category_tag_in_results,
                    );
                }
            }
            SCommand::SetFontScalePercent(_) => {
                let scale = f32::from(self.settings.view().settings.font_scale_percent) / 100.0;
                if let Some(window) = self.window.upgrade() {
                    window.global::<UiTheme>().set_font_scale(scale);
                }
            }
            SCommand::SetSearchWindowWidth(_) => {
                // Applied on the next placement of the search window.
                set_search_window_width(self.settings.view().settings.search_window_width);
            }
            SCommand::SetSettingsWindowWidth(_) => {
                if let (Some(window), Some(width)) = (
                    self.window.upgrade(),
                    self.settings.view().settings.settings_window_width,
                ) {
                    apply_settings_window_width(&window, width);
                }
            }
            _ => {}
        }
    }

    /// Refresh the search window after a search-affecting settings change.
    fn refresh_search_settings(&mut self) {
        let settings = self.settings.view().settings.clone();
        self.main.borrow_mut().apply_settings(settings);
    }

    fn settings_clear_notice_after_apply(&self) {
        // Theme application is immediate; the Saved notice is pushed by the
        // controller's persist, nothing to clear here.
    }

    /// The shared repository (used by the search window to rebuild entries
    /// after a management change).
    fn manager_repo(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<filego::storage::repository::DocumentRepository>> {
        self.repo.clone()
    }

    /// Sync the M06 settings view into the Slint window.
    fn sync_settings_ui(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let view = self.settings.view();
        let settings = &view.settings;
        let locale = locale_for(settings.language_preference);

        // NOTE: `page` is pushed by the nav handlers (`on_command_show_page` /
        // `on_s_show_page`) — the settings controller and the manager each own a
        // sub-range, and having both syncs write `page` would let one overwrite
        // the other. One source of truth.

        window.set_s_launch_at_login(view.launch_at_login_os.unwrap_or(settings.launch_at_login));
        window.set_s_silent_start(settings.silent_start);
        window.set_s_show_main(settings.show_main_window_at_startup);
        window.set_s_hide_after_open(settings.hide_after_open);
        window.set_s_clear_after_open(settings.clear_after_open);
        window.set_s_hide_on_focus_loss(settings.hide_on_focus_loss);
        window.set_s_monitor_strategy(settings.monitor_strategy as i32);
        window.set_s_language(settings.language_preference as i32);

        window.set_s_search_paths(settings.search_paths);
        window.set_s_search_categories(settings.search_categories);
        window.set_s_search_tags(settings.search_tags);
        window.set_s_search_notes(settings.search_notes);
        window.set_s_search_aliases(settings.search_aliases);
        window.set_s_fuzzy(settings.fuzzy_matching);
        window.set_s_pinyin(settings.search_pinyin);
        window.set_s_english_initials(settings.search_english_initials);
        window.set_s_edit_distance(i32::from(settings.max_edit_distance));
        window.set_s_max_results(i32::from(settings.max_results));
        window.set_s_empty_query(settings.empty_query_strategy as i32);
        window.set_s_highlight(settings.highlight_results);
        window.set_s_recent_sort(settings.recent_sort_first);

        window.set_s_theme(settings.theme as i32);
        window.set_s_row_height(settings.row_height_preference as i32);
        window.set_s_search_width(i32::from(settings.search_window_width_or_default()));
        window.set_s_settings_width(i32::from(
            settings
                .settings_window_width
                .unwrap_or(filego::domain::settings::DEFAULT_SETTINGS_WIDTH),
        ));
        window.set_s_font_scale(i32::from(settings.font_scale_percent));
        window.set_s_show_path(settings.show_path_in_results);
        window.set_s_show_cat_tag(settings.show_category_tag_in_results);

        window.set_s_hotkey_runtime(match view.hotkey_runtime {
            filego::presentation::settings_controller::HotkeyRuntime::Disabled => 0,
            filego::presentation::settings_controller::HotkeyRuntime::Active => 1,
            filego::presentation::settings_controller::HotkeyRuntime::Paused => 2,
        });
        window.set_s_hotkey_text(hotkey_display_text(settings.hotkey).into());
        window.set_s_hotkey_error(
            view.hotkey_last_error
                .map(|error| hotkey_error_text(error, locale))
                .unwrap_or_default()
                .into(),
        );
        window.set_s_recording(
            view.hotkey_record_phase
                == filego::presentation::settings_controller::HotkeyRecordPhase::Listening,
        );
        window.set_s_record_draft(
            view.hotkey_draft
                .map(hotkey_combo_text)
                .unwrap_or_default()
                .into(),
        );

        window.set_s_data_location(self.data_dir.to_string_lossy().to_string().into());
        let folder_count = self
            .repo
            .borrow()
            .document()
            .map(|document| document.data.folders.len() as i32)
            .unwrap_or(0);
        window.set_s_folder_count(folder_count);
        window.set_s_backups(ModelRc::new(VecModel::from(
            view.backups
                .iter()
                .cloned()
                .map(slint::SharedString::from)
                .collect::<Vec<_>>(),
        )));
        let previewing = matches!(
            view.data_flow,
            filego::presentation::settings_controller::DataFlow::ImportPreview { .. }
        );
        window.set_s_import_preview(previewing);
        if let filego::presentation::settings_controller::DataFlow::ImportPreview { plan, mode } =
            &view.data_flow
        {
            window.set_s_import_added(plan.added.len() as i32);
            window.set_s_import_updated(plan.updated.len() as i32);
            window.set_s_import_skipped(plan.skipped.len() as i32);
            window.set_s_import_conflicts(plan.conflicts.len() as i32);
            window.set_s_import_mode(import_mode_index(*mode));
            window.set_s_import_added_names(ModelRc::new(VecModel::from(
                plan.added
                    .clone()
                    .into_iter()
                    .map(slint::SharedString::from)
                    .collect::<Vec<_>>(),
            )));
            window.set_s_import_updated_names(ModelRc::new(VecModel::from(
                plan.updated
                    .clone()
                    .into_iter()
                    .map(slint::SharedString::from)
                    .collect::<Vec<_>>(),
            )));
            window.set_s_import_skipped_names(ModelRc::new(VecModel::from(
                plan.skipped
                    .clone()
                    .into_iter()
                    .map(slint::SharedString::from)
                    .collect::<Vec<_>>(),
            )));
        } else {
            window.set_s_import_added(0);
            window.set_s_import_updated(0);
            window.set_s_import_skipped(0);
            window.set_s_import_conflicts(0);
            window.set_s_import_mode(0);
            window.set_s_import_added_names(ModelRc::new(VecModel::from(Vec::new())));
            window.set_s_import_updated_names(ModelRc::new(VecModel::from(Vec::new())));
            window.set_s_import_skipped_names(ModelRc::new(VecModel::from(Vec::new())));
        }

        window.set_s_about_version(filego::version::display().into());
        window.set_s_about_arch("x86-64".into());
        window.set_s_about_project("https://github.com/yorelll/filego".into());
        window.set_s_about_license("MIT".into());
        window.set_s_about_privacy(Msg::MonoAboutPrivacy.tr(locale).into());

        // Notice (localized; anonymous).
        let notice = view
            .notice
            .map(|notice| snotice_text(notice, locale))
            .unwrap_or_default();
        window.set_notice_text(notice.into());
    }

    /// Project the native hotkey machine's runtime state into the settings
    /// controller (Active/Paused/Disabled + last error), then refresh the UI.
    fn sync_hotkey_from_native(&mut self) {
        use filego::platform::hotkey::HotkeyState;
        let native = self.native.borrow();
        let runtime = match native.hotkey_state() {
            HotkeyState::Disabled => {
                filego::presentation::settings_controller::HotkeyRuntime::Disabled
            }
            HotkeyState::Active(_) => {
                filego::presentation::settings_controller::HotkeyRuntime::Active
            }
            HotkeyState::Paused(_) => {
                filego::presentation::settings_controller::HotkeyRuntime::Paused
            }
        };
        let error = native.hotkey_last_error();
        drop(native);
        self.settings
            .handle(SCommand::SyncHotkeyRuntime(runtime, error));
        self.sync_settings_ui();
    }

    /// Re-apply persisted appearance (theme + font scale + locale) on the open
    /// of the settings window so the UI matches the stored settings.
    fn refresh_settings_appearance(&mut self) {
        let settings = self.settings.view().settings.clone();
        let resolved = filego::presentation::theme::ResolvedTheme::resolve(
            settings.theme,
            filego::presentation::theme::ResolvedColorScheme::Light,
        );
        if let Some(window) = self.window.upgrade() {
            ui_set_theme_for_settings(&window, resolved);
            let scale = f32::from(settings.font_scale_percent) / 100.0;
            window.global::<UiTheme>().set_font_scale(scale);
        }
        if let (Some(width), Some(window)) = (settings.settings_window_width, self.window.upgrade())
        {
            apply_settings_window_width(&window, width);
        }
    }

    /// "打开数据目录": shell-open the data directory (reuses the folder-open
    /// boundary; never builds a command string).
    fn open_data_dir(&mut self) {
        let path = self.data_dir.to_string_lossy().to_string();
        if filego::platform::windows::tray_open::open_folder(&path).is_ok() {
            self.settings.set_notice(SNotice::DataDirOpened);
        } else {
            self.settings.set_notice(SNotice::DataDirOpenFailed);
        }
        self.settings.handle(SCommand::DismissDataFlow);
        self.sync_settings_ui();
    }

    /// Export: prompt a target file (the OS dialog enforces the overwrite
    /// prompt), write the versioned JSON document bytes, and notice the result.
    fn export_data(&mut self) {
        use filego::platform::windows::file_dialog::{FileDialogOutcome, pick_save_path};
        let Some(document) = self.settings.document() else {
            self.settings.set_notice(SNotice::ExportFailed);
            self.sync_settings_ui();
            return;
        };
        let outcome = pick_save_path("filego-export.json");
        let path = match outcome {
            FileDialogOutcome::Picked(path) => path,
            FileDialogOutcome::Cancelled => return,
            FileDialogOutcome::Failed => {
                self.settings.set_notice(SNotice::ExportFailed);
                self.sync_settings_ui();
                return;
            }
        };
        let bytes = match filego::storage::import_export::export_bytes(&document) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.settings.set_notice(SNotice::ExportFailed);
                self.sync_settings_ui();
                return;
            }
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => self.settings.set_notice(SNotice::ExportWritten),
            Err(_) => self.settings.set_notice(SNotice::ExportFailed),
        }
        self.sync_settings_ui();
    }

    /// Import: pick a file, parse + validate it, plan against the current
    /// document, and push the preview into the settings controller (nothing is
    /// applied yet — all-or-nothing apply happens on the explicit Preview Apply).
    fn import_data(&mut self) {
        use filego::platform::windows::file_dialog::{FileDialogOutcome, pick_open_path};
        use filego::storage::import_export::{self, ImportMode};
        let outcome = pick_open_path("json");
        let path = match outcome {
            FileDialogOutcome::Picked(path) => path,
            FileDialogOutcome::Cancelled => return,
            FileDialogOutcome::Failed => {
                self.settings.set_notice(SNotice::ImportPreviewFailed);
                self.sync_settings_ui();
                return;
            }
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.settings.set_notice(SNotice::ImportParseFailed);
                self.sync_settings_ui();
                return;
            }
        };
        let parsed = match import_export::parse_import(&bytes) {
            Ok(document) => document,
            Err(error) => {
                use filego::storage::import_export::ImportErrorKind;
                let notice = match error.kind {
                    ImportErrorKind::InvalidJson | ImportErrorKind::InvalidDocument => {
                        SNotice::ImportInvalidDocument
                    }
                    ImportErrorKind::FutureSchema => SNotice::ImportFutureSchema,
                    ImportErrorKind::MigrationNeeded => SNotice::ImportMigrationNeeded,
                    ImportErrorKind::UnresolvedReference => SNotice::ImportUnresolvedReference,
                    // `DuplicatePath` cannot arise from parsing a single
                    // standalone document (it is an apply-time union check),
                    // but the match must stay exhaustive.
                    ImportErrorKind::DuplicatePath => SNotice::ImportInvalidDocument,
                    ImportErrorKind::EncodeFailed => SNotice::ImportParseFailed,
                };
                self.settings.set_notice(notice);
                self.settings.handle(SCommand::DismissDataFlow);
                self.sync_settings_ui();
                return;
            }
        };
        let Some(current) = self.settings.document() else {
            self.settings.set_notice(SNotice::ImportPreviewFailed);
            self.sync_settings_ui();
            return;
        };
        let mode = self
            .window
            .upgrade()
            .map(|w| import_mode_from_index(w.get_s_import_mode()))
            .unwrap_or(ImportMode::Merge);
        let plan = import_export::plan_import(&current.data, &parsed.data, mode);
        self.pending_import = Some(parsed.data);
        self.settings.push_import_preview(plan, mode);
        self.sync_settings_ui();
    }

    /// Apply the previewed import (all-or-nothing): the union must re-validate
    /// (apply_import enforces it), then the repository document is replaced and
    /// saved atomically as one revision. Real directories are never touched.
    fn apply_import(&mut self) {
        use filego::storage::import_export;
        let Some(incoming) = self.pending_import.take() else {
            return;
        };
        let Some(current) = self.settings.document() else {
            return;
        };
        let mode = self
            .window
            .upgrade()
            .map(|w| import_mode_from_index(w.get_s_import_mode()))
            .unwrap_or(import_export::ImportMode::Merge);
        match import_export::apply_import(&current.data, &incoming, mode) {
            Ok((applied, _)) => {
                // M06 review M1: before an OVERWRITE-mode import actually
                // applies (mutates the document), take an explicit user-facing
                // backup snapshot of the CURRENT (pre-import) document, so the
                // user has a restorable point beyond the internal data.json.bak.
                // All-or-nothing still holds: this runs only right before the
                // apply, and a failed import never reaches this branch. The
                // snapshot reuses the standard backup list (visible + restorable
                // on the Data page) under a recognizable `before-import-` stamp.
                if mode == import_export::ImportMode::Overwrite {
                    let stamp = format!(
                        "before-import-{}",
                        chrono::Utc::now().format("%Y%m%d-%H%M%S")
                    );
                    let _ =
                        filego::storage::backup::create_backup(&self.data_dir, &current, &stamp);
                }
                let revision = {
                    let mut repo = self.repo.borrow_mut();
                    if repo.set_data(applied.data).is_err() {
                        self.settings.set_notice(SNotice::ImportUnresolvedReference);
                        self.sync_settings_ui();
                        return;
                    }
                    let _ = applied;
                    repo.save_at()
                };
                let revision_result = revision;
                // Refresh the Data-page backup list so the pre-import snapshot
                // (and any earlier manual backups) stay visible after the apply.
                let names =
                    filego::storage::backup::list_backups(&self.data_dir).unwrap_or_default();
                self.settings.set_backups(names);
                match revision_result {
                    Ok(_) => {
                        self.settings.handle(SCommand::ApplyImport);
                    }
                    Err(_) => {
                        self.settings.set_notice(SNotice::ImportPreviewFailed);
                    }
                }
            }
            Err(error) => {
                // M06 review M3: a refused overwrite-import (ambiguous path
                // collision) gets its own clear anonymous notice; other apply
                // failures keep the existing unresolved-reference notice.
                let notice = if error.kind == import_export::ImportErrorKind::DuplicatePath {
                    SNotice::ImportDuplicatePath
                } else {
                    SNotice::ImportUnresolvedReference
                };
                self.settings.set_notice(notice);
            }
        }
        self.sync_settings_ui();
        self.refresh_search_settings();
        // The manager's folder list re-reads the shared doc on next sync.
        self.manager.reload_from_store();
        self.sync_ui();
    }

    /// Restore a listed backup: decode it, validate, replace the working copy
    /// and save atomically. Never touches a real directory.
    fn restore_backup(&mut self, index: i32) {
        use filego::storage::backup;
        let backups = self.settings.view().backups.clone();
        let Some(name) = backups.get(index as usize).cloned() else {
            self.settings.set_notice(SNotice::BackupRestoreFailed);
            self.sync_settings_ui();
            return;
        };
        let decoded = match backup::read_backup(&self.data_dir, &name) {
            Ok(document) => document,
            Err(_) => {
                self.settings.set_notice(SNotice::BackupRestoreFailed);
                self.sync_settings_ui();
                return;
            }
        };
        let revision = {
            let mut repo = self.repo.borrow_mut();
            if repo.set_data(decoded.data).is_err() {
                self.settings.set_notice(SNotice::BackupRestoreFailed);
                self.sync_settings_ui();
                return;
            }
            repo.save_at()
        };
        match revision {
            Ok(_) => self.settings.set_notice(SNotice::BackupRestoreApplied),
            Err(_) => self.settings.set_notice(SNotice::BackupRestoreFailed),
        }
        self.sync_settings_ui();
        self.refresh_search_settings();
        self.manager.reload_from_store();
        self.sync_ui();
    }

    /// Create a user-facing backup snapshot (a new `backup-*.json` sibling).
    fn create_backup(&mut self) {
        use filego::storage::backup;
        let Some(document) = self.settings.document() else {
            self.settings.set_notice(SNotice::BackupRestoreFailed);
            self.sync_settings_ui();
            return;
        };
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        match backup::create_backup(&self.data_dir, &document, &stamp) {
            Ok(_) => {
                let names = backup::list_backups(&self.data_dir).unwrap_or_default();
                self.settings.set_backups(names);
                self.settings.set_notice(SNotice::BackupCreated);
            }
            Err(_) => self.settings.set_notice(SNotice::BackupRestoreFailed),
        }
        self.sync_settings_ui();
    }

    /// List the user-facing backups (newest first) and refresh the Data page.
    fn list_backups(&mut self) {
        use filego::storage::backup;
        match backup::list_backups(&self.data_dir) {
            Ok(names) => {
                self.settings.set_backups(names);
                if self.settings.view().backups.is_empty() {
                    self.settings.set_notice(SNotice::BackupsNone);
                }
            }
            Err(_) => self.settings.set_notice(SNotice::BackupListFailed),
        }
        self.sync_settings_ui();
    }

    /// "清除最近使用": clears the recently-used statistics of every folder
    /// record (open_count + last_opened_at reset) and saves. Search history is
    /// not persisted in 0.0.1, so there is nothing else to clear — the Data
    /// page states this honestly.
    fn clear_recent(&mut self) {
        let mut repo = self.repo.borrow_mut();
        let Some(document) = repo.document() else {
            return;
        };
        let mut document = document.clone();
        let mut any = false;
        for folder in &mut document.data.folders {
            if folder.open_count != 0 || folder.last_opened_at.is_some() {
                folder.open_count = 0;
                folder.last_opened_at = None;
                any = true;
            }
        }
        if !any {
            return;
        }
        if repo.set_data(document.data).is_err() || repo.save_at().is_err() {
            self.settings.set_notice(SNotice::SaveFailed);
            self.sync_settings_ui();
            return;
        }
        drop(repo);
        self.settings.set_notice(SNotice::Saved);
        self.sync_settings_ui();
        self.refresh_search_settings();
        self.sync_ui();
    }

    /// "重置全部设置": restore the default settings snapshot WITHOUT touching
    /// folder records; the settings changes are persisted and the UI re-applies
    /// theme/locale/font-scale/hotkey. This is the pure controller's
    /// `RestoreDefaultSettings`, routed through the adapter so the live
    /// platform effects are re-applied.
    fn reset_settings(&mut self) {
        self.settings.handle(SCommand::RestoreDefaultSettings);
        self.refresh_settings_appearance();
        self.refresh_search_settings();
        self.sync_settings_ui();
        self.sync_ui();
    }

    /// "删除全部文件夹记录": two-step-confirmed clear of EVERY folder RECORD.
    /// `clear_all_records` on the repository is record-only (the real folders
    /// stay on disk — the canary covers it); the document is saved atomically.
    fn clear_all_records(&mut self) {
        {
            let mut repo = self.repo.borrow_mut();
            let changed = repo.clear_all_records();
            if !changed {
                return;
            }
            if repo.save_at().is_err() {
                self.settings.set_notice(SNotice::SaveFailed);
                self.sync_settings_ui();
                return;
            }
        }
        self.settings.set_notice(SNotice::ClearAllApplied);
        self.sync_settings_ui();
        self.refresh_search_settings();
        self.manager.reload_from_store();
        self.sync_ui();
    }

    /// Drive a recorded hotkey key (from the Hotkey page FocusScope) through
    /// the pure controller; on a valid draft the adapter registers it with the
    /// native machine and keeps old-on-conflict.
    fn record_hotkey_key(&mut self, control: bool, alt: bool, shift: bool, win: bool, text: &str) {
        let Some(key) = filego::presentation::settings_controller::hotkey_from_text(text) else {
            return;
        };
        self.settings
            .handle(SCommand::SetRecordedModifiers(control, alt, shift, win));
        self.settings.handle(SCommand::FinishRecording(key));
        // A valid draft is then registered against the native machine; a
        // conflict keeps the old hotkey (machine semantics) and reports it.
        let draft = self.settings.view().hotkey_draft;
        if let Some(combo) = draft {
            let registered = self.native.borrow().set_hotkey(combo);
            match registered {
                Ok(()) => {
                    self.settings.handle(SCommand::PersistHotkey(Some(combo)));
                    self.settings.handle(SCommand::SyncHotkeyRuntime(
                        filego::presentation::settings_controller::HotkeyRuntime::Active,
                        None,
                    ));
                }
                Err(kind) => {
                    // Keep-old: the machine already restored the previous
                    // combo; report anonymous conflict/unavailable.
                    let notice = match kind {
                        filego::platform::hotkey::HotkeyErrorKind::Conflict => {
                            SNotice::HotkeyConflict
                        }
                        filego::platform::hotkey::HotkeyErrorKind::Unavailable => {
                            SNotice::HotkeyUnavailable
                        }
                    };
                    self.settings.set_notice(notice);
                    self.settings.handle(SCommand::SyncHotkeyRuntime(
                        filego::presentation::settings_controller::HotkeyRuntime::Disabled,
                        Some(kind),
                    ));
                    self.settings.handle(SCommand::CancelRecording);
                }
            }
        }
        self.sync_settings_ui();
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

        // NOTE: `page` is pushed by the nav handlers; the manager controls the
        // 1/2/3 range and the settings controller the 0/4..8 range. See
        // `on_command_show_page` / `on_s_show_page`.

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
        let (enabled, pinned, favorite): (Vec<bool>, Vec<bool>, Vec<bool>) = view
            .rows
            .iter()
            .map(|r| (r.enabled, r.pinned, r.favorite))
            .collect();
        window.set_rows_enabled(ModelRc::new(VecModel::from(enabled)));
        window.set_rows_pinned(ModelRc::new(VecModel::from(pinned)));
        window.set_rows_favorite(ModelRc::new(VecModel::from(favorite)));

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

        // M05 review M1: the persisted one-level-import enablement (default OFF).
        window.set_one_level_import_on(view.one_level_import_setting);

        // M05 review M2: folder-list filter + sort selector state.
        window.set_filter_category_model(string_model(
            std::iter::once(Msg::FilterStatusAll.tr(locale))
                .chain(std::iter::once(Msg::Uncategorized.tr(locale)))
                .chain(view.categories.iter().map(|c| c.name.clone())),
        ));
        let category_index = match view.filter.category {
            None => 0,
            Some(id) if id == filego::domain::ids::CategoryId::from_uuid(uuid::Uuid::nil()) => 1,
            Some(id) => {
                2 + view
                    .categories
                    .iter()
                    .position(|c| c.id == id)
                    .map_or(usize::MAX, |p| p)
            }
        };
        window.set_filter_category_index(category_index.min(1 + view.categories.len()) as i32);
        window.set_filter_status_index(match view.filter.enabled {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        });
        window.set_sort_index(match view.sort {
            filego::presentation::management::FolderSort::Name => 0,
            filego::presentation::management::FolderSort::RecentlyUsed => 1,
            filego::presentation::management::FolderSort::AddedTime => 2,
        });

        // Draft dialog.
        match &view.add_flow {
            filego::presentation::manager::AddFlowView::Draft(draft) => {
                window.set_draft_visible(true);
                window.set_batch_visible(false);
                window.set_draft_name(draft.display_name.clone().into());
                window.set_draft_path(draft.path.clone().into());
                window.set_draft_note(draft.note.clone().into());
                window.set_draft_weight(i32::from(draft.manual_weight));
                let title = if draft.id.is_some() {
                    Msg::EditDialogTitle
                } else {
                    Msg::AddDialogTitle
                };
                window.set_draft_title(title.tr(locale).into());
                let duplicate = draft.duplicate_existing.is_some();
                let error = if !draft.valid {
                    Msg::NoticeInvalidPath.tr(locale)
                } else if duplicate {
                    format!(
                        "{}: {}",
                        Msg::NoticeDuplicateBlocked.tr(locale),
                        draft.duplicate_existing.as_deref().unwrap_or_default()
                    )
                } else {
                    String::new()
                };
                window.set_draft_error(error.into());
                window.set_draft_duplicate(duplicate);
                window.set_draft_unsaved(draft.unsaved);
                window.set_draft_enabled(draft.enabled);
                // The draft color (editable): resolve the stored color via the
                // palette, mirroring `cycle_draft_color` so the swatch matches.
                window.set_draft_color(draft_color_to_slint(draft));
                // Category select (index 0 = uncategorized, 1+k = category k).
                window.set_draft_categories_model(string_model(
                    std::iter::once(String::new())
                        .chain(view.categories.iter().map(|c| c.name.clone())),
                ));
                let category_index = match draft.category_id {
                    None => 0,
                    Some(id) => {
                        1 + view
                            .categories
                            .iter()
                            .position(|c| c.id == id)
                            .map_or(0, |p| p + 1)
                    }
                };
                window.set_draft_category_index(
                    (category_index).clamp(0, view.categories.len()) as i32
                );
                // Tag multi-select (names + on/off state parallel arrays).
                window.set_draft_tags_model(string_model(view.tags.iter().map(|t| t.name.clone())));
                window.set_draft_tags_state(ModelRc::new(VecModel::from(
                    view.tags
                        .iter()
                        .map(|t| draft.tag_ids.contains(&t.id))
                        .collect::<Vec<bool>>(),
                )));
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

/// Resolve the draft color for the swatch (M05 review H2). `None` (no color
/// yet) renders the default accent; a stored palette color renders as-is so the
/// shown swatch always matches the value `CycleDraftColor` will preserve.
fn draft_color_to_slint(draft: &filego::presentation::manager::FolderDraftView) -> slint::Color {
    match draft.color {
        Some(color) => slint::Color::from_argb_encoded(color.0),
        None => slint::Color::from_argb_encoded(0xFF25_63EB),
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

/// Localize an M06 settings notice (anonymous — never a path or query).
fn snotice_text(
    notice: filego::presentation::settings_controller::SNotice,
    locale: filego::presentation::i18n::Locale,
) -> String {
    let msg = match notice {
        filego::presentation::settings_controller::SNotice::Saved => Msg::NoticeSaved,
        filego::presentation::settings_controller::SNotice::SaveFailed => Msg::NoticeSaveFailed,
        filego::presentation::settings_controller::SNotice::StartupWriteFailed => {
            Msg::MonoNoticeStartupWriteFailed
        }
        filego::presentation::settings_controller::SNotice::StartupReadFailed => {
            Msg::MonoNoticeStartupReadFailed
        }
        filego::presentation::settings_controller::SNotice::HotkeyConflict => Msg::NoticeSaveFailed,
        filego::presentation::settings_controller::SNotice::HotkeyUnavailable => {
            Msg::NoticeSaveFailed
        }
        filego::presentation::settings_controller::SNotice::HotkeyInvalid => Msg::NoticeSaveFailed,
        filego::presentation::settings_controller::SNotice::ImportParseFailed => {
            Msg::MonoNoticeImportParseFailed
        }
        filego::presentation::settings_controller::SNotice::ImportFutureSchema => {
            Msg::MonoNoticeImportFutureSchema
        }
        filego::presentation::settings_controller::SNotice::ImportMigrationNeeded => {
            Msg::MonoNoticeImportMigrationNeeded
        }
        filego::presentation::settings_controller::SNotice::ImportInvalidDocument => {
            Msg::MonoNoticeImportInvalidDocument
        }
        filego::presentation::settings_controller::SNotice::ImportUnresolvedReference => {
            Msg::MonoNoticeImportUnresolvedReference
        }
        filego::presentation::settings_controller::SNotice::ImportApplied => {
            Msg::MonoNoticeImportApplied
        }
        filego::presentation::settings_controller::SNotice::ImportPreviewFailed => {
            Msg::MonoNoticeImportPreviewFailed
        }
        filego::presentation::settings_controller::SNotice::ExportFailed => {
            Msg::MonoNoticeExportFailed
        }
        filego::presentation::settings_controller::SNotice::ImportDuplicatePath => {
            Msg::MonoNoticeImportDuplicatePath
        }
        filego::presentation::settings_controller::SNotice::BackupsNone => {
            Msg::MonoNoticeBackupsNone
        }
        filego::presentation::settings_controller::SNotice::BackupCreated => {
            Msg::MonoNoticeBackupCreated
        }
        filego::presentation::settings_controller::SNotice::BackupRestoreFailed => {
            Msg::MonoNoticeBackupRestoreFailed
        }
        filego::presentation::settings_controller::SNotice::BackupRestoreApplied => {
            Msg::MonoNoticeBackupRestoreApplied
        }
        filego::presentation::settings_controller::SNotice::BackupListFailed => {
            Msg::MonoNoticeBackupListFailed
        }
        filego::presentation::settings_controller::SNotice::ResetDefaultApplied => {
            Msg::MonoNoticeResetDefaultApplied
        }
        filego::presentation::settings_controller::SNotice::ClearAllApplied => {
            Msg::MonoNoticeClearAllApplied
        }
        filego::presentation::settings_controller::SNotice::DataDirOpened => {
            Msg::MonoNoticeDataDirOpened
        }
        filego::presentation::settings_controller::SNotice::DataDirOpenFailed => {
            Msg::MonoNoticeDataDirOpenFailed
        }
        filego::presentation::settings_controller::SNotice::ExportWritten => {
            Msg::MonoNoticeExportWritten
        }
    };
    msg.tr(locale)
}

/// The UI locale for a language preference (M06.2). `System` follows the OS
/// default through `Locale::detect` (no live system-locale notification hook in
/// 0.0.1; the preference is persisted and re-applied on each open).
fn locale_for(preference: filego::domain::settings::LanguagePreference) -> Locale {
    match preference {
        filego::domain::settings::LanguagePreference::System => Locale::detect(None),
        filego::domain::settings::LanguagePreference::ZhCN => Locale::ZhCN,
        filego::domain::settings::LanguagePreference::EnUS => Locale::EnUS,
    }
}

/// Render a hotkey setting as a human string (for the Hotkey page + the tray).
fn hotkey_combo_text(setting: filego::domain::settings::HotkeySetting) -> String {
    let mods = setting.modifiers;
    let mut parts: Vec<String> = Vec::new();
    if mods.control {
        parts.push("Ctrl".into());
    }
    if mods.alt {
        parts.push("Alt".into());
    }
    if mods.shift {
        parts.push("Shift".into());
    }
    if mods.win {
        parts.push("Win".into());
    }
    let key = hotkey_key_text(setting.key);
    parts.push(key);
    parts.join("+")
}

fn hotkey_key_text(key: filego::domain::settings::HotkeyKey) -> String {
    match key {
        filego::domain::settings::HotkeyKey::Vk { vk: 0x20 } => "Space".into(),
        filego::domain::settings::HotkeyKey::Vk { vk } => {
            if vk.is_ascii_uppercase() {
                (vk as char).to_string()
            } else {
                format!("VK-{vk}")
            }
        }
        filego::domain::settings::HotkeyKey::Function { index } => format!("F{index}"),
    }
}

fn hotkey_display_text(setting: Option<filego::domain::settings::HotkeySetting>) -> String {
    setting
        .map(hotkey_combo_text)
        .unwrap_or_else(|| Msg::MonoHotkeyNone.tr(Locale::ZhCN))
}

/// Localize a hotkey error (anonymous kind).
fn hotkey_error_text(kind: filego::platform::hotkey::HotkeyErrorKind, _locale: Locale) -> String {
    kind.as_detail().to_owned()
}

/// The import-mode combo index for a mode.
fn import_mode_index(mode: filego::storage::import_export::ImportMode) -> i32 {
    match mode {
        filego::storage::import_export::ImportMode::Overwrite => 0,
        filego::storage::import_export::ImportMode::Merge => 1,
        filego::storage::import_export::ImportMode::SkipDuplicates => 2,
    }
}

fn import_mode_from_index(index: i32) -> filego::storage::import_export::ImportMode {
    match index {
        1 => filego::storage::import_export::ImportMode::Merge,
        2 => filego::storage::import_export::ImportMode::SkipDuplicates,
        _ => filego::storage::import_export::ImportMode::Overwrite,
    }
}

/// Push the M06 theme tokens into the settings window's global (shared with the
/// main window in the same compiled module).
fn ui_set_theme_for_settings(
    window: &SettingsWindow,
    theme: filego::presentation::theme::ResolvedTheme,
) {
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

/// Apply the persisted settings-window width override (physical px = logical ×
/// the window's current scale; Slint 1.18 has no logical `set_width`).
fn apply_settings_window_width(window: &SettingsWindow, logical_width: u16) {
    let scale = window.window().scale_factor();
    let physical = slint::PhysicalSize::new(
        (f32::from(logical_width) * scale).round() as u32,
        window.window().size().height,
    );
    window.window().set_size(physical);
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
        // M05 review H4: the per-row enable/disable toggle label is bound in
        // Slint to the row state (context-menu.disable/enable); this static
        // string is now only a fallback and must never read "添加".
        ("action_toggle_enabled", Msg::DisabledLabel),
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
        // M7: the import hover is a `max 100` hint, not a repeat of the
        // children button label.
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
        // M05 review M2: filter + sort selector labels.
        ("filter_status_all", Msg::FilterStatusAll),
        ("filter_status_enabled", Msg::FilterStatusEnabled),
        ("filter_status_disabled", Msg::FilterStatusDisabled),
        ("sort_name", Msg::SortName),
        ("sort_recent", Msg::SortRecent),
        ("sort_added", Msg::SortAdded),
        // M05 review H2: add/edit dialog field labels + duplicate resolution.
        ("field_note", Msg::FieldNote),
        ("field_weight", Msg::FieldWeight),
        ("field_category", Msg::FieldCategory),
        ("field_tags", Msg::FieldTags),
        ("field_color", Msg::FieldColor),
        ("duplicate_cancel", Msg::DuplicateCancel),
        ("duplicate_edit_existing", Msg::DuplicateEditExisting),
        ("duplicate_save_different", Msg::DuplicateSaveDifferent),
        ("unsaved_changes", Msg::UnsavedChanges),
        ("discard_changes", Msg::DiscardChanges),
        ("discard_no", Msg::DiscardNo),
        // M05 review H3: category/tag delete confirmation.
        ("confirm_delete_category", Msg::ConfirmDeleteCategory),
        ("confirm_delete_tag", Msg::ConfirmDeleteTag),
        // ---- M06 settings pages ----
        ("settings_general", Msg::SettingsGeneral),
        ("settings_search", Msg::SettingsSearch),
        ("settings_appearance", Msg::SettingsAppearance),
        ("settings_hotkey", Msg::SettingsHotkey),
        ("settings_data", Msg::SettingsData),
        ("settings_about", Msg::SettingsAbout),
        ("settings_mono_launch_at_login", Msg::MonoLaunchAtLogin),
        ("settings_mono_silent_start", Msg::MonoSilentStart),
        ("settings_mono_show_main", Msg::MonoShowMainWindowAtStartup),
        (
            "settings_mono_start_notification",
            Msg::MonoStartNotification,
        ),
        (
            "settings_mono_start_notification_none",
            Msg::MonoStartNotificationNone,
        ),
        ("settings_mono_hide_after_open", Msg::MonoHideAfterOpen),
        ("settings_mono_clear_after_open", Msg::MonoClearAfterOpen),
        ("settings_mono_hide_on_focus_loss", Msg::MonoHideOnFocusLoss),
        ("settings_mono_monitor_strategy", Msg::MonoMonitorStrategy),
        ("settings_mono_monitor_mouse", Msg::MonoMonitorMouse),
        (
            "settings_mono_monitor_active_window",
            Msg::MonoMonitorActiveWindow,
        ),
        ("settings_mono_language", Msg::MonoLanguage),
        ("settings_mono_language_system", Msg::MonoLanguageSystem),
        ("settings_mono_language_zhcn", Msg::MonoLanguageZhCN),
        ("settings_mono_language_enus", Msg::MonoLanguageEnUS),
        ("settings_mono_restore_defaults", Msg::MonoRestoreDefaults),
        (
            "settings_mono_restore_defaults_impact",
            Msg::MonoRestoreDefaultsImpact,
        ),
        (
            "settings_mono_restore_defaults_confirm",
            Msg::MonoRestoreDefaultsConfirm,
        ),
        ("settings_search_paths", Msg::MonoSearchPaths),
        ("settings_search_categories", Msg::MonoSearchCategories),
        ("settings_search_tags", Msg::MonoSearchTags),
        ("settings_search_notes", Msg::MonoSearchNotes),
        ("settings_search_aliases", Msg::MonoSearchAliases),
        ("settings_search_fuzzy", Msg::MonoFuzzyMatching),
        ("settings_search_pinyin", Msg::MonoSearchPinyin),
        (
            "settings_search_english_initials",
            Msg::MonoSearchEnglishInitials,
        ),
        (
            "settings_search_max_edit_distance",
            Msg::MonoMaxEditDistance,
        ),
        ("settings_search_max_results", Msg::MonoMaxResults),
        ("settings_search_empty_query", Msg::MonoEmptyQueryStrategy),
        (
            "settings_search_empty_query_favorites_first",
            Msg::MonoEmptyQueryFavoritesFirst,
        ),
        ("settings_search_empty_query_all", Msg::MonoEmptyQueryAll),
        (
            "settings_search_empty_query_pinned_only",
            Msg::MonoEmptyQueryPinnedOnly,
        ),
        (
            "settings_search_empty_query_blank",
            Msg::MonoEmptyQueryBlank,
        ),
        ("settings_search_highlight", Msg::MonoHighlightResults),
        ("settings_search_recent_sort", Msg::MonoRecentSort),
        ("settings_search_recent_sort_hint", Msg::MonoRecentSortHint),
        (
            "settings_search_history_future",
            Msg::MonoSearchHistoryFuture,
        ),
        ("settings_appearance_theme", Msg::MonoTheme),
        ("settings_appearance_theme_system", Msg::MonoThemeSystem),
        ("settings_appearance_theme_light", Msg::MonoThemeLight),
        ("settings_appearance_theme_dark", Msg::MonoThemeDark),
        ("settings_appearance_row_height", Msg::MonoRowHeight),
        (
            "settings_appearance_row_height_compact",
            Msg::MonoRowHeightCompact,
        ),
        (
            "settings_appearance_row_height_standard",
            Msg::MonoRowHeightStandard,
        ),
        (
            "settings_appearance_search_width",
            Msg::MonoSearchWindowWidth,
        ),
        (
            "settings_appearance_settings_width",
            Msg::MonoSettingsWindowWidth,
        ),
        ("settings_appearance_font_scale", Msg::MonoFontScale),
        ("settings_appearance_show_path", Msg::MonoShowPathInResults),
        (
            "settings_appearance_show_cat_tag",
            Msg::MonoShowCategoryTagInResults,
        ),
        ("settings_appearance_transparency", Msg::MonoTransparency),
        ("settings_appearance_high_contrast", Msg::MonoHighContrast),
        ("settings_appearance_future_version", Msg::MonoFutureVersion),
        ("settings_hotkey_current", Msg::MonoHotkeyCurrentCombo),
        ("settings_hotkey_none", Msg::MonoHotkeyNone),
        ("settings_hotkey_record", Msg::MonoHotkeyRecord),
        (
            "settings_hotkey_recording_hint",
            Msg::MonoHotkeyRecordingHint,
        ),
        (
            "settings_hotkey_cancel_recording",
            Msg::MonoHotkeyCancelRecording,
        ),
        ("settings_hotkey_pause", Msg::MonoHotkeyPause),
        ("settings_hotkey_resume", Msg::MonoHotkeyResume),
        ("settings_hotkey_clear", Msg::MonoHotkeyClear),
        (
            "settings_hotkey_restore_default",
            Msg::MonoHotkeyRestoreDefault,
        ),
        (
            "settings_hotkey_state_disabled",
            Msg::MonoHotkeyStateDisabled,
        ),
        ("settings_hotkey_state_active", Msg::MonoHotkeyStateActive),
        ("settings_hotkey_state_paused", Msg::MonoHotkeyStatePaused),
        ("settings_data_location", Msg::MonoDataLocation),
        ("settings_data_open_dir", Msg::MonoOpenDataDir),
        ("settings_data_folder_count", Msg::MonoFolderCount),
        ("settings_data_export", Msg::MonoExport),
        ("settings_data_import", Msg::MonoImport),
        (
            "settings_data_import_preview_title",
            Msg::MonoImportPreviewTitle,
        ),
        ("settings_data_import_added", Msg::MonoImportAdded),
        ("settings_data_import_updated", Msg::MonoImportUpdated),
        ("settings_data_import_skipped", Msg::MonoImportSkipped),
        ("settings_data_import_conflicts", Msg::MonoImportConflicts),
        ("settings_data_import_mode", Msg::MonoImportMode),
        (
            "settings_data_import_mode_overwrite",
            Msg::MonoImportModeOverwrite,
        ),
        ("settings_data_import_mode_merge", Msg::MonoImportModeMerge),
        ("settings_data_import_mode_skip", Msg::MonoImportModeSkip),
        ("settings_data_import_apply", Msg::MonoImportApply),
        ("settings_data_import_cancel", Msg::MonoImportCancel),
        ("settings_data_backup_create", Msg::MonoBackupCreate),
        ("settings_data_backup_list", Msg::MonoBackupList),
        ("settings_data_backup_restore", Msg::MonoBackupRestore),
        ("settings_data_backup_none", Msg::MonoBackupNone),
        ("settings_data_clear_recent", Msg::MonoClearRecentlyUsed),
        (
            "settings_data_clear_recent_note",
            Msg::MonoClearRecentlyUsedNote,
        ),
        ("settings_data_reset_settings", Msg::MonoResetSettings),
        (
            "settings_data_reset_settings_confirm",
            Msg::MonoResetSettingsConfirm,
        ),
        ("settings_data_clear_all_records", Msg::MonoClearAllRecords),
        (
            "settings_data_clear_all_records_confirm",
            Msg::MonoClearAllRecordsConfirm,
        ),
        (
            "settings_data_clear_all_never_deletes",
            Msg::MonoClearAllNeverDeletes,
        ),
        ("settings_about_version", Msg::MonoAboutVersion),
        ("settings_about_arch", Msg::MonoAboutArch),
        ("settings_about_project", Msg::MonoAboutProject),
        ("settings_about_license", Msg::MonoAboutLicense),
        ("settings_about_privacy", Msg::MonoAboutPrivacy),
        ("settings_category_rename", Msg::MonoCategoryRename),
        ("settings_tag_rename", Msg::MonoTagRename),
        ("settings_tag_merge_into", Msg::MonoTagMergeInto),
        ("settings_tag_merge_confirm", Msg::MonoMergeConfirm),
        (
            "settings_notice_startup_write_failed",
            Msg::MonoNoticeStartupWriteFailed,
        ),
        (
            "settings_notice_startup_read_failed",
            Msg::MonoNoticeStartupReadFailed,
        ),
        (
            "settings_notice_data_dir_open_failed",
            Msg::MonoNoticeDataDirOpenFailed,
        ),
        (
            "settings_notice_import_parse_failed",
            Msg::MonoNoticeImportParseFailed,
        ),
        (
            "settings_notice_import_future_schema",
            Msg::MonoNoticeImportFutureSchema,
        ),
        (
            "settings_notice_import_migration_needed",
            Msg::MonoNoticeImportMigrationNeeded,
        ),
        (
            "settings_notice_import_invalid_document",
            Msg::MonoNoticeImportInvalidDocument,
        ),
        (
            "settings_notice_import_unresolved_reference",
            Msg::MonoNoticeImportUnresolvedReference,
        ),
        (
            "settings_notice_import_applied",
            Msg::MonoNoticeImportApplied,
        ),
        (
            "settings_notice_import_preview_failed",
            Msg::MonoNoticeImportPreviewFailed,
        ),
        ("settings_notice_export_failed", Msg::MonoNoticeExportFailed),
        (
            "settings_notice_import_duplicate_path",
            Msg::MonoNoticeImportDuplicatePath,
        ),
        ("settings_notice_backups_none", Msg::MonoNoticeBackupsNone),
        (
            "settings_notice_backup_created",
            Msg::MonoNoticeBackupCreated,
        ),
        (
            "settings_notice_backup_restore_failed",
            Msg::MonoNoticeBackupRestoreFailed,
        ),
        (
            "settings_notice_backup_restore_applied",
            Msg::MonoNoticeBackupRestoreApplied,
        ),
        (
            "settings_notice_backup_list_failed",
            Msg::MonoNoticeBackupListFailed,
        ),
        (
            "settings_notice_reset_default_applied",
            Msg::MonoNoticeResetDefaultApplied,
        ),
        (
            "settings_notice_clear_all_applied",
            Msg::MonoNoticeClearAllApplied,
        ),
        (
            "settings_notice_data_dir_opened",
            Msg::MonoNoticeDataDirOpened,
        ),
        (
            "settings_notice_export_written",
            Msg::MonoNoticeExportWritten,
        ),
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
        "filter_status_all" => strings.set_filter_status_all(value),
        "filter_status_enabled" => strings.set_filter_status_enabled(value),
        "filter_status_disabled" => strings.set_filter_status_disabled(value),
        "sort_name" => strings.set_sort_name(value),
        "sort_recent" => strings.set_sort_recent(value),
        "sort_added" => strings.set_sort_added(value),
        "field_note" => strings.set_field_note(value),
        "field_weight" => strings.set_field_weight(value),
        "field_category" => strings.set_field_category(value),
        "field_tags" => strings.set_field_tags(value),
        "field_color" => strings.set_field_color(value),
        "duplicate_cancel" => strings.set_duplicate_cancel(value),
        "duplicate_edit_existing" => strings.set_duplicate_edit_existing(value),
        "duplicate_save_different" => strings.set_duplicate_save_different(value),
        "unsaved_changes" => strings.set_unsaved_changes(value),
        "discard_changes" => strings.set_discard_changes(value),
        "discard_no" => strings.set_discard_no(value),
        "confirm_delete_category" => strings.set_confirm_delete_category(value),
        "confirm_delete_tag" => strings.set_confirm_delete_tag(value),
        // ---- M06 ----
        "settings_general" => strings.set_settings_general(value),
        "settings_search" => strings.set_settings_search(value),
        "settings_appearance" => strings.set_settings_appearance(value),
        "settings_hotkey" => strings.set_settings_hotkey(value),
        "settings_data" => strings.set_settings_data(value),
        "settings_about" => strings.set_settings_about(value),
        "settings_mono_launch_at_login" => strings.set_settings_mono_launch_at_login(value),
        "settings_mono_silent_start" => strings.set_settings_mono_silent_start(value),
        "settings_mono_show_main" => strings.set_settings_mono_show_main(value),
        "settings_mono_start_notification" => strings.set_settings_mono_start_notification(value),
        "settings_mono_start_notification_none" => {
            strings.set_settings_mono_start_notification_none(value)
        }
        "settings_mono_hide_after_open" => strings.set_settings_mono_hide_after_open(value),
        "settings_mono_clear_after_open" => strings.set_settings_mono_clear_after_open(value),
        "settings_mono_hide_on_focus_loss" => strings.set_settings_mono_hide_on_focus_loss(value),
        "settings_mono_monitor_strategy" => strings.set_settings_mono_monitor_strategy(value),
        "settings_mono_monitor_mouse" => strings.set_settings_mono_monitor_mouse(value),
        "settings_mono_monitor_active_window" => {
            strings.set_settings_mono_monitor_active_window(value)
        }
        "settings_mono_language" => strings.set_settings_mono_language(value),
        "settings_mono_language_system" => strings.set_settings_mono_language_system(value),
        "settings_mono_language_zhcn" => strings.set_settings_mono_language_zhcn(value),
        "settings_mono_language_enus" => strings.set_settings_mono_language_enus(value),
        "settings_mono_restore_defaults" => strings.set_settings_mono_restore_defaults(value),
        "settings_mono_restore_defaults_impact" => {
            strings.set_settings_mono_restore_defaults_impact(value)
        }
        "settings_mono_restore_defaults_confirm" => {
            strings.set_settings_mono_restore_defaults_confirm(value)
        }
        "settings_search_paths" => strings.set_settings_search_paths(value),
        "settings_search_categories" => strings.set_settings_search_categories(value),
        "settings_search_tags" => strings.set_settings_search_tags(value),
        "settings_search_notes" => strings.set_settings_search_notes(value),
        "settings_search_aliases" => strings.set_settings_search_aliases(value),
        "settings_search_fuzzy" => strings.set_settings_search_fuzzy(value),
        "settings_search_pinyin" => strings.set_settings_search_pinyin(value),
        "settings_search_english_initials" => strings.set_settings_search_english_initials(value),
        "settings_search_max_edit_distance" => strings.set_settings_search_max_edit_distance(value),
        "settings_search_max_results" => strings.set_settings_search_max_results(value),
        "settings_search_empty_query" => strings.set_settings_search_empty_query(value),
        "settings_search_empty_query_favorites_first" => {
            strings.set_settings_search_empty_query_favorites_first(value)
        }
        "settings_search_empty_query_all" => strings.set_settings_search_empty_query_all(value),
        "settings_search_empty_query_pinned_only" => {
            strings.set_settings_search_empty_query_pinned_only(value)
        }
        "settings_search_empty_query_blank" => strings.set_settings_search_empty_query_blank(value),
        "settings_search_highlight" => strings.set_settings_search_highlight(value),
        "settings_search_recent_sort" => strings.set_settings_search_recent_sort(value),
        "settings_search_recent_sort_hint" => strings.set_settings_search_recent_sort_hint(value),
        "settings_search_history_future" => strings.set_settings_search_history_future(value),
        "settings_appearance_theme" => strings.set_settings_appearance_theme(value),
        "settings_appearance_theme_system" => strings.set_settings_appearance_theme_system(value),
        "settings_appearance_theme_light" => strings.set_settings_appearance_theme_light(value),
        "settings_appearance_theme_dark" => strings.set_settings_appearance_theme_dark(value),
        "settings_appearance_row_height" => strings.set_settings_appearance_row_height(value),
        "settings_appearance_row_height_compact" => {
            strings.set_settings_appearance_row_height_compact(value)
        }
        "settings_appearance_row_height_standard" => {
            strings.set_settings_appearance_row_height_standard(value)
        }
        "settings_appearance_search_width" => strings.set_settings_appearance_search_width(value),
        "settings_appearance_settings_width" => {
            strings.set_settings_appearance_settings_width(value)
        }
        "settings_appearance_font_scale" => strings.set_settings_appearance_font_scale(value),
        "settings_appearance_show_path" => strings.set_settings_appearance_show_path(value),
        "settings_appearance_show_cat_tag" => strings.set_settings_appearance_show_cat_tag(value),
        "settings_appearance_transparency" => strings.set_settings_appearance_transparency(value),
        "settings_appearance_high_contrast" => strings.set_settings_appearance_high_contrast(value),
        "settings_appearance_future_version" => {
            strings.set_settings_appearance_future_version(value)
        }
        "settings_hotkey_current" => strings.set_settings_hotkey_current(value),
        "settings_hotkey_none" => strings.set_settings_hotkey_none(value),
        "settings_hotkey_record" => strings.set_settings_hotkey_record(value),
        "settings_hotkey_recording_hint" => strings.set_settings_hotkey_recording_hint(value),
        "settings_hotkey_cancel_recording" => strings.set_settings_hotkey_cancel_recording(value),
        "settings_hotkey_pause" => strings.set_settings_hotkey_pause(value),
        "settings_hotkey_resume" => strings.set_settings_hotkey_resume(value),
        "settings_hotkey_clear" => strings.set_settings_hotkey_clear(value),
        "settings_hotkey_restore_default" => strings.set_settings_hotkey_restore_default(value),
        "settings_hotkey_state_disabled" => strings.set_settings_hotkey_state_disabled(value),
        "settings_hotkey_state_active" => strings.set_settings_hotkey_state_active(value),
        "settings_hotkey_state_paused" => strings.set_settings_hotkey_state_paused(value),
        "settings_data_location" => strings.set_settings_data_location(value),
        "settings_data_open_dir" => strings.set_settings_data_open_dir(value),
        "settings_data_folder_count" => strings.set_settings_data_folder_count(value),
        "settings_data_export" => strings.set_settings_data_export(value),
        "settings_data_import" => strings.set_settings_data_import(value),
        "settings_data_import_preview_title" => {
            strings.set_settings_data_import_preview_title(value)
        }
        "settings_data_import_added" => strings.set_settings_data_import_added(value),
        "settings_data_import_updated" => strings.set_settings_data_import_updated(value),
        "settings_data_import_skipped" => strings.set_settings_data_import_skipped(value),
        "settings_data_import_conflicts" => strings.set_settings_data_import_conflicts(value),
        "settings_data_import_mode" => strings.set_settings_data_import_mode(value),
        "settings_data_import_mode_overwrite" => {
            strings.set_settings_data_import_mode_overwrite(value)
        }
        "settings_data_import_mode_merge" => strings.set_settings_data_import_mode_merge(value),
        "settings_data_import_mode_skip" => strings.set_settings_data_import_mode_skip(value),
        "settings_data_import_apply" => strings.set_settings_data_import_apply(value),
        "settings_data_import_cancel" => strings.set_settings_data_import_cancel(value),
        "settings_data_backup_create" => strings.set_settings_data_backup_create(value),
        "settings_data_backup_list" => strings.set_settings_data_backup_list(value),
        "settings_data_backup_restore" => strings.set_settings_data_backup_restore(value),
        "settings_data_backup_none" => strings.set_settings_data_backup_none(value),
        "settings_data_clear_recent" => strings.set_settings_data_clear_recent(value),
        "settings_data_clear_recent_note" => strings.set_settings_data_clear_recent_note(value),
        "settings_data_reset_settings" => strings.set_settings_data_reset_settings(value),
        "settings_data_reset_settings_confirm" => {
            strings.set_settings_data_reset_settings_confirm(value)
        }
        "settings_data_clear_all_records" => strings.set_settings_data_clear_all_records(value),
        "settings_data_clear_all_records_confirm" => {
            strings.set_settings_data_clear_all_records_confirm(value)
        }
        "settings_data_clear_all_never_deletes" => {
            strings.set_settings_data_clear_all_never_deletes(value)
        }
        "settings_about_version" => strings.set_settings_about_version(value),
        "settings_about_arch" => strings.set_settings_about_arch(value),
        "settings_about_project" => strings.set_settings_about_project(value),
        "settings_about_license" => strings.set_settings_about_license(value),
        "settings_about_privacy" => strings.set_settings_about_privacy(value),
        "settings_category_rename" => strings.set_settings_category_rename(value),
        "settings_tag_rename" => strings.set_settings_tag_rename(value),
        "settings_tag_merge_into" => strings.set_settings_tag_merge_into(value),
        "settings_tag_merge_confirm" => strings.set_settings_tag_merge_confirm(value),
        "settings_notice_startup_write_failed" => {
            strings.set_settings_notice_startup_write_failed(value)
        }
        "settings_notice_startup_read_failed" => {
            strings.set_settings_notice_startup_read_failed(value)
        }
        "settings_notice_data_dir_open_failed" => {
            strings.set_settings_notice_data_dir_open_failed(value)
        }
        "settings_notice_import_parse_failed" => {
            strings.set_settings_notice_import_parse_failed(value)
        }
        "settings_notice_import_future_schema" => {
            strings.set_settings_notice_import_future_schema(value)
        }
        "settings_notice_import_migration_needed" => {
            strings.set_settings_notice_import_migration_needed(value)
        }
        "settings_notice_import_invalid_document" => {
            strings.set_settings_notice_import_invalid_document(value)
        }
        "settings_notice_import_unresolved_reference" => {
            strings.set_settings_notice_import_unresolved_reference(value)
        }
        "settings_notice_import_applied" => strings.set_settings_notice_import_applied(value),
        "settings_notice_import_preview_failed" => {
            strings.set_settings_notice_import_preview_failed(value)
        }
        "settings_notice_export_failed" => strings.set_settings_notice_export_failed(value),
        "settings_notice_import_duplicate_path" => {
            strings.set_settings_notice_import_duplicate_path(value)
        }
        "settings_notice_backups_none" => strings.set_settings_notice_backups_none(value),
        "settings_notice_backup_created" => strings.set_settings_notice_backup_created(value),
        "settings_notice_backup_restore_failed" => {
            strings.set_settings_notice_backup_restore_failed(value)
        }
        "settings_notice_backup_restore_applied" => {
            strings.set_settings_notice_backup_restore_applied(value)
        }
        "settings_notice_backup_list_failed" => {
            strings.set_settings_notice_backup_list_failed(value)
        }
        "settings_notice_reset_default_applied" => {
            strings.set_settings_notice_reset_default_applied(value)
        }
        "settings_notice_clear_all_applied" => strings.set_settings_notice_clear_all_applied(value),
        "settings_notice_data_dir_opened" => strings.set_settings_notice_data_dir_opened(value),
        "settings_notice_export_written" => strings.set_settings_notice_export_written(value),
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
    // M05.5 context menu strings.
    strings.set_context_menu_open(
        filego::presentation::i18n::Msg::ContextMenuOpen
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_copy_path(
        filego::presentation::i18n::Msg::ContextMenuCopyPath
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_copy_name(
        filego::presentation::i18n::Msg::ContextMenuCopyName
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_edit(
        filego::presentation::i18n::Msg::ContextMenuEdit
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_pin(
        filego::presentation::i18n::Msg::ContextMenuPin
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_unpin(
        filego::presentation::i18n::Msg::ContextMenuUnpin
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_disable(
        filego::presentation::i18n::Msg::ContextMenuDisable
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_enable(
        filego::presentation::i18n::Msg::ContextMenuEnable
            .tr(locale)
            .into(),
    );
    strings.set_context_menu_remove_record(
        filego::presentation::i18n::Msg::ContextMenuRemove
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

    // ---- M05: shared repository (opened early so M06 can read the persisted
    // settings for the hotkey + startup behavior) --------------------------
    let repo = open_repository();

    // ---- M04.2/M04.3 native platform --------------------------------
    // M06: the hotkey is the PERSISTED one (from the shared repository), not a
    // hardcoded default. A failed worker still degrades to `disabled`.
    let persisted_hotkey = repo
        .borrow()
        .document()
        .and_then(|document| document.data.settings.hotkey);
    let native =
        filego::platform::windows::NativePlatform::start(persisted_hotkey).unwrap_or_else(|_| {
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

    // M06: startup locale follows the persisted language preference; the theme
    // and font scale are applied from the repository's settings.
    let startup_locale = locale_for(
        repo.borrow()
            .document()
            .map(|document| document.data.settings.language_preference)
            .unwrap_or_default(),
    );
    apply_localization_and_theme(&app, startup_locale);
    let startup_window_visible = {
        let document = repo.borrow().document().cloned();
        document
            .map(|document| {
                let settings = document.data.settings;
                let resolved = filego::presentation::theme::ResolvedTheme::resolve(
                    settings.theme,
                    filego::presentation::theme::ResolvedColorScheme::Light,
                );
                let theme_global = app.global::<UiTheme>();
                theme_global.set_font_scale(f32::from(settings.font_scale_percent) / 100.0);
                ui_set_theme(&app, resolved);
                set_monitor_strategy(settings.monitor_strategy);
                set_search_window_width(settings.search_window_width);
                !settings.silent_start || settings.show_main_window_at_startup
            })
            .unwrap_or(false)
    };

    // M06: show the main window at startup when the persisted settings ask for
    // it (default is silent tray-only).
    if startup_window_visible {
        let controller = Rc::clone(&controller);
        let port = Rc::clone(&port);
        let _ = controller
            .borrow_mut()
            .handle(LifecycleCommand::Show, &mut *port.borrow_mut());
    }

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
    let (context_tx, context_rx) = std::sync::mpsc::channel();
    let settings_window = SettingsWindow::new()?;
    apply_settings_localization(
        &settings_window,
        filego::presentation::i18n::Locale::default(),
    );
    let store = filego::presentation::manager::SharedStore::new(Rc::clone(&repo));
    // `settings_adapter` is defined AFTER `main` (below) because the settings
    // controller needs a clone of the main controller to refresh the search
    // window when search settings change; `main` only needs `context_tx`.
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
    // M06: the search window runs on the FULL persisted settings (search
    // toggles, empty-query strategy, widths, theme, ...) so settings changes
    // take effect immediately when the adapter rebuilds the ViewModel.
    let resolved = resolved_from_repository(&repo);
    let settings = repo
        .borrow()
        .document()
        .map(|document| document.data.settings.clone())
        .unwrap_or_default();
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
    // M05.5: context-menu action for a requested row. The adapter selects the
    // row and dispatches the corresponding RowAction (the ViewModel maps it to
    // an ExternalEffect; edit/pin/enable/remove route to the settings window).
    let main_ui = Rc::clone(&main);
    app.on_command_context_action(move |index, action| {
        let mut main = main_ui.borrow_mut();
        main.handle(ViewCommand::SelectIndex(index as usize));
        let row_action = match action {
            0 => RowAction::Open,
            1 => RowAction::CopyPath,
            2 => RowAction::CopyName,
            3 => RowAction::Edit,
            4 => RowAction::TogglePin,
            5 => RowAction::ToggleEnable,
            _ => RowAction::RemoveFromList,
        };
        main.handle(ViewCommand::RowAction(row_action));
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

    // ---- M05/M06: real settings adapter (needs `main` for search refresh) ----
    // The settings controller shares the same repository; the adapter also
    // holds the native platform (hotkey + tray) and the data directory.
    let settings_adapter = Rc::new(RefCell::new(SettingsWindowController::new(
        settings_window.as_weak(),
        store,
        context_rx,
        Rc::clone(&native),
        data_dir(),
        Rc::clone(&main),
    )));
    // Push the initial settings view + apply persisted startup appearance.
    {
        let mut adapter = settings_adapter.borrow_mut();
        // Launch-at-login OS state (read from HKCU; failure = None → the toggle
        // is driven by the persisted flag with an honest "unread" fallback).
        adapter
            .settings
            .set_launch_at_login_os(filego::platform::windows::tray_open::launch_at_login().ok());
        // Hotkey runtime from the native machine (Active/Paused/Disabled).
        adapter.sync_hotkey_from_native();
        // Persisted theme + locale + font-scale applied on open.
        adapter.refresh_settings_appearance();
    }
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
            // M05/M06: tray "设置" opens the settings window (M06 pages included).
            // M06 review H1: the tray Settings menu is now enabled and this is
            // its only reachable path. Show + bring-to-front so the window is
            // not just mapped but foreground-focused. The SettingsWindow has no
            // focus-loss hide wiring (hide-on-focus-loss only applies to the
            // main search window), so it stays open while the user navigates.
            open_settings_window(&settings);
        });
    }

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
            // M06: pages 1/2/3 stay on the M05 management controller; pages
            // 0/4/5/6/7/8 route to the M06 settings controller.
            match page {
                1 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Folders,
                )),
                2 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Categories,
                )),
                3 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Tags,
                )),
                other => {
                    settings
                        .borrow_mut()
                        .handle_settings(SCommand::ShowPage(other as u8));
                }
            }
            // One source of truth for the current page.
            if let Some(window) = settings.borrow().window.upgrade() {
                window.set_page(page);
            }
            let repo = settings.borrow().manager_repo();
            main.borrow_mut().refresh_from_repository(&repo);
        });
    }
    // M06 nav buttons call `s-show-page`; route them identically.
    {
        let settings = Rc::clone(&settings_adapter);
        let main = Rc::clone(&main);
        settings_window.on_s_show_page(move |page| {
            match page {
                1 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Folders,
                )),
                2 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Categories,
                )),
                3 => settings.borrow_mut().handle(MCommand::ShowPage(
                    filego::presentation::manager::Page::Tags,
                )),
                other => {
                    settings
                        .borrow_mut()
                        .handle_settings(SCommand::ShowPage(other as u8));
                }
            }
            if let Some(window) = settings.borrow().window.upgrade() {
                window.set_page(page);
            }
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
    // M05 review C1 (Critical): folder row actions carry the ROW INDEX (never
    // a truncating `u128 as i32` id); the index resolves to the real 128-bit
    // FolderId held in the current view rows. An out-of-range index no-ops
    // instead of acting on a wrong record — deterministic, no silent
    // wrong-record.
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_edit(move |index| {
            let folder_id = {
                let s = settings.borrow();
                filego::presentation::manager::folder_id_at(&s.manager.view().rows, index as usize)
            };
            if let Some(folder_id) = folder_id {
                settings
                    .borrow_mut()
                    .handle(MCommand::EditFolder(folder_id));
                if let Some(window) = settings.borrow().window.upgrade() {
                    let _ = window.show();
                }
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_remove(move |index| {
            let folder_id = {
                let s = settings.borrow();
                filego::presentation::manager::folder_id_at(&s.manager.view().rows, index as usize)
            };
            if let Some(folder_id) = folder_id {
                settings
                    .borrow_mut()
                    .handle(MCommand::StartRemove(folder_id));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_enabled(move |index| {
            let folder_id = {
                let s = settings.borrow();
                filego::presentation::manager::folder_id_at(&s.manager.view().rows, index as usize)
            };
            if let Some(folder_id) = folder_id {
                settings
                    .borrow_mut()
                    .handle(MCommand::ToggleEnable(folder_id));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_pin(move |index| {
            let folder_id = {
                let s = settings.borrow();
                filego::presentation::manager::folder_id_at(&s.manager.view().rows, index as usize)
            };
            if let Some(folder_id) = folder_id {
                settings.borrow_mut().handle(MCommand::TogglePin(folder_id));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_check(move |index| {
            let folder_id = {
                let s = settings.borrow();
                filego::presentation::manager::folder_id_at(&s.manager.view().rows, index as usize)
            };
            if let Some(folder_id) = folder_id {
                settings.borrow_mut().handle(MCommand::CheckPath(folder_id));
            }
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
    // M05 review M2: category / enabled-status filters and the sort selector
    // map their combo indices to the existing pure `FolderFilter`/`FolderSort`
    // logic (index 0 = all / by-name; status 1 = enabled only, 2 = disabled
    // only; category index 0 = all, 1 = 未分类, 2+k = categories[k]).
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_filter_category(move |index| {
            let category = match index {
                i if i <= 0 => None,
                1 => Some(filego::domain::ids::CategoryId::from_uuid(uuid::Uuid::nil())),
                _ => {
                    let s = settings.borrow();
                    s.manager
                        .view()
                        .categories
                        .get((index - 2) as usize)
                        .map(|c| c.id)
                }
            };
            settings
                .borrow_mut()
                .handle(MCommand::SetFilterCategory(category));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_filter_enabled(move |index| {
            let enabled = match index {
                1 => Some(true),
                2 => Some(false),
                _ => None,
            };
            settings
                .borrow_mut()
                .handle(MCommand::SetFilterEnabled(enabled));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_sort(move |index| {
            let sort = match index {
                1 => filego::presentation::management::FolderSort::RecentlyUsed,
                2 => filego::presentation::management::FolderSort::AddedTime,
                _ => filego::presentation::management::FolderSort::Name,
            };
            settings.borrow_mut().handle(MCommand::SetSort(sort));
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
    // M05 review H3: category/tag deletion requires a two-step confirmation.
    // The UI arms a per-row confirm state; only the explicit confirm callback
    // performs the delete (whose semantics stay right: category delete → 未分类,
    // tag delete → refs cleared, and real directories are never touched).
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_confirm_delete_category(move |index| {
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
    // The two cancel handlers are intentional no-ops: the Slint UI clears its
    // own per-row confirm arming locally; Rust has nothing else to do (the
    // closure captures nothing, so no `settings` clone).
    {
        settings_window.on_command_cancel_delete_category(move || {});
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_confirm_delete_tag(move |index| {
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
        settings_window.on_command_cancel_delete_tag(move || {});
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
    // M05 review H2: add/edit dialog fields bound to the real draft values.
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_draft_note(move |note| {
            settings
                .borrow_mut()
                .handle(MCommand::EditNote(note.to_string()));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_draft_weight(move |weight| {
            settings
                .borrow_mut()
                .handle(MCommand::SetDraftWeight(weight as i16));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_cycle_draft_color(move || {
            settings.borrow_mut().handle(MCommand::CycleDraftColor);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_draft_category(move |index| {
            let category = {
                let s = settings.borrow();
                if index <= 0 {
                    None
                } else {
                    s.manager
                        .view()
                        .categories
                        .get((index - 1) as usize)
                        .map(|c| c.id)
                }
            };
            settings
                .borrow_mut()
                .handle(MCommand::SetDraftCategory(category));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_toggle_draft_tag(move |index, on| {
            let tag_id = {
                let s = settings.borrow();
                s.manager.view().tags.get(index as usize).map(|t| t.id)
            };
            if let Some(tag_id) = tag_id {
                settings
                    .borrow_mut()
                    .handle(MCommand::SetDraftTag(tag_id, on));
            }
        });
    }
    // M05 review H2: duplicate-policy resolution (0 = cancel, 1 = edit
    // existing, 2 = save as different name) is delegated to the pure logic.
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_resolve_duplicate(move |policy| {
            let policy = match policy {
                1 => filego::presentation::management::DuplicatePolicy::EditExisting,
                2 => filego::presentation::management::DuplicatePolicy::SaveAsDifferentName,
                _ => filego::presentation::management::DuplicatePolicy::Cancel,
            };
            settings
                .borrow_mut()
                .handle(MCommand::ResolveDuplicate(policy));
        });
    }
    // M05 review M1: persist the one-level-import setting via the controller.
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_command_set_one_level_import(move |enabled| {
            settings
                .borrow_mut()
                .handle(MCommand::SetOneLevelImport(enabled));
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

    // ---- M06: settings-page callbacks --------------------------------------
    // Each Slint control forwards an SCommand; the adapter persists through the
    // settings controller and performs the platform side effect.
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_close(move || {
            if let Some(window) = settings.borrow().window.upgrade() {
                let _ = window.hide();
            }
        });
    }
    // 常规
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_launch_at_login(move |on| {
            // HKCU first; only a successful registry write persists + confirms
            // (no fake success). On failure the toggle snaps back to the old.
            let written = filego::platform::windows::tray_open::set_launch_at_login(on).is_ok();
            let mut adapter = settings.borrow_mut();
            if written {
                adapter.settings.handle(SCommand::SetLaunchAtLogin(on));
                adapter.settings.set_launch_at_login_os(Some(on));
                // Keep the tray menu glyph in sync (M06.3 status↔menu synced).
                if let Some(tray) = tray.as_weak().upgrade() {
                    tray.set_launch_at_login_glyph(if on { "✓ " } else { "" }.into());
                }
            } else {
                adapter.settings.set_notice(SNotice::StartupWriteFailed);
            }
            adapter.sync_settings_ui();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_silent_start(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSilentStart(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_show_main(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetShowMainWindowAtStartup(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hide_after_open(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetHideAfterOpen(on));
            let repo = settings.borrow().manager_repo();
            settings
                .borrow()
                .main
                .borrow_mut()
                .refresh_from_repository(&repo);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_clear_after_open(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetClearAfterOpen(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hide_on_focus_loss(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetHideOnFocusLoss(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_monitor_strategy(move |index| {
            let strategy = if index == 1 {
                filego::domain::settings::MonitorStrategy::ActiveWindow
            } else {
                filego::domain::settings::MonitorStrategy::Mouse
            };
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetMonitorStrategy(strategy));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_language(move |index| {
            let language = match index {
                1 => filego::domain::settings::LanguagePreference::ZhCN,
                2 => filego::domain::settings::LanguagePreference::EnUS,
                _ => filego::domain::settings::LanguagePreference::System,
            };
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetLanguage(language));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_restore_defaults(move || {
            settings.borrow_mut().reset_settings();
        });
    }
    // 搜索
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_paths(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchPaths(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_categories(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchCategories(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_tags(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchTags(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_notes(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchNotes(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_aliases(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchAliases(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_fuzzy(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetFuzzyMatching(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_pinyin(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchPinyin(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_english_initials(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchEnglishInitials(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_highlight(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetHighlightResults(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_recent_sort(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetRecentSort(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_edit_distance(move |distance| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetMaxEditDistance(distance as u8));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_max_results(move |count| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetMaxResults(count as u16));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_empty_query(move |index| {
            let strategy = match index {
                1 => filego::domain::settings::EmptyQueryStrategy::All,
                2 => filego::domain::settings::EmptyQueryStrategy::PinnedOnly,
                3 => filego::domain::settings::EmptyQueryStrategy::Blank,
                _ => filego::domain::settings::EmptyQueryStrategy::FavoritesFirst,
            };
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetEmptyQueryStrategy(strategy));
        });
    }
    // 外观
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_theme(move |index| {
            let theme = match index {
                1 => filego::domain::settings::ThemePreference::Light,
                2 => filego::domain::settings::ThemePreference::Dark,
                _ => filego::domain::settings::ThemePreference::System,
            };
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetTheme(theme));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_row_height(move |index| {
            let preference = if index == 1 {
                filego::domain::settings::RowHeightPreference::Standard
            } else {
                filego::domain::settings::RowHeightPreference::Compact
            };
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetRowHeight(preference));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_search_width(move |width| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSearchWindowWidth(width as u16));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_settings_width(move |width| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetSettingsWindowWidth(width as u16));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_font_scale(move |percent| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetFontScalePercent(percent as u16));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_show_path(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetShowPathInResults(on));
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_show_cat_tag(move |on| {
            settings
                .borrow_mut()
                .handle_settings(SCommand::SetShowCategoryTagInResults(on));
        });
    }
    // 快捷键
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_record(move || {
            settings
                .borrow_mut()
                .handle_settings(SCommand::StartRecording);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_cancel(move || {
            settings
                .borrow_mut()
                .handle_settings(SCommand::CancelRecording);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_pause(move || {
            settings.borrow().native.borrow().toggle_pause();
            settings.borrow_mut().sync_hotkey_from_native();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_resume(move || {
            settings.borrow().native.borrow().toggle_pause();
            settings.borrow_mut().sync_hotkey_from_native();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_clear(move || {
            let mut adapter = settings.borrow_mut();
            let _ = adapter.native.borrow().clear_hotkey();
            adapter.settings.handle(SCommand::PersistHotkey(None));
            adapter.sync_hotkey_from_native();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_restore(move || {
            let default_combo = filego::domain::settings::HotkeySetting {
                modifiers: filego::domain::settings::DEFAULT_HOTKEY_MODIFIERS,
                key: filego::domain::settings::DEFAULT_HOTKEY_KEY,
            };
            let mut adapter = settings.borrow_mut();
            let registered = adapter.native.borrow().set_hotkey(default_combo);
            if registered.is_ok() {
                adapter.settings.handle(SCommand::RestoreDefaultHotkey);
            } else {
                adapter.settings.set_notice(SNotice::HotkeyConflict);
                adapter.settings.handle(SCommand::SyncHotkeyRuntime(
                    filego::presentation::settings_controller::HotkeyRuntime::Disabled,
                    Some(filego::platform::hotkey::HotkeyErrorKind::Conflict),
                ));
            }
            adapter.sync_hotkey_from_native();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_hotkey_key(move |control, alt, shift, win, text| {
            settings
                .borrow_mut()
                .record_hotkey_key(control, alt, shift, win, text.as_str());
        });
    }
    // 数据
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_open_data_dir(move || {
            settings.borrow_mut().open_data_dir();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_export(move || {
            settings.borrow_mut().export_data();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_import(move || {
            settings.borrow_mut().import_data();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_set_import_mode(move |index| {
            let mode = import_mode_from_index(index);
            // Re-plan the preview with the new mode.
            let adapter = settings.borrow_mut();
            let plan = {
                let incoming = adapter.pending_import.clone();
                let current = adapter.settings.document();
                match (incoming, current) {
                    (Some(incoming), Some(current)) => Some(
                        filego::storage::import_export::plan_import(&current.data, &incoming, mode),
                    ),
                    _ => None,
                }
            };
            drop(adapter);
            let mut adapter = settings.borrow_mut();
            if let Some(plan) = plan {
                adapter.settings.push_import_preview(plan, mode);
            }
            adapter.sync_settings_ui();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_apply_import(move || {
            settings.borrow_mut().apply_import();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_cancel_import(move || {
            let mut adapter = settings.borrow_mut();
            adapter.pending_import = None;
            adapter.settings.handle(SCommand::DismissDataFlow);
            adapter.sync_settings_ui();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_create_backup(move || {
            settings.borrow_mut().create_backup();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_list_backups(move || {
            settings.borrow_mut().list_backups();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_restore_backup(move |index| {
            settings.borrow_mut().restore_backup(index);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_clear_recent(move || {
            settings.borrow_mut().clear_recent();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_confirm_reset(move || {
            settings.borrow_mut().reset_settings();
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_confirm_clear_all(move || {
            settings.borrow_mut().clear_all_records();
        });
    }
    // M05-deferral 重命名 / 合并（row 索引 → 控制器命令）。
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_rename_category(move |index, name| {
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
                    .handle_settings(SCommand::RenameCategory(CategoryNameCommand {
                        id: category_id,
                        name: name.to_string(),
                    }));
            }
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_rename_tag(move |index, name| {
            let tag_id = {
                let s = settings.borrow();
                s.manager.view().tags.get(index as usize).map(|t| t.id)
            };
            if let Some(tag_id) = tag_id {
                settings
                    .borrow_mut()
                    .handle_settings(SCommand::RenameTag(TagNameCommand {
                        id: tag_id,
                        name: name.to_string(),
                    }));
            }
            let repo = settings.borrow().manager_repo();
            settings
                .borrow()
                .main
                .borrow_mut()
                .refresh_from_repository(&repo);
        });
    }
    {
        let settings = Rc::clone(&settings_adapter);
        settings_window.on_s_command_merge_tag(move |index, target_name| {
            let source_id = {
                let s = settings.borrow();
                s.manager.view().tags.get(index as usize).map(|t| t.id)
            };
            if let Some(source_id) = source_id {
                settings.borrow_mut().handle_settings(SCommand::MergeTag {
                    source: source_id,
                    target_name: target_name.to_string(),
                });
            }
            let repo = settings.borrow().manager_repo();
            settings
                .borrow()
                .main
                .borrow_mut()
                .refresh_from_repository(&repo);
        });
    }

    // Push the initial settings view once. Initial page = 常规 (the settings
    // controller's default; the nav handlers drive it from then on).
    settings_window.set_page(0);
    settings_adapter.borrow_mut().sync_ui();
    settings_adapter.borrow_mut().sync_settings_ui();

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
    fn tray_settings_menu_item_is_enabled() {
        // M06 review H1: the tray "Settings" MenuItem must NOT be gated with
        // `enabled: false` — it is the only direct entry to the settings
        // window. Source guard against re-introducing the dead gate.
        let slint_source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/app-window.slint"),
        )
        .expect("app-window.slint is checked in at the repo root");
        let tray_section = slint_source
            .split("component AppTray inherits SystemTrayIcon")
            .nth(1)
            .expect("AppTray component present");
        let title_at = tray_section
            .find("title: @tr(\"Settings\")")
            .expect("Settings menu item present");
        // Scope to the entire Settings `MenuItem { ... }` block: the nearest
        // `{` after the opening `MenuItem` up to the closing `}` of its last
        // `activated` handler (the block's top brace becomes `open_at`).
        let open_at = tray_section[..title_at]
            .rfind("MenuItem {")
            .map(|i| i + "MenuItem {".len())
            .expect("Settings item opened with MenuItem {");
        let rest = &tray_section[open_at..];
        // The first `}` after the title closes `{ root.open-settings(); }`; the
        // block is balanced, so the SECOND `}` closes the MenuItem element.
        let first_close = rest.find('}').expect("handler close");
        let second_close = rest[first_close + 1..].find('}').expect("element close");
        let item_body = &rest[..first_close + 1 + second_close + 1];
        // Match the Slint gate exactly (`enabled: false`), so a comment
        // merely mentioning the form can never false-positive.
        assert!(
            !item_body.contains("enabled: false"),
            "the tray Settings MenuItem must not carry `enabled: false` (H1)"
        );
        assert!(
            item_body.contains("activated => { root.open-settings(); }"),
            "the tray Settings item must still call open-settings()"
        );
    }

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
