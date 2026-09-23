//! Presentation boundary between application state and Slint callbacks (M03).
//!
//! M03 introduces a pure-Rust, Slint-independent ViewModel that is the single
//! source of truth for the main search window. The Slint UI (see
//! `ui/app-window.slint`) is a thin adapter on top: it emits [`crate::presentation::commands::ViewCommand`]s
//! through callbacks and renders the [`crate::presentation::view_model::SearchViewModel`]'s
//! observable state.
//!
//! Module layout:
//! - `commands` — every user gesture the UI can emit, as a small typed enum.
//! - `view_model` — the state machine + command application + search runner
//!   seam; fully unit-testable without Slint.
//! - `state` — the observable state pieces (query, results, selection, filters,
//!   error, busy) exchanged between ViewModel and adapters.
//! - `i18n` — locale model + the zh-CN / en-US key/value catalogs.
//! - `theme` — resolved theme tokens derived from settings/system.
//!
//! The real Windows tray / hotkey / single-instance integration stays M04; the
//! existing `AppTray` shell continues to drive show/hide through
//! [`crate::app::LifecycleController`].

pub mod commands;
pub mod i18n;
pub mod management;
pub mod manager;
pub mod settings_controller;
pub mod state;
pub mod theme;
pub mod view_model;
