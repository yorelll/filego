#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{cell::RefCell, rc::Rc};

use filego::{
    AppTray, AppWindow, UiStrings, UiTheme,
    app::{LifecycleCommand, LifecycleController, WindowPort},
    domain::settings::AppSettings,
    presentation::{
        commands::{RowAction, SearchKey, ViewCommand},
        state::SelectionMove,
        view_model::{ExternalEffect, NoopEffects, SearchViewModel, default_runner},
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
/// Re-computed on every show so monitor count/DPI changes are picked up
/// deterministically.
fn place_window(app: &AppWindow) {
    let window = app.window();
    let scale = window.scale_factor();
    // Slint reports the logical window size; scale to physical, then place.
    let logical = window.size().to_logical(scale);
    let (physical_w, physical_h) = if logical.width > 0.0 && logical.height > 0.0 {
        (logical.width * scale, logical.height * scale)
    } else {
        // Default first-show size (600x140 logical).
        (600.0 * scale, 140.0 * scale)
    };
    let rect = filego::platform::windows::window_placement::placement_rect(
        physical_w.round().max(1.0) as i32,
        physical_h.round().max(1.0) as i32,
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
/// M03 intentionally has no real repository wiring yet (M05/M06). This adapter
/// hands the ViewModel a small set of resolved entries so the window can
/// display and search with fake data, as the M03.5 smoke requirement asks; the
/// storage-backed construction is M06.
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
}

impl MainWindowController {
    fn new(
        window: slint::Weak<AppWindow>,
        open_results: std::sync::mpsc::Receiver<OpenResult>,
        open_sender: std::sync::mpsc::Sender<OpenResult>,
        shell: ShellHandle,
    ) -> Self {
        let settings = AppSettings::default();
        // M03 demo data: a couple of local shortcut records (paths are never
        // probed by the search core; accessibility stays Unknown).
        let searchable = vec![
            demo_entry(1, "Documents", r"C:\Users\demo\Documents", true),
            demo_entry(2, "Photos", r"C:\Users\demo\Pictures\Photos", false),
            demo_entry(3, "设计资料", r"D:\work\design-assets", false),
            demo_entry(4, "Code Repos", r"C:\dev\repos", true),
        ];
        let resolved =
            filego::presentation::view_model::resolved_from_search(&searchable, |entry| {
                entry
                    .path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&entry.path)
                    .to_owned()
            });
        let view_model = SearchViewModel::new(
            settings.clone(),
            resolved,
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
        }
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
            // is async off the UI thread; the result is delivered back through
            // `slint::invoke_from_event_loop` into `apply_open_result`.
            ExternalEffect::OpenEntry => self.open_selected(),
            // Copy the full selection path to the system clipboard. M04 keeps
            // the ViewModel's effect; the wiring is clipboard via Slint's
            // `Slint.Clipboard` text copying through platform. Slint 1.18 does
            // not expose clipboard on `Window`, so we surface the selected
            // path into the OS clipboard with a UTF-16 write in the clipboard
            // adapter (part of M04 native wiring, but only best-effort).
            ExternalEffect::CopyPath => self.copy_selected_path(),
            ExternalEffect::ClearInput => {
                // The LineEdit already clears via state-query; keep focus on it.
            }
        }
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
            OpenResult::Failed(_kind) => {
                // Keep the window; surface an anonymous failure; the selection
                // stays so Retry (Enter) / Copy (Ctrl+C) keep working.
                self.view_model.set_open_failure();
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
            Some(filego::presentation::state::SearchFailure::Open) => (
                filego::presentation::i18n::Msg::ErrorOpenTitle.tr(state.locale),
                filego::presentation::i18n::Msg::ErrorOpenBody.tr(state.locale),
            ),
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

fn demo_entry(id: u128, name: &str, path: &str, pinned: bool) -> filego::search::SearchEntry {
    filego::search::SearchEntry {
        id: filego::domain::ids::FolderId::from_uuid(uuid::Uuid::from_u128(id)),
        display_name: name.to_owned(),
        aliases: Vec::new(),
        path: path.to_owned(),
        category_name: None,
        tag_names: Vec::new(),
        note: String::new(),
        pinned,
        favorite: false,
        manual_weight: 0,
        open_count: 0,
        last_opened_at: Some("2026-09-21T00:00:00Z".parse().expect("fixed date")),
        accessibility: filego::search::Accessibility::Unknown,
        origin: filego::search::Origin::Unknown,
    }
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

    {
        let app_weak = app.as_weak();
        tray.on_add_folder(move || {
            // M05 wires the folder picker; M04 keeps the menu item present but
            // does nothing (no pseudo-action).
            let _ = app_weak;
        });
    }
    {
        tray.on_open_settings(move || {
            // M06 wires the settings window.
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

    // ---- M03: ViewModel adapter wiring --------------------------------
    let (open_tx, open_rx) = std::sync::mpsc::channel();
    let shell = ShellHandle::real();
    let main = Rc::new(RefCell::new(MainWindowController::new(
        app.as_weak(),
        open_rx,
        open_tx,
        shell,
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
