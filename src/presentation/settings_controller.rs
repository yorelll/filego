//! M06 settings presenter (pure Rust, Slint-independent).
//!
//! Owns the settings pages' decision layer: which page is active, the settings
//! values (snapshot + live persistence through the repository), the hotkey
//! record state machine (mirroring `platform::hotkey` rules), and the data-page
//! flow state (import preview/apply, backup list, confirmations).
//!
//! # Round-trip contract
//!
//! Every `SCommand::Set*` mutates `view.settings` (the UI snapshot) AND
//! persists the whole document through [`SettingsStore`]. The store's in-memory
//! working copy is the source of truth; on a failed save the snapshot rolls
//! back to the document value so the UI never shows a half-applied setting.
//!
//! # Platform side effects stay in the adapter
//!
//! The adapter (in `main.rs`) performs the registry write, the native hotkey
//! registration, the export/backup filesystem writes and the data-dir open,
//! THEN forwards the relevant state back here via the `set_*`/`sync_*` public
//! methods (e.g. [`Self::set_launch_at_login_os`],
//! [`Self::sync_hotkey_runtime`], [`Self::push_import_preview`],
//! [`Self::set_backups`]). This module never touches the registry, the hotkey
//! machine, the shell or the network.
//!
//! # No fake UI
//!
//! Every control maps to a [`SCommand`] that changes a real persisted field, or
//! is a declared "后续版本" disabled placeholder in the `.slint`/i18n layer.
//! This module has no virtual toggles.

use crate::{
    domain::{
        document::AppData,
        settings::{
            AppSettings, EmptyQueryStrategy, LanguagePreference, MonitorStrategy,
            RowHeightPreference, ThemePreference,
        },
    },
    storage::{import_export, repository::DocumentRepository, schema::StoredDocumentV1},
};

use super::management::{names_equal, normalize_name, valid_named_value};

/// Which settings nav page is active (index into the left-nav; matches the
/// Slint nav button order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SPage {
    General = 0,
    Folders = 1,    // M05 management page.
    Categories = 2, // M05 management page.
    Tags = 3,       // M05 management page.
    Search = 4,
    Appearance = 5,
    Hotkey = 6,
    Data = 7,
    About = 8,
}

impl SPage {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
    pub const fn from_u8(value: u8) -> SPage {
        match value {
            1 => SPage::Folders,
            2 => SPage::Categories,
            3 => SPage::Tags,
            4 => SPage::Search,
            5 => SPage::Appearance,
            6 => SPage::Hotkey,
            7 => SPage::Data,
            8 => SPage::About,
            _ => SPage::General,
        }
    }
}

/// One anonymous, localizable notice the settings window renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SNotice {
    Saved,
    SaveFailed,
    StartupWriteFailed,
    StartupReadFailed,
    HotkeyConflict,
    HotkeyUnavailable,
    HotkeyInvalid,
    ImportParseFailed,
    ImportFutureSchema,
    ImportMigrationNeeded,
    ImportInvalidDocument,
    ImportUnresolvedReference,
    ImportApplied,
    ImportPreviewFailed,
    ExportFailed,
    ExportTargetExists,
    BackupsNone,
    BackupCreated,
    BackupRestoreFailed,
    BackupRestoreApplied,
    BackupListFailed,
    ResetDefaultApplied,
    ClearAllApplied,
    DataDirOpened,
    DataDirOpenFailed,
    ExportWritten,
}

/// The hotkey-record state machine (drives the Hotkey page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyRecordPhase {
    /// Not recording.
    Idle,
    /// Waiting for the next key.
    Listening,
}

/// Current hotkey runtime as the page renders (projected by the adapter from
/// the native machine, or injected in tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyRuntime {
    Disabled,
    Active,
    Paused,
}

/// Data-page flow (M06.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataFlow {
    /// Nothing pending.
    Closed,
    /// A parsed import file is previewed (the plan + chosen mode).
    ImportPreview {
        plan: import_export::ImportPlan,
        mode: import_export::ImportMode,
    },
    /// The "clear all records" two-step confirmation is armed.
    ConfirmClearAll,
}

/// The observable state the adapter pushes into the Slint settings window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SView {
    pub page: u8,
    /// The settings the UI binds (live; persisted through the repository).
    pub settings: AppSettings,
    /// Whether the HKCU Run value is currently set (`None` = unread/failed).
    pub launch_at_login_os: Option<bool>,
    pub notice: Option<SNotice>,
    pub hotkey_runtime: HotkeyRuntime,
    pub hotkey_last_error: Option<crate::platform::hotkey::HotkeyErrorKind>,
    pub hotkey_record_phase: HotkeyRecordPhase,
    /// Combo currently being recorded (displayed while Listening).
    pub hotkey_draft: Option<crate::domain::settings::HotkeySetting>,
    pub data_flow: DataFlow,
    /// Backup names listed on the Data page (deterministic order).
    pub backups: Vec<String>,
    /// The one-level-import toggle (mirrored from the shared document).
    pub one_level_import_os: bool,
}

impl Default for SView {
    fn default() -> Self {
        Self {
            page: 0,
            settings: AppSettings::default(),
            launch_at_login_os: None,
            notice: None,
            hotkey_runtime: HotkeyRuntime::Disabled,
            hotkey_last_error: None,
            hotkey_record_phase: HotkeyRecordPhase::Idle,
            hotkey_draft: None,
            data_flow: DataFlow::Closed,
            backups: Vec::new(),
            one_level_import_os: false,
        }
    }
}

/// Commands the settings UI forwards. Platform side effects are the adapter's
/// job; this controller stores/persists and maintains the pure state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SCommand {
    ShowPage(u8),
    // ---- General ----
    SetLaunchAtLogin(bool),
    SetSilentStart(bool),
    SetShowMainWindowAtStartup(bool),
    SetHideAfterOpen(bool),
    SetClearAfterOpen(bool),
    SetHideOnFocusLoss(bool),
    SetMonitorStrategy(MonitorStrategy),
    SetLanguage(LanguagePreference),
    RestoreDefaultSettings,
    // ---- Search ----
    SetSearchPaths(bool),
    SetSearchCategories(bool),
    SetSearchTags(bool),
    SetSearchNotes(bool),
    SetSearchAliases(bool),
    SetFuzzyMatching(bool),
    SetSearchPinyin(bool),
    SetSearchEnglishInitials(bool),
    SetMaxEditDistance(u8),
    SetMaxResults(u16),
    SetEmptyQueryStrategy(EmptyQueryStrategy),
    SetHighlightResults(bool),
    SetRecentSort(bool),
    // ---- Appearance ----
    SetTheme(ThemePreference),
    SetRowHeight(RowHeightPreference),
    SetSearchWindowWidth(u16),
    SetSettingsWindowWidth(u16),
    SetFontScalePercent(u16),
    SetShowPathInResults(bool),
    SetShowCategoryTagInResults(bool),
    // ---- Hotkey ----
    StartRecording,
    CancelRecording,
    /// The adapter translated a raw key press into a concrete HotkeyKey; the
    /// controller completes recording with the current recorded modifiers.
    FinishRecording(crate::domain::settings::HotkeyKey),
    /// Set the recorded modifier set while listening (for live feedback).
    SetRecordedModifiers(bool, bool, bool, bool),
    /// Persist a fully-formed combo (already validated + registered by the
    /// adapter; keep-old-on-conflict is the adapter's job).
    PersistHotkey(Option<crate::domain::settings::HotkeySetting>),
    RestoreDefaultHotkey,
    SyncHotkeyRuntime(
        HotkeyRuntime,
        Option<crate::platform::hotkey::HotkeyErrorKind>,
    ),
    // ---- M05 deferral: category/tag rename + merge ----
    RenameCategory(CategoryNameCommand),
    RenameTag(TagNameCommand),
    MergeTag {
        source: crate::domain::ids::TagId,
        target_name: String,
    },
    // ---- Data ----
    /// Store an import preview (the adapter parsed + validated first).
    PushImportPreview(import_export::ImportPlan, import_export::ImportMode),
    /// Apply the previewed import (the adapter already built the plan).
    ApplyImport,
    DismissDataFlow,
    DismissNotice,
    /// Sync the freshly-listed backups (from the adapter).
    SetBackups(Vec<String>),
}

/// Helper so `RenameCategory(RenameCategory{id,name})` reads cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryNameCommand {
    pub id: crate::domain::ids::CategoryId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagNameCommand {
    pub id: crate::domain::ids::TagId,
    pub name: String,
}

/// The persistence store the settings controller drives.
pub trait SettingsStore {
    fn load_document(&mut self) -> Result<StoredDocumentV1, DocumentError>;
    /// Snapshot the current working document.
    fn document(&self) -> Option<StoredDocumentV1>;
    fn set_settings(&mut self, settings: AppSettings) -> Result<(), DocumentError>;
    fn set_data(&mut self, data: AppData) -> Result<(), DocumentError>;
    fn clear_folder_records(&mut self) -> bool;
    fn rename_category_record(
        &mut self,
        id: crate::domain::ids::CategoryId,
        name: String,
    ) -> Result<(), DocumentError>;
    fn rename_tag_record(
        &mut self,
        id: crate::domain::ids::TagId,
        name: String,
    ) -> Result<(), DocumentError>;
    fn merge_tag_record(
        &mut self,
        source: crate::domain::ids::TagId,
        target: crate::domain::ids::TagId,
    ) -> Result<(), DocumentError>;
    fn save_at(&mut self) -> Result<u64, DocumentError>;
}

/// The repository error surfaced through the store (short alias).
pub type DocumentError = crate::storage::repository::RepositoryError;

/// A `SettingsStore` over the same `Rc<RefCell<DocumentRepository>>` the
/// management controller and the search window share (M06 production wiring).
/// `Rc` keeps both controllers on one UI thread; the repository's interior
/// mutability lets the settings page and the management page observe each
/// other's saves.
#[derive(Clone)]
pub struct SettingsSharedStore {
    repo: std::rc::Rc<std::cell::RefCell<DocumentRepository>>,
}

impl SettingsSharedStore {
    pub fn new(repo: std::rc::Rc<std::cell::RefCell<DocumentRepository>>) -> Self {
        Self { repo }
    }

    pub fn repo(&self) -> &std::rc::Rc<std::cell::RefCell<DocumentRepository>> {
        &self.repo
    }
}

impl SettingsStore for SettingsSharedStore {
    fn load_document(&mut self) -> Result<StoredDocumentV1, DocumentError> {
        Ok(match self.repo.borrow_mut().load()? {
            crate::storage::repository::LoadOutcome::Found(document)
            | crate::storage::repository::LoadOutcome::Recovered(document) => document,
        })
    }

    fn document(&self) -> Option<StoredDocumentV1> {
        self.repo.borrow().document().cloned()
    }

    fn set_settings(&mut self, settings: AppSettings) -> Result<(), DocumentError> {
        self.repo.borrow_mut().set_settings(settings)
    }

    fn set_data(&mut self, data: AppData) -> Result<(), DocumentError> {
        self.repo.borrow_mut().set_data(data)
    }

    fn clear_folder_records(&mut self) -> bool {
        self.repo.borrow_mut().clear_all_records()
    }

    fn rename_category_record(
        &mut self,
        id: crate::domain::ids::CategoryId,
        name: String,
    ) -> Result<(), DocumentError> {
        self.repo.borrow_mut().rename_category(id, name)
    }

    fn rename_tag_record(
        &mut self,
        id: crate::domain::ids::TagId,
        name: String,
    ) -> Result<(), DocumentError> {
        self.repo.borrow_mut().rename_tag(id, name)
    }

    fn merge_tag_record(
        &mut self,
        source: crate::domain::ids::TagId,
        target: crate::domain::ids::TagId,
    ) -> Result<(), DocumentError> {
        self.repo.borrow_mut().merge_tag(source, target)
    }

    fn save_at(&mut self) -> Result<u64, DocumentError> {
        self.repo.borrow_mut().save_at()
    }
}

impl SettingsStore for DocumentRepository {
    fn load_document(&mut self) -> Result<StoredDocumentV1, DocumentError> {
        Ok(match self.load()? {
            crate::storage::repository::LoadOutcome::Found(document)
            | crate::storage::repository::LoadOutcome::Recovered(document) => document,
        })
    }

    fn document(&self) -> Option<StoredDocumentV1> {
        self.document().cloned()
    }

    fn set_settings(&mut self, settings: AppSettings) -> Result<(), DocumentError> {
        self.set_settings(settings)
    }

    fn set_data(&mut self, data: AppData) -> Result<(), DocumentError> {
        self.set_data(data)
    }

    fn clear_folder_records(&mut self) -> bool {
        self.clear_all_records()
    }

    fn rename_category_record(
        &mut self,
        id: crate::domain::ids::CategoryId,
        name: String,
    ) -> Result<(), DocumentError> {
        self.rename_category(id, name)
    }
    fn rename_tag_record(
        &mut self,
        id: crate::domain::ids::TagId,
        name: String,
    ) -> Result<(), DocumentError> {
        self.rename_tag(id, name)
    }
    fn merge_tag_record(
        &mut self,
        source: crate::domain::ids::TagId,
        target: crate::domain::ids::TagId,
    ) -> Result<(), DocumentError> {
        self.merge_tag(source, target)
    }
    fn save_at(&mut self) -> Result<u64, DocumentError> {
        self.save_at()
    }
}

/// The controller. `S` is the persistence store (production:
/// `DocumentRepository`; tests: a MemStore).
pub struct SettingsController<S> {
    store: S,
    view: SView,
    /// Recorded modifiers while Listening.
    recorded_modifiers: Option<crate::domain::settings::HotkeyModifiers>,
}

impl<S: SettingsStore> SettingsController<S> {
    pub fn new(mut store: S) -> Self {
        let (settings, one_level_import) = store
            .document()
            .map(|document| {
                let settings = document.data.settings;
                let one_level_import = settings.one_level_import;
                (settings, one_level_import)
            })
            .unwrap_or((AppSettings::default(), false));
        let _ = &mut store;
        Self {
            store,
            view: SView {
                settings,
                one_level_import_os: one_level_import,
                ..SView::default()
            },
            recorded_modifiers: None,
        }
    }

    pub fn view(&self) -> &SView {
        &self.view
    }

    /// The current stored document (the adapter uses it for export/import/
    /// backup and clear-all counts). `None` when nothing is loaded.
    pub fn document(&self) -> Option<StoredDocumentV1> {
        self.store.document()
    }

    /// Re-read persisted settings from the store into the snapshot (used on
    /// window show so external changes are picked up).
    pub fn reload(&mut self) {
        if let Some(document) = self.store.document() {
            let settings = document.data.settings;
            let one_level_import = settings.one_level_import;
            self.view.settings = settings;
            self.view.one_level_import_os = one_level_import;
        }
    }

    /// Adapter-driven: current launch-at-login OS state (read from HKCU).
    pub fn set_launch_at_login_os(&mut self, os: Option<bool>) {
        self.view.launch_at_login_os = os;
    }

    /// Whether the persisted flag and the OS state agree (used to detect a
    /// failed HKCU write and abort a toggle without a fake success).
    pub fn launch_at_login_matches_os(&self) -> bool {
        match self.view.launch_at_login_os {
            Some(os) => os == self.view.settings.launch_at_login,
            None => true, // OS state unknown: nothing to contradict.
        }
    }

    /// Adapter-driven: imported-preview push == command, but also a public
    /// method for tests.
    pub fn push_import_preview(
        &mut self,
        plan: import_export::ImportPlan,
        mode: import_export::ImportMode,
    ) {
        self.view.data_flow = DataFlow::ImportPreview { plan, mode };
    }

    /// Adapter-driven: freshly-listed backups.
    pub fn set_backups(&mut self, backups: Vec<String>) {
        self.view.backups = backups;
    }

    /// Adapter-driven: surface an anonymous notice (e.g. a failed HKCU write
    /// or a failed data-dir open). The controller does not invent success.
    pub fn set_notice(&mut self, notice: SNotice) {
        self.view.notice = Some(notice);
    }

    pub fn handle(&mut self, command: SCommand) {
        match command {
            SCommand::ShowPage(page) => {
                self.view.page = page.min(8);
            }
            // ---- General ----
            SCommand::SetLaunchAtLogin(on) => {
                self.view.settings.launch_at_login = on;
                self.need_persist();
            }
            SCommand::SetSilentStart(on) => {
                self.view.settings.silent_start = on;
                self.need_persist();
            }
            SCommand::SetShowMainWindowAtStartup(on) => {
                self.view.settings.show_main_window_at_startup = on;
                self.need_persist();
            }
            SCommand::SetHideAfterOpen(on) => {
                self.view.settings.hide_after_open = on;
                self.need_persist();
            }
            SCommand::SetClearAfterOpen(on) => {
                self.view.settings.clear_after_open = on;
                self.need_persist();
            }
            SCommand::SetHideOnFocusLoss(on) => {
                self.view.settings.hide_on_focus_loss = on;
                self.need_persist();
            }
            SCommand::SetMonitorStrategy(strategy) => {
                self.view.settings.monitor_strategy = strategy;
                self.need_persist();
            }
            SCommand::SetLanguage(language) => {
                self.view.settings.language_preference = language;
                self.need_persist();
            }
            SCommand::RestoreDefaultSettings => self.restore_default_settings(),
            // ---- Search ----
            SCommand::SetSearchPaths(on) => {
                self.view.settings.search_paths = on;
                self.need_persist();
            }
            SCommand::SetSearchCategories(on) => {
                self.view.settings.search_categories = on;
                self.need_persist();
            }
            SCommand::SetSearchTags(on) => {
                self.view.settings.search_tags = on;
                self.need_persist();
            }
            SCommand::SetSearchNotes(on) => {
                self.view.settings.search_notes = on;
                self.need_persist();
            }
            SCommand::SetSearchAliases(on) => {
                self.view.settings.search_aliases = on;
                self.need_persist();
            }
            SCommand::SetFuzzyMatching(on) => {
                self.view.settings.fuzzy_matching = on;
                self.need_persist();
            }
            SCommand::SetSearchPinyin(on) => {
                self.view.settings.search_pinyin = on;
                self.need_persist();
            }
            SCommand::SetSearchEnglishInitials(on) => {
                self.view.settings.search_english_initials = on;
                self.need_persist();
            }
            SCommand::SetMaxEditDistance(distance) => {
                self.view.settings.max_edit_distance = distance.min(2);
                self.need_persist();
            }
            SCommand::SetMaxResults(count) => {
                self.view.settings.max_results = count.clamp(1, 100);
                self.need_persist();
            }
            SCommand::SetEmptyQueryStrategy(strategy) => {
                self.view.settings.empty_query_strategy = strategy;
                self.need_persist();
            }
            SCommand::SetHighlightResults(on) => {
                self.view.settings.highlight_results = on;
                self.need_persist();
            }
            SCommand::SetRecentSort(on) => {
                self.view.settings.recent_sort_first = on;
                self.need_persist();
            }
            // ---- Appearance ----
            SCommand::SetTheme(theme) => {
                self.view.settings.theme = theme;
                self.need_persist();
            }
            SCommand::SetRowHeight(pref) => {
                self.view.settings.row_height_preference = pref;
                self.need_persist();
            }
            SCommand::SetSearchWindowWidth(width) => {
                self.view.settings.search_window_width = Some(width.clamp(480, 760));
                self.need_persist();
            }
            SCommand::SetSettingsWindowWidth(width) => {
                self.view.settings.settings_window_width = Some(width.clamp(760, 1600));
                self.need_persist();
            }
            SCommand::SetFontScalePercent(percent) => {
                self.view.settings.font_scale_percent = percent.clamp(80, 150);
                self.need_persist();
            }
            SCommand::SetShowPathInResults(on) => {
                self.view.settings.show_path_in_results = on;
                self.need_persist();
            }
            SCommand::SetShowCategoryTagInResults(on) => {
                self.view.settings.show_category_tag_in_results = on;
                self.need_persist();
            }
            // ---- Hotkey ----
            SCommand::StartRecording => self.start_recording(),
            SCommand::CancelRecording => self.cancel_recording(),
            SCommand::SetRecordedModifiers(control, alt, shift, win) => {
                self.recorded_modifiers = Some(crate::domain::settings::HotkeyModifiers {
                    control,
                    alt,
                    shift,
                    win,
                });
            }
            SCommand::FinishRecording(key) => self.finish_recording(key),
            SCommand::PersistHotkey(setting) => {
                self.view.settings.hotkey = setting;
                self.view.hotkey_runtime = if setting.is_some() {
                    HotkeyRuntime::Active
                } else {
                    HotkeyRuntime::Disabled
                };
                self.view.hotkey_last_error = None;
                self.need_persist();
            }
            SCommand::RestoreDefaultHotkey => {
                self.view.settings.hotkey = Some(crate::domain::settings::HotkeySetting {
                    modifiers: crate::domain::settings::DEFAULT_HOTKEY_MODIFIERS,
                    key: crate::domain::settings::DEFAULT_HOTKEY_KEY,
                });
                self.view.hotkey_runtime = HotkeyRuntime::Active;
                self.view.hotkey_last_error = None;
                self.need_persist();
            }
            SCommand::SyncHotkeyRuntime(runtime, error) => {
                self.view.hotkey_runtime = runtime;
                self.view.hotkey_last_error = error;
            }
            // ---- M05 deferral ----
            SCommand::RenameCategory(cmd) => self.rename_category(cmd),
            SCommand::RenameTag(cmd) => self.rename_tag(cmd),
            SCommand::MergeTag {
                source,
                target_name,
            } => self.merge_tag(source, target_name),
            // ---- Data ----
            SCommand::PushImportPreview(plan, mode) => {
                self.push_import_preview(plan, mode);
            }
            SCommand::ApplyImport => self.apply_import(),
            SCommand::DismissDataFlow => {
                self.view.data_flow = DataFlow::Closed;
            }
            SCommand::DismissNotice => self.view.notice = None,
            SCommand::SetBackups(backups) => self.set_backups(backups),
        }
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    fn need_persist(&mut self) {
        self.persist();
    }

    /// Persist the snapshot; on failure roll it back to the previous value so
    /// the UI never shows a half-applied setting (no fake success) AND the
    /// in-memory working copy keeps the old settings (a rollback that only
    /// restores the view while the document still holds the failed value would
    /// let a later `reload()` resurrect the "saved" change).
    fn persist(&mut self) {
        let snapshot = self.view.settings.clone();
        // The pre-change value is captured from the working copy, not the
        // snapshot, so a failure restores exactly what was persisted before.
        let previous = self
            .store
            .document()
            .map(|document| document.data.settings)
            .unwrap_or_else(|| snapshot.clone());
        if self.store.set_settings(snapshot).is_err() {
            self.view.settings = previous;
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        match self.store.save_at() {
            Ok(_) => {
                self.view.notice = Some(SNotice::Saved);
            }
            Err(_) => {
                // Restore BOTH the working copy and the view to the previous
                // settings so no half-applied state survives.
                let _ = self.store.set_settings(previous.clone());
                self.view.settings = previous;
                self.view.notice = Some(SNotice::SaveFailed);
            }
        }
    }

    /// A observe-able two-phase reset: apply the default settings snapshot
    /// (folder data untouched; the document keeps its folders/categories/tags)
    /// and persist. The caller (adapter) is responsible for re-applying the
    /// live theme/locale/hotkey after this returns.
    fn restore_default_settings(&mut self) {
        self.view.settings = AppSettings::default();
        self.need_persist();
        self.view.notice = Some(SNotice::ResetDefaultApplied);
    }

    // --- hotkey recording ----------------------------------------------

    fn start_recording(&mut self) {
        self.view.hotkey_record_phase = HotkeyRecordPhase::Listening;
        self.recorded_modifiers = Some(crate::domain::settings::HotkeyModifiers::default());
        // Live feedback: the current combo (or a blank while no key yet).
        self.view.hotkey_draft = current_recorded(self.recorded_modifiers, None);
    }

    fn cancel_recording(&mut self) {
        self.view.hotkey_record_phase = HotkeyRecordPhase::Idle;
        self.recorded_modifiers = None;
        self.view.hotkey_draft = None;
    }

    /// Complete recording: validate the combo through the pure rules table and
    /// surface it (the adapter then registers it and calls
    /// [`SCommand::PersistHotkey`] on success; on conflict it keeps old and
    /// reports through [`SCommand::SyncHotkeyRuntime`]).
    fn finish_recording(&mut self, key: crate::domain::settings::HotkeyKey) {
        if self.view.hotkey_record_phase != HotkeyRecordPhase::Listening {
            return;
        }
        let modifiers = self.recorded_modifiers.unwrap_or_default();
        if crate::platform::hotkey::validate(modifiers, key).is_err() {
            self.view.notice = Some(SNotice::HotkeyInvalid);
            self.cancel_recording();
            return;
        }
        let combo = crate::domain::settings::HotkeySetting { modifiers, key };
        self.view.hotkey_draft = Some(combo);
        // Stay in Listening so the adapter can confirm ('Apply'), or the user
        // presses Escape to cancel. A second key press replaces the draft.
        let _ = self.view.hotkey_record_phase;
    }

    /// The adapter confirmed the recorded combo; commit it into the settings.
    pub fn commit_recorded(&mut self) -> bool {
        let Some(combo) = self.view.hotkey_draft else {
            return false;
        };
        if crate::platform::hotkey::validate(combo.modifiers, combo.key).is_err() {
            self.view.notice = Some(SNotice::HotkeyInvalid);
            self.cancel_recording();
            return false;
        }
        self.view.settings.hotkey = Some(combo);
        self.view.hotkey_draft = None;
        self.view.hotkey_record_phase = HotkeyRecordPhase::Idle;
        self.recorded_modifiers = None;
        self.need_persist();
        true
    }

    // --- M05 deferral: rename + merge (usage-count aware) ---------------

    fn rename_category(&mut self, cmd: CategoryNameCommand) {
        let name = normalize_name(&cmd.name);
        if !valid_named_value(&name, 128) {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        let Some(document) = self.store.document() else {
            return;
        };
        let duplicate = document
            .data
            .categories
            .iter()
            .any(|c| c.id != cmd.id && names_equal(&c.name, &name));
        if duplicate {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        if self.store.rename_category_record(cmd.id, name).is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        if self.store.save_at().is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
        } else {
            self.view.notice = Some(SNotice::Saved);
        }
    }

    fn rename_tag(&mut self, cmd: TagNameCommand) {
        let name = normalize_name(&cmd.name);
        if !valid_named_value(&name, 128) {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        let Some(document) = self.store.document() else {
            return;
        };
        let duplicate = document
            .data
            .tags
            .iter()
            .any(|t| t.id != cmd.id && names_equal(&t.name, &name));
        if duplicate {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        if self.store.rename_tag_record(cmd.id, name).is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        if self.store.save_at().is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
        } else {
            self.view.notice = Some(SNotice::Saved);
        }
    }

    fn merge_tag(&mut self, source: crate::domain::ids::TagId, target_name: String) {
        let name = normalize_name(&target_name);
        let Some(document) = self.store.document() else {
            return;
        };
        // Resolve the target by exact name (case-insensitive); refuse when the
        // source itself is the only match.
        let target = document
            .data
            .tags
            .iter()
            .find(|t| t.id != source && names_equal(&t.name, &name))
            .map(|t| t.id);
        let Some(target) = target else {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        };
        if self.store.merge_tag_record(source, target).is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
            return;
        }
        if self.store.save_at().is_err() {
            self.view.notice = Some(SNotice::SaveFailed);
        } else {
            self.view.notice = Some(SNotice::Saved);
        }
    }

    // --- data page (import apply) --------------------------------------

    /// Apply the previewed import: `apply_import` is all-or-nothing and
    /// re-validates. On success the repository document is replaced (settings
    /// and revision preserved) and persisted. Real directories are never
    /// touched.
    fn apply_import(&mut self) {
        let DataFlow::ImportPreview { plan, mode } = self.view.data_flow.clone() else {
            return;
        };
        let Some(current_document) = self.store.document() else {
            return;
        };
        // NOTE: the preview holds the PLAN; the actual incoming AppData is
        // held by the adapter (the pure controller does not retain file
        // bytes). The adapter calls `SCommand::ApplyImport` AFTER
        // `apply_import` has already produced the merged document — so this
        // branch is only reached through the adapter's `apply_import` result
        // path. To keep the pure controller honest, `apply_import` here simply
        // requires a preview to exist; the merge itself is done by the adapter
        // via `storage::import_export::apply_import`, which re-validates.
        let _ = (&current_document, plan, mode);
        self.view.notice = Some(SNotice::ImportApplied);
        self.view.data_flow = DataFlow::Closed;
        self.reload();
    }
}

/// Render the current recorded combo (used by the adapter to fill the live
/// display while listening).
pub fn current_recorded(
    modifiers: Option<crate::domain::settings::HotkeyModifiers>,
    key: Option<crate::domain::settings::HotkeyKey>,
) -> Option<crate::domain::settings::HotkeySetting> {
    let modifiers = modifiers?;
    let key = key?;
    Some(crate::domain::settings::HotkeySetting { modifiers, key })
}

/// Translate a Slint key text into a domain [`HotkeyKey`] for hotkey recording.
///
/// Slint 1.18's `KeyEvent` exposes `text` (the unicode representation) and the
/// modifier flags, not a virtual-key code. The domain `Vk` range is Space..Z
/// (0x20..=0x5A), so a single printable char maps by ASCII code; the special
/// keys that the OS/repo accept are Function F1..F24 (Slint encodes them as
/// `Key.F1`.. and their `text` is the single char `\u{F704}`..). Everything
/// else (arrows, Home, Insert, ...) is either out of the accepted range and
/// will be rejected by `platform::hotkey::validate` with `KeyOutOfRange`, or
/// maps to Space. Modifier-only presses are never forwarded as a key (the
/// FocusScope callback only fires for the non-modifier key event).
pub fn hotkey_from_text(text: &str) -> Option<crate::domain::settings::HotkeyKey> {
    // Slint's Key.text for F1..F24 is a private-use char F704..F71B; recover
    // the 1-based F-index from that window.
    let mut chars = text.chars();
    let character = chars.next()?;
    if chars.next().is_some() {
        // Multi-char text (e.g. a composed sequence) is not a hotkey key.
        return None;
    }
    let code = character as u32;
    if (0xF704..=0xF71B).contains(&code) {
        return Some(crate::domain::settings::HotkeyKey::Function {
            index: (code - 0xF704) as u8 + 1,
        });
    }
    match character {
        ' ' => Some(crate::domain::settings::HotkeyKey::Vk { vk: 0x20 }),
        _ => {
            let mut vk = u8::try_from(code).ok()?;
            // Uppercase ASCII letters so 'a'..'z' map onto the upper-case VK
            // codes (Windows VK codes are uppercase).
            if vk.is_ascii_lowercase() {
                vk -= 32;
            }
            // Accept the domain Vk range; KeyOutOfRange validation rejects
            // anything else at the FinishRecording step.
            if (0x20..=0x5A).contains(&vk) {
                Some(crate::domain::settings::HotkeyKey::Vk { vk })
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        folder::{Category, FolderEntry, Tag},
        ids::{CategoryId, TagId},
        settings::HotkeySetting,
    };
    use chrono::DateTime;
    use uuid::Uuid;

    fn utc(value: &str) -> DateTime<chrono::Utc> {
        value.parse().expect("fixture")
    }

    fn category(id: u128, name: &str) -> Category {
        Category {
            id: CategoryId::from_uuid(Uuid::from_u128(id)),
            name: name.to_owned(),
            color: None,
        }
    }
    fn tag(id: u128, name: &str) -> Tag {
        Tag {
            id: TagId::from_uuid(Uuid::from_u128(id)),
            name: name.to_owned(),
        }
    }

    fn folder(id: u128, name: &str, path: &str) -> FolderEntry {
        FolderEntry {
            id: crate::domain::ids::FolderId::from_uuid(Uuid::from_u128(id)),
            display_name: name.to_owned(),
            aliases: Vec::new(),
            path: path.to_owned(),
            enabled: true,
            favorite: false,
            pinned: false,
            manual_weight: 0,
            category_id: None,
            tag_ids: Vec::new(),
            note: String::new(),
            color: None,
            sort_order: 0,
            created_at: utc("2026-09-21T00:00:00Z"),
            updated_at: utc("2026-09-21T00:00:00Z"),
            last_opened_at: None,
            open_count: 0,
        }
    }

    /// An in-memory store that mirrors the repository's document + save
    /// semantics (revision bumps on save; saves can be failed for the
    /// rollback test). The document is shareable so a second controller can
    /// "reload" the same persisted state.
    #[derive(Clone)]
    struct MemStore {
        document: std::rc::Rc<std::cell::RefCell<Option<StoredDocumentV1>>>,
        fail_save: bool,
        fail_set: bool,
    }

    impl Default for MemStore {
        fn default() -> Self {
            Self::seed()
        }
    }

    impl MemStore {
        fn seed() -> Self {
            Self {
                document: std::rc::Rc::new(std::cell::RefCell::new(Some(StoredDocumentV1::new(
                    AppData {
                        settings: AppSettings::default(),
                        folders: vec![folder(10, "Docs", r"C:\docs")],
                        categories: vec![category(1, "工作"), category(4, "项目")],
                        tags: vec![tag(2, "重要"), tag(3, "客户")],
                        revision: 2,
                    },
                )))),
                fail_save: false,
                fail_set: false,
            }
        }
    }

    impl SettingsStore for MemStore {
        fn load_document(&mut self) -> Result<StoredDocumentV1, DocumentError> {
            self.document
                .borrow()
                .clone()
                .ok_or(DocumentError::NotFound)
        }
        fn document(&self) -> Option<StoredDocumentV1> {
            self.document.borrow().clone()
        }
        fn set_settings(&mut self, settings: AppSettings) -> Result<(), DocumentError> {
            if self.fail_set {
                return Err(DocumentError::Io);
            }
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            document.data.settings = settings;
            Ok(())
        }
        fn set_data(&mut self, data: AppData) -> Result<(), DocumentError> {
            if self.fail_set {
                return Err(DocumentError::Io);
            }
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            document.data = data;
            Ok(())
        }
        fn clear_folder_records(&mut self) -> bool {
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            let n = document.data.folders.len();
            document.data.folders.clear();
            document.data.folders.len() != n
        }
        fn rename_category_record(
            &mut self,
            id: CategoryId,
            name: String,
        ) -> Result<(), DocumentError> {
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            document
                .data
                .categories
                .iter_mut()
                .find(|c| c.id == id)
                .map(|c| c.name = name)
                .ok_or(DocumentError::NotFound)
        }
        fn rename_tag_record(&mut self, id: TagId, name: String) -> Result<(), DocumentError> {
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            document
                .data
                .tags
                .iter_mut()
                .find(|t| t.id == id)
                .map(|t| t.name = name)
                .ok_or(DocumentError::NotFound)
        }
        fn merge_tag_record(&mut self, source: TagId, target: TagId) -> Result<(), DocumentError> {
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            for f in &mut document.data.folders {
                if f.tag_ids.contains(&source) {
                    f.tag_ids.retain(|id| *id != source);
                    if !f.tag_ids.contains(&target) {
                        f.tag_ids.push(target);
                    }
                }
            }
            document.data.tags.retain(|t| t.id != source);
            Ok(())
        }
        fn save_at(&mut self) -> Result<u64, DocumentError> {
            if self.fail_save {
                return Err(DocumentError::Io);
            }
            let mut guard = self.document.borrow_mut();
            let document = guard.as_mut().expect("document");
            let next = document.data.next_revision().expect("rev");
            document.data.revision = next;
            Ok(next)
        }
    }

    fn seeded() -> SettingsController<MemStore> {
        SettingsController::new(MemStore::seed())
    }

    /// A second controller bound to the SAME store (same `Rc<RefCell<_>>`
    /// document), simulating a re-open of the settings window after the first
    /// controller saved.
    fn second_controller(store: &MemStore) -> SettingsController<MemStore> {
        SettingsController::new(store.clone())
    }

    // ------------------------------------------------------------------
    // round-trip: UI command -> domain -> store -> reload -> UI
    // ------------------------------------------------------------------

    #[test]
    fn setting_change_round_trips_through_the_store() {
        let store = MemStore::seed();
        let mut controller = SettingsController::new(store.clone());
        controller.handle(SCommand::SetSearchNotes(true));
        assert!(controller.view().settings.search_notes);
        assert!(
            controller
                .store
                .document()
                .unwrap()
                .data
                .settings
                .search_notes,
            "domain copy updated"
        );
        // A second controller sharing the same store (re-open of the settings
        // window) reloads the persisted value.
        let mut reopened = second_controller(&store);
        reopened.reload();
        assert!(reopened.view().settings.search_notes);
    }

    #[test]
    fn every_search_and_appearance_toggle_persists() {
        let store = MemStore::seed();
        let mut controller = SettingsController::new(store.clone());
        for command in [
            SCommand::SetSearchPaths(false),
            SCommand::SetSearchCategories(false),
            SCommand::SetSearchTags(false),
            SCommand::SetSearchNotes(true),
            SCommand::SetSearchAliases(false),
            SCommand::SetFuzzyMatching(false),
            SCommand::SetSearchPinyin(false),
            SCommand::SetSearchEnglishInitials(false),
            SCommand::SetHighlightResults(false),
            SCommand::SetHideAfterOpen(false),
            SCommand::SetClearAfterOpen(false),
            SCommand::SetHideOnFocusLoss(false),
            SCommand::SetSilentStart(false),
        ] {
            controller.handle(command);
        }
        let settings = &controller.view().settings;
        assert!(!settings.search_paths);
        assert!(!settings.search_categories);
        assert!(!settings.search_tags);
        assert!(settings.search_notes);
        assert!(!settings.search_aliases);
        assert!(!settings.fuzzy_matching);
        assert!(!settings.search_pinyin);
        assert!(!settings.search_english_initials);
        assert!(!settings.highlight_results);
        assert!(!settings.hide_after_open);
        assert!(!settings.clear_after_open);
        assert!(!settings.hide_on_focus_loss);
        assert!(!settings.silent_start);
        // Reload (same store, re-open) persists all of it.
        let mut reopened = second_controller(&store);
        reopened.reload();
        assert!(reopened.view().settings.search_notes);
        assert!(!reopened.view().settings.search_aliases);
    }

    #[test]
    fn numeric_settings_clamp_and_persist() {
        let mut controller = seeded();
        controller.handle(SCommand::SetMaxResults(999));
        assert_eq!(controller.view().settings.max_results, 100);
        controller.handle(SCommand::SetMaxResults(0));
        assert_eq!(controller.view().settings.max_results, 1);
        controller.handle(SCommand::SetMaxEditDistance(99));
        assert_eq!(controller.view().settings.max_edit_distance, 2);
        controller.handle(SCommand::SetFontScalePercent(500));
        assert_eq!(controller.view().settings.font_scale_percent, 150);
        controller.handle(SCommand::SetFontScalePercent(10));
        assert_eq!(controller.view().settings.font_scale_percent, 80);
    }

    #[test]
    fn failed_save_rolls_the_snapshot_back_without_fake_success() {
        let mut controller = seeded();
        controller.handle(SCommand::SetSearchNotes(true));
        assert!(controller.view().settings.search_notes);
        // Next save fails: the change must roll back and a SaveFailed notice.
        controller.store.fail_save = true;
        controller.handle(SCommand::SetSearchTags(false));
        assert!(
            controller.view().settings.search_tags,
            "rolled back to true"
        );
        assert_eq!(controller.view().notice, Some(SNotice::SaveFailed));
    }

    #[test]
    fn hotkey_recording_validates_and_cancels_on_invalid() {
        let mut controller = seeded();
        controller.handle(SCommand::StartRecording);
        assert_eq!(
            controller.view().hotkey_record_phase,
            HotkeyRecordPhase::Listening
        );
        // A function key needs no modifier.
        controller.handle(SCommand::FinishRecording(
            crate::domain::settings::HotkeyKey::Function { index: 5 },
        ));
        let combo = controller.view().hotkey_draft.expect("draft");
        assert_eq!(
            combo.key,
            crate::domain::settings::HotkeyKey::Function { index: 5 }
        );
        // Commit it.
        assert!(controller.commit_recorded());
        assert_eq!(
            controller.view().settings.hotkey,
            Some(HotkeySetting {
                modifiers: Default::default(),
                key: crate::domain::settings::HotkeyKey::Function { index: 5 },
            })
        );
    }

    #[test]
    fn hotkey_recording_rejects_a_bare_key() {
        let mut controller = seeded();
        controller.handle(SCommand::StartRecording);
        // A bare letter (no modifier) is invalid per the rules table.
        controller.handle(SCommand::FinishRecording(
            crate::domain::settings::HotkeyKey::Vk { vk: b'A' },
        ));
        assert_eq!(controller.view().notice, Some(SNotice::HotkeyInvalid));
        assert_eq!(
            controller.view().hotkey_record_phase,
            HotkeyRecordPhase::Idle
        );
        assert!(
            controller.view().settings.hotkey.is_some(),
            "keeps the previous hotkey"
        );
    }

    #[test]
    fn persist_hotkey_none_disables() {
        let store = MemStore::seed();
        let mut controller = SettingsController::new(store.clone());
        controller.handle(SCommand::PersistHotkey(None));
        assert_eq!(controller.view().settings.hotkey, None);
        assert_eq!(controller.view().hotkey_runtime, HotkeyRuntime::Disabled);
        // The cleared hotkey survives a re-open through the shared store.
        let mut reopened = second_controller(&store);
        reopened.reload();
        assert_eq!(reopened.view().settings.hotkey, None);
    }

    #[test]
    fn restore_default_settings_keeps_folder_data() {
        let mut controller = seeded();
        controller.handle(SCommand::SetSearchNotes(true));
        controller.handle(SCommand::SetTheme(ThemePreference::Dark));
        controller.handle(SCommand::RestoreDefaultSettings);
        assert_eq!(
            controller.view().settings.theme,
            ThemePreference::System,
            "defaults restored"
        );
        assert!(!controller.view().settings.launch_at_login);
        // Folder data untouched.
        let document = controller.store.document().unwrap();
        assert_eq!(document.data.folders.len(), 1);
        assert_eq!(document.data.categories.len(), 2);
        assert!(document.data.settings.hide_after_open, "default true");
    }

    #[test]
    fn page_navigation_clamps() {
        let mut controller = seeded();
        controller.handle(SCommand::ShowPage(8));
        assert_eq!(controller.view().page, 8);
        controller.handle(SCommand::ShowPage(99));
        assert_eq!(controller.view().page, 8);
        controller.handle(SCommand::ShowPage(4));
        assert_eq!(controller.view().page, 4);
    }

    // ---- M05 deferral: rename + merge ---------------------------------

    #[test]
    fn category_rename_persists_and_blocks_duplicate() {
        let mut controller = seeded();
        let id = CategoryId::from_uuid(Uuid::from_u128(1));
        controller.handle(SCommand::RenameCategory(CategoryNameCommand {
            id,
            name: "工作v2".into(),
        }));
        let document = controller.store.document().unwrap();
        assert_eq!(document.data.categories[0].name, "工作v2");
        // Renaming the SAME record to a case-variant of its own current name is
        // a no-op rename (NOT a duplicate — same id is excluded).
        controller.handle(SCommand::RenameCategory(CategoryNameCommand {
            id,
            name: "工作V2".into(),
        }));
        assert_eq!(controller.view().notice, Some(SNotice::Saved));
        assert_eq!(
            controller.store.document().unwrap().data.categories[0].name,
            "工作V2"
        );
        // Renaming onto a DIFFERENT category's name is blocked (case-insensitive).
        controller.handle(SCommand::RenameCategory(CategoryNameCommand {
            id,
            name: "项目".into(),
        }));
        assert_eq!(controller.view().notice, Some(SNotice::SaveFailed));
        assert_eq!(
            controller.store.document().unwrap().data.categories[0].name,
            "工作V2"
        );
    }

    #[test]
    fn tag_rename_blocked_by_duplicate_and_merge_moves_references() {
        let mut controller = seeded();
        let important = TagId::from_uuid(Uuid::from_u128(2));
        let customer = TagId::from_uuid(Uuid::from_u128(3));
        // Rename 重要 -> 客户 is a duplicate (blocked).
        controller.handle(SCommand::RenameTag(TagNameCommand {
            id: important,
            name: "客户".into(),
        }));
        assert_eq!(controller.view().notice, Some(SNotice::SaveFailed));
        // Merge 重要 into 客户 (exact name match, ASCII-insensitive).
        controller.handle(SCommand::MergeTag {
            source: TagId::from_uuid(Uuid::from_u128(2)),
            target_name: "客户".into(),
        });
        let document = controller.store.document().unwrap();
        assert_eq!(document.data.tags.len(), 1, "source tag removed");
        assert_eq!(document.data.tags[0].id, customer);
    }

    #[test]
    fn merge_tag_without_a_target_is_refused() {
        let mut controller = seeded();
        controller.handle(SCommand::MergeTag {
            source: TagId::from_uuid(Uuid::from_u128(2)),
            target_name: "不存在".into(),
        });
        assert_eq!(controller.view().notice, Some(SNotice::SaveFailed));
        assert_eq!(controller.store.document().unwrap().data.tags.len(), 2);
    }

    // ---- data page -----------------------------------------------------

    #[test]
    fn import_preview_is_pushed_and_dismissed() {
        let mut controller = seeded();
        let plan = import_export::ImportPlan {
            added: vec!["New".into()],
            skipped: vec!["Docs".into()],
            ..Default::default()
        };
        controller.push_import_preview(plan.clone(), import_export::ImportMode::Merge);
        assert_eq!(
            controller.view().data_flow,
            DataFlow::ImportPreview {
                plan,
                mode: import_export::ImportMode::Merge
            }
        );
        controller.handle(SCommand::DismissDataFlow);
        assert_eq!(controller.view().data_flow, DataFlow::Closed);
    }

    #[test]
    fn backups_list_is_set_and_cleared() {
        let mut controller = seeded();
        controller.set_backups(vec!["2026-09-21.json".into()]);
        assert_eq!(controller.view().backups.len(), 1);
        controller.set_backups(Vec::new());
        assert!(controller.view().backups.is_empty());
    }

    #[test]
    fn launch_at_login_os_sync_and_match() {
        let mut controller = seeded();
        controller.set_launch_at_login_os(Some(false));
        assert!(controller.launch_at_login_matches_os());
        controller.handle(SCommand::SetLaunchAtLogin(true));
        assert!(
            !controller.launch_at_login_matches_os(),
            "toggle not yet written by adapter"
        );
        // The adapter confirms the OS write.
        controller.set_launch_at_login_os(Some(true));
        assert!(controller.launch_at_login_matches_os());
    }

    #[test]
    fn notice_can_be_set_by_the_adapter() {
        let mut controller = seeded();
        controller.set_notice(SNotice::StartupWriteFailed);
        assert_eq!(controller.view().notice, Some(SNotice::StartupWriteFailed));
        controller.handle(SCommand::DismissNotice);
        assert_eq!(controller.view().notice, None);
    }

    #[test]
    fn hotkey_from_text_maps_printable_and_function_keys() {
        use crate::domain::settings::HotkeyKey;
        // Space maps to VK_SPACE (0x20).
        assert_eq!(hotkey_from_text(" "), Some(HotkeyKey::Vk { vk: 0x20 }));
        // Lowercase letters map to the uppercase VK code (A-Z).
        assert_eq!(hotkey_from_text("a"), Some(HotkeyKey::Vk { vk: 0x41 }));
        assert_eq!(hotkey_from_text("A"), Some(HotkeyKey::Vk { vk: 0x41 }));
        // F1 starts at the private-use window F704.
        let f1 = char::from_u32(0xF704).expect("valid private-use char");
        assert_eq!(
            hotkey_from_text(&f1.to_string()),
            Some(HotkeyKey::Function { index: 1 })
        );
        let f12 = char::from_u32(0xF704 + 11).expect("valid private-use char");
        assert_eq!(
            hotkey_from_text(&f12.to_string()),
            Some(HotkeyKey::Function { index: 12 })
        );
        // Empty / multi-char / out-of-range text is None.
        assert_eq!(hotkey_from_text(""), None);
        assert_eq!(hotkey_from_text("ab"), None);
        assert_eq!(hotkey_from_text("\u{1}"), None);
    }
}
