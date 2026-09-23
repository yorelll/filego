#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{cell::RefCell, rc::Rc};

use filego::{
    AppTray, AppWindow, UiStrings, UiTheme,
    app::{LifecycleCommand, LifecycleController, WindowPort},
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
        self.app()?.show()
    }

    fn hide(&mut self) -> Result<(), Self::Error> {
        self.app()?.hide()
    }

    fn quit(&mut self) -> Result<(), Self::Error> {
        slint::quit_event_loop().map_err(event_loop_error_as_platform_error)
    }
}

fn event_loop_error_as_platform_error(error: slint::EventLoopError) -> slint::PlatformError {
    slint::PlatformError::from(error.to_string())
}

/// The 0.0.1 ViewModel-driven adapter for the M03 search window.
///
/// M03 intentionally has no real repository wiring yet (M05/M06). This adapter
/// hands the ViewModel a small set of resolved entries so the window can
/// display and search with fake data, as the M03.5 smoke requirement asks; the
/// storage-backed construction is M06. "Open selected" is a stub effect:
/// M04.5 wires the real Explorer shell open.
struct MainWindowController {
    view_model: SearchViewModel,
    window: slint::Weak<AppWindow>,
}

impl MainWindowController {
    fn new(window: slint::Weak<AppWindow>) -> Self {
        let settings = filego::domain::settings::AppSettings::default();
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
            settings,
            resolved,
            Box::new(default_runner),
            Box::new(NoopEffects),
        );
        Self {
            view_model,
            window: window.clone(),
        }
    }

    fn handle(&mut self, command: ViewCommand) {
        let effects = self.view_model.handle(command);
        self.sync_ui();
        for effect in effects {
            self.apply_effect(effect);
        }
    }

    // IME gate hook point (documented; final native wiring is M03.3
    // manual-acceptance). M04's native preedit adapter will call
    // `view_model.set_ime_composition(active)` at composition start/end (the
    // ViewModel blocks Enter/Up/Down result actions while composing) and then
    // `sync_ui()` mirrors the flag into the UI's `ime-composing` property,
    // which makes the input key handler reject Enter/arrows to the IME. Slint
    // 1.18 exposes no preedit event on the public `Window` API, so this is
    // wired by the future adapter.

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
            // M03 stub: "open selected" only signals intent. M04.5 wires the
            // shell verb to open the path. No shell command is constructed.
            ExternalEffect::OpenEntry => {}
            // M03 stub: the path-copy effect exists in the ViewModel; M04/M05
            // wires it to the system clipboard through the platform adapter
            // (Slint 1.18 does not expose clipboard on `Window`).
            ExternalEffect::CopyPath => {}
            ExternalEffect::ClearInput => {
                // The LineEdit already clears via state-query; keep focus on it.
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
        window.set_rows_count(rows.len() as i32);
        window.set_rows_name(string_model(
            rows.iter().map(|row| row.display_name.clone()),
        ));
        window.set_rows_path(string_model(rows.iter().map(|row| row.path_label.clone())));
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
    strings.set_count(
        filego::presentation::i18n::Msg::ResultsCount(0)
            .tr(locale)
            .into(),
    );
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
    let app = AppWindow::new()?;
    let tray = AppTray::new()?;
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
            report_platform_error(
                controller
                    .borrow_mut()
                    .handle(LifecycleCommand::ExitFromTray, &mut *port.borrow_mut()),
            );
        });
    }

    // ---- M03: ViewModel adapter wiring --------------------------------
    let main = Rc::new(RefCell::new(MainWindowController::new(app.as_weak())));
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
    // through `MainWindowController::set_ime_composition`, which the native
    // preedit adapter (M04+) will call. sync_ui mirrors it into
    // `set_ime_composing`.

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
}
