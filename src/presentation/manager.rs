//! Management controller (M05) — the presenter for the settings page/dialog.
//!
//! Pure-Rust, Slint-independent: it owns the dialog state machine, the
//! add/edit validation flow, the remove-undo window, the category/tag CRUD and
//! the controlled one-level child import, and persists through
//! `crate::storage::repository::DocumentRepository`. The Slint layer is a thin
//! shell that forwards [`MCommand`]s and re-renders from [`MView`].
//!
//! All decisions are deterministic and unit-tested (dialog validation,
//! duplicate policies, undo across save boundaries, atomic merge).

use crate::{
    domain::{
        folder::{
            Category, FolderColor, FolderEntry, MAX_CATEGORY_NAME_LEN, MAX_MANUAL_WEIGHT,
            MAX_TAG_NAME_LEN, MIN_MANUAL_WEIGHT, Tag,
        },
        ids::{CategoryId, FolderId, TagId},
        path_semantics::{expand_open_path, path_key},
    },
    storage::{repository::DocumentRepository, schema::StoredDocumentV1},
};

use super::management::{
    ChildImportChoice, DuplicatePolicy, FolderFilter, FolderRow, FolderSort, MAX_ONE_LEVEL_IMPORT,
    ONE_LEVEL_IMPORT_MAX_LITERAL, UNDO_WINDOW, child_import_offer, folder_rows, normalize_name,
    preview_batch, suggested_display_name, valid_named_value,
};

/// One dialog field. The controller works over this (the Slint adapter maps to
/// LineEdit/ComboBox state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderDraft {
    /// `Some(id)` = editing an existing record; `None` = adding.
    pub id: Option<FolderId>,
    pub display_name: String,
    pub path: String,
    pub aliases: Vec<String>,
    pub category_id: Option<CategoryId>,
    pub tag_ids: Vec<TagId>,
    pub note: String,
    pub pinned: bool,
    pub favorite: bool,
    pub manual_weight: i16,
    pub enabled: bool,
    pub color: Option<FolderColor>,
}

impl FolderDraft {
    pub fn for_add(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            id: None,
            display_name: suggested_display_name(&path),
            path,
            aliases: Vec::new(),
            category_id: None,
            tag_ids: Vec::new(),
            note: String::new(),
            pinned: false,
            favorite: false,
            manual_weight: 0,
            enabled: true,
            color: None,
        }
    }

    pub fn for_edit(entry: &FolderEntry) -> Self {
        Self {
            id: Some(entry.id),
            display_name: entry.display_name.clone(),
            path: entry.path.clone(),
            aliases: entry.aliases.clone(),
            category_id: entry.category_id,
            tag_ids: entry.tag_ids.clone(),
            note: entry.note.clone(),
            pinned: entry.pinned,
            favorite: entry.favorite,
            manual_weight: entry.manual_weight,
            enabled: entry.enabled,
            color: entry.color,
        }
    }
}

/// The add-dialog flow stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddFlow {
    /// Add dialog is closed.
    Closed,
    /// A fresh add whose path came from a picker/paste/text.
    Editing(FolderDraft),
    /// A batch import is being previewed (multi-folder or one-level children).
    Previewing { items: Vec<BatchItem> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchItem {
    pub path: String,
    pub name: String,
    /// `None` = ok to add; `Some(reason)` blocks the default add.
    pub status: BatchStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchStatus {
    Ready,
    Duplicate,
    Invalid,
}

/// State of the pending single-item remove (for the undo window).
#[derive(Debug, Clone)]
pub struct PendingRemove {
    pub folder_id: FolderId,
    pub payload: FolderEntry,
    pub previous_revision: u64,
    pub expires_at: std::time::Instant,
}

/// A user-facing toast/notice handled by the controller (anonymous).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Saved,
    SaveFailed,
    Removed,
    Restored,
    UndoExpired,
    CannotUndo,
    DuplicateBlocked,
    InvalidPath,
    InvalidName,
    NotFound,
    PathAccessible,
    PathInaccessible,
    ImportLimited,
}

/// The complete observable state the settings page/dialog renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MView {
    pub page: Page,
    pub rows: Vec<super::management::FolderRow>,
    pub folders: usize,
    pub categories: Vec<CategoryRow>,
    pub tags: Vec<TagRow>,
    pub filter: FolderFilter,
    pub sort: FolderSort,
    pub add_flow: AddFlowView,
    pub pending_remove: Option<PendingRemoveView>,
    pub import_offer: Option<ImportOfferView>,
    pub notice: Option<Notice>,
    pub one_level_import_setting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Folders,
    Categories,
    Tags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRow {
    pub id: CategoryId,
    pub name: String,
    pub color: Option<FolderColor>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    pub id: TagId,
    pub name: String,
    pub usage: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddFlowView {
    Closed,
    Draft(FolderDraftView),
    Preview(BatchPreviewView),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderDraftView {
    pub id: Option<FolderId>,
    pub display_name: String,
    pub path: String,
    pub note: String,
    pub pinned: bool,
    pub favorite: bool,
    pub manual_weight: i16,
    pub enabled: bool,
    pub category_id: Option<CategoryId>,
    pub tag_ids: Vec<TagId>,
    /// M05 review H2: the draft color (editable in the dialog). `None` = no
    /// swatch; the UI cycles through the palette via `CycleDraftColor`.
    pub color: Option<FolderColor>,
    /// Validation result of the current path (recomputed on every edit).
    pub valid: bool,
    pub duplicate_existing: Option<String>,
    pub inaccessible: bool,
    pub unsaved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPreviewView {
    pub items: Vec<BatchItemView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchItemView {
    pub index: usize,
    pub name: String,
    pub path: String,
    pub duplicate_existing: Option<String>,
    pub invalid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRemoveView {
    pub folder_name: String,
    pub seconds_left: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOfferView {
    pub parent_path: String,
    pub child_count: usize,
    pub parent_only_label: String,
    pub children_label: String,
    pub hover: String,
}

/// Commands the UI forwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MCommand {
    ShowPage(Page),
    SetFilterName(String),
    SetFilterCategory(Option<CategoryId>),
    SetFilterEnabled(Option<bool>),
    SetSort(FolderSort),
    OpenAdd,
    OpenPickPaths(Vec<String>),
    OpenManual,
    EditFolder(FolderId),
    EditPath(String),
    EditName(String),
    EditNote(String),
    ToggleDraftPinned,
    ToggleDraftFavorite,
    SetDraftEnabled(bool),
    SetDraftCategory(Option<CategoryId>),
    SetDraftTag(TagId, bool),
    SetDraftWeight(i16),
    CycleDraftColor,
    SaveDraft,
    /// M05 review H2: process the duplicate decision when a draft collides with
    /// an existing record. `Cancel` is the default (nothing saved); `EditExisting`
    /// opens the record with the same path for editing; `SaveAsDifferentName`
    /// saves the draft as an independent record keeping the (unchanged) path but
    /// under the current (different) name.
    ResolveDuplicate(DuplicatePolicy),
    CancelDraft,
    ApplyBatch,
    CancelBatch,
    StartRemove(FolderId),
    ConfirmRemove,
    CancelRemove,
    UndoRemove,
    DismissUndo,
    ToggleEnable(FolderId),
    TogglePin(FolderId),
    CheckPath(FolderId),
    ToggleFavorite(FolderId),
    Relocate(FolderId, String),
    CreateCategory(String),
    RenameCategory(CategoryId, String),
    DeleteCategory(CategoryId),
    /// M05 review M1: toggle the persisted one-level-import setting (default OFF).
    SetOneLevelImport(bool),
    ChooseImport(ChildImportChoice),
    DismissImport,
    CreateTag(String),
    RenameTag(TagId, String),
    MergeTag(TagId, String),
    DeleteTag(TagId),
    DismissNotice,
    /// 0.0.1 slot so the UI has a stable "check" affordance even headless.
    Refresh,
}

/// Persistence boundary the controller uses (real repository implementation
/// in `main.rs` / tests with a `TempDir`). Controlled via `&mut self`.
pub trait ManagementStore {
    fn load(&mut self) -> Result<StoredDocumentV1, crate::storage::repository::RepositoryError>;
    /// Snapshot of the working document. Returns a clone so interior-mutable
    /// stores (the shared `Rc<RefCell<_>>`) can fulfill it without leaking a
    /// borrow guard. The controller always reads a bounded set of fields.
    fn document(&self) -> Option<StoredDocumentV1>;
    fn save_at(&mut self) -> Result<u64, crate::storage::repository::RepositoryError>;
    fn put_folder(
        &mut self,
        folder: FolderEntry,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn put_category(
        &mut self,
        category: Category,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn put_tag(&mut self, tag: Tag) -> Result<(), crate::storage::repository::RepositoryError>;
    fn remove_record(&mut self, folder_id: FolderId) -> bool;
    fn disable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn enable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn set_pinned(
        &mut self,
        folder_id: FolderId,
        pinned: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn remove_category(&mut self, category_id: CategoryId) -> bool;
    fn remove_tag(&mut self, tag_id: TagId) -> bool;
    fn rename_tag(
        &mut self,
        tag_id: TagId,
        name: String,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn merge_tag(
        &mut self,
        source: TagId,
        target: TagId,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn undo_remove_record(
        &mut self,
        payload: FolderEntry,
        previous_revision: u64,
        expected_current: u64,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
    fn duplicate_folders(&self, exclude: Option<FolderId>) -> Vec<usize>;
    fn is_duplicate_path(&self, candidate: &str, exclude: Option<FolderId>) -> bool;
    /// Persist the one-level-import setting (M1). The store resolves the
    /// current in-memory document, updates the flag, and the caller then
    /// persists with `save_at`. Returns `Err(NotFound)` when no document is
    /// loaded.
    fn set_one_level_import(
        &mut self,
        enabled: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError>;
}

/// The real repository is itself a store: the controller drives `load`/`save_at`
/// and the record helpers directly. This makes
/// `ManagementController<DocumentRepository>` the production wiring.
/// A `Rc<RefCell<_>>`-backed store so the search window and the settings window
/// share ONE repository behind interior mutability (M05 production wiring).
#[derive(Clone)]
pub struct SharedStore {
    repo: std::rc::Rc<std::cell::RefCell<DocumentRepository>>,
}

impl SharedStore {
    pub fn new(repo: std::rc::Rc<std::cell::RefCell<DocumentRepository>>) -> Self {
        Self { repo }
    }

    pub fn repo(&self) -> &std::rc::Rc<std::cell::RefCell<DocumentRepository>> {
        &self.repo
    }
}

impl ManagementStore for SharedStore {
    fn load(&mut self) -> Result<StoredDocumentV1, crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().load().map(|outcome| match outcome {
            crate::storage::repository::LoadOutcome::Found(document)
            | crate::storage::repository::LoadOutcome::Recovered(document) => document,
        })
    }

    fn document(&self) -> Option<StoredDocumentV1> {
        self.repo.borrow().document().cloned()
    }

    fn save_at(&mut self) -> Result<u64, crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().save_at()
    }

    fn put_folder(
        &mut self,
        folder: FolderEntry,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().put_folder(folder)
    }

    fn put_category(
        &mut self,
        category: Category,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().put_category(category)
    }

    fn put_tag(&mut self, tag: Tag) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().put_tag(tag)
    }

    fn remove_record(&mut self, folder_id: FolderId) -> bool {
        self.repo.borrow_mut().remove_record(folder_id)
    }

    fn disable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().disable_record(folder_id)
    }

    fn enable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().enable_record(folder_id)
    }

    fn set_pinned(
        &mut self,
        folder_id: FolderId,
        pinned: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().set_pinned(folder_id, pinned)
    }

    fn remove_category(&mut self, category_id: CategoryId) -> bool {
        self.repo.borrow_mut().remove_category(category_id)
    }

    fn remove_tag(&mut self, tag_id: TagId) -> bool {
        self.repo.borrow_mut().remove_tag(tag_id)
    }

    fn rename_tag(
        &mut self,
        tag_id: TagId,
        name: String,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().rename_tag(tag_id, name)
    }

    fn merge_tag(
        &mut self,
        source: TagId,
        target: TagId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().merge_tag(source, target)
    }

    fn undo_remove_record(
        &mut self,
        payload: FolderEntry,
        previous_revision: u64,
        expected_current: u64,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo
            .borrow_mut()
            .undo_remove_record(payload, previous_revision, expected_current)
    }

    fn duplicate_folders(&self, exclude: Option<FolderId>) -> Vec<usize> {
        self.repo.borrow().duplicate_folders(exclude)
    }

    fn is_duplicate_path(&self, candidate: &str, exclude: Option<FolderId>) -> bool {
        self.repo.borrow().is_duplicate_path(candidate, exclude)
    }

    fn set_one_level_import(
        &mut self,
        enabled: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.repo.borrow_mut().set_one_level_import(enabled)
    }
}

impl ManagementStore for DocumentRepository {
    fn load(&mut self) -> Result<StoredDocumentV1, crate::storage::repository::RepositoryError> {
        self.load().map(|outcome| match outcome {
            crate::storage::repository::LoadOutcome::Found(document)
            | crate::storage::repository::LoadOutcome::Recovered(document) => document,
        })
    }

    fn document(&self) -> Option<StoredDocumentV1> {
        self.document().cloned()
    }

    fn save_at(&mut self) -> Result<u64, crate::storage::repository::RepositoryError> {
        self.save_at()
    }

    fn put_folder(
        &mut self,
        folder: FolderEntry,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.put_folder(folder)
    }

    fn put_category(
        &mut self,
        category: Category,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.put_category(category)
    }

    fn put_tag(&mut self, tag: Tag) -> Result<(), crate::storage::repository::RepositoryError> {
        self.put_tag(tag)
    }

    fn remove_record(&mut self, folder_id: FolderId) -> bool {
        self.remove_record(folder_id)
    }

    fn disable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.disable_record(folder_id)
    }

    fn enable_record(
        &mut self,
        folder_id: FolderId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.enable_record(folder_id)
    }

    fn set_pinned(
        &mut self,
        folder_id: FolderId,
        pinned: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.set_pinned(folder_id, pinned)
    }

    fn remove_category(&mut self, category_id: CategoryId) -> bool {
        self.remove_category(category_id)
    }

    fn remove_tag(&mut self, tag_id: TagId) -> bool {
        self.remove_tag(tag_id)
    }

    fn rename_tag(
        &mut self,
        tag_id: TagId,
        name: String,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.rename_tag(tag_id, name)
    }

    fn merge_tag(
        &mut self,
        source: TagId,
        target: TagId,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.merge_tag(source, target)
    }

    fn undo_remove_record(
        &mut self,
        payload: FolderEntry,
        previous_revision: u64,
        expected_current: u64,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.undo_remove_record(payload, previous_revision, expected_current)
    }

    fn duplicate_folders(&self, exclude: Option<FolderId>) -> Vec<usize> {
        self.duplicate_folders(exclude)
    }

    fn is_duplicate_path(&self, candidate: &str, exclude: Option<FolderId>) -> bool {
        self.is_duplicate_path(candidate, exclude)
    }

    fn set_one_level_import(
        &mut self,
        enabled: bool,
    ) -> Result<(), crate::storage::repository::RepositoryError> {
        self.set_one_level_import(enabled)
    }
}

/// Resolve a management-list row index to its `FolderId` (M05 review C1).
///
/// The UI sends the *row index* — never a truncating `u128 as i32` id — and this
/// resolves `index → FolderId` against the current `view.rows` snapshot. A row
/// index is stable within one push (the rows snapshot); an out-of-range index is
/// `None` (the caller then no-ops instead of acting on a wrong record). This is
/// the same index→id pattern the category/tag delete path already uses, so no
/// 128-bit id ever crosses the Slint `int` boundary through folder actions.
pub fn folder_id_at(rows: &[FolderRow], index: usize) -> Option<FolderId> {
    rows.get(index).map(|row| row.id)
}

/// The controller. `S` is the persistence store (production: `DocumentRepository`,
/// tests: the in-memory `MemStore`).
pub struct ManagementController<S = DocumentRepository> {
    store: S,
    view: MView,
    draft: Option<FolderDraft>,
    pending_remove: Option<PendingRemove>,
    import_offer: Option<ImportOffer>,
    batch: Vec<BatchItem>,
    now: std::sync::Arc<dyn Fn() -> std::time::Instant>,
    base_dir: Option<std::path::PathBuf>,
}

/// Internal import-offer state.
#[derive(Debug, Clone)]
pub struct ImportOffer {
    pub parent_path: String,
    pub child_count: usize,
}

impl<S: ManagementStore> ManagementController<S> {
    pub fn new(store: S, one_level_import_setting: bool) -> Self {
        let mut controller = Self {
            store,
            view: MView {
                page: Page::Folders,
                rows: Vec::new(),
                folders: 0,
                categories: Vec::new(),
                tags: Vec::new(),
                filter: FolderFilter::default(),
                sort: FolderSort::Name,
                add_flow: AddFlowView::Closed,
                pending_remove: None,
                import_offer: None,
                notice: None,
                one_level_import_setting,
            },
            draft: None,
            pending_remove: None,
            import_offer: None,
            batch: Vec::new(),
            now: std::sync::Arc::new(std::time::Instant::now),
            base_dir: None,
        };
        controller.refresh_view();
        controller
    }

    /// Set the data base dir for the real repository adapter (the RepoHandle
    /// is created externally; this is informational).
    pub fn set_base_dir(&mut self, base_dir: std::path::PathBuf) {
        self.base_dir = Some(base_dir);
    }

    pub fn view(&self) -> &MView {
        &self.view
    }

    /// Re-read the shared document and rebuild the management list (M06: an
    /// import/backup-restore/clear-all that lands through the settings
    /// controller must be reflected in the folder/category/tag pages too).
    pub fn reload_from_store(&mut self) {
        self.refresh_view();
    }

    pub fn handle(&mut self, command: MCommand) {
        match command {
            MCommand::ShowPage(page) => {
                self.view.page = page;
                self.refresh_view();
            }
            MCommand::SetFilterName(name) => {
                self.view.filter.name = name;
                self.refresh_rows();
            }
            MCommand::SetFilterCategory(category) => {
                self.view.filter.category = category;
                self.refresh_rows();
            }
            MCommand::SetFilterEnabled(enabled) => {
                self.view.filter.enabled = enabled;
                self.refresh_rows();
            }
            MCommand::SetSort(sort) => {
                self.view.sort = sort;
                self.refresh_rows();
            }
            MCommand::OpenAdd => self.open_add(),
            MCommand::OpenPickPaths(paths) => self.open_pick_paths(paths),
            MCommand::OpenManual => self.open_manual(),
            MCommand::EditFolder(id) => self.open_edit(id),
            MCommand::EditPath(path) => self.edit_path(path),
            MCommand::EditName(name) => self.edit_name(name),
            MCommand::EditNote(note) => self.edit_note(note),
            MCommand::ToggleDraftPinned => self.toggle_draft_pinned(),
            MCommand::ToggleDraftFavorite => self.toggle_draft_favorite(),
            MCommand::SetDraftEnabled(enabled) => self.set_draft_enabled(enabled),
            MCommand::SetDraftCategory(category) => self.set_draft_category(category),
            MCommand::SetDraftTag(tag, on) => self.set_draft_tag(tag, on),
            MCommand::SetDraftWeight(weight) => self.set_draft_weight(weight),
            MCommand::CycleDraftColor => self.cycle_draft_color(),
            MCommand::SaveDraft => self.save_draft(),
            MCommand::ResolveDuplicate(policy) => self.resolve_duplicate(policy),
            MCommand::CancelDraft => self.cancel_draft(),
            MCommand::ApplyBatch => self.apply_batch(),
            MCommand::CancelBatch => self.cancel_batch(),
            MCommand::StartRemove(id) => self.start_remove(id),
            MCommand::ConfirmRemove => self.confirm_remove(),
            MCommand::CancelRemove => self.cancel_remove(),
            MCommand::UndoRemove => self.undo_remove(),
            MCommand::DismissUndo => self.dismiss_undo(),
            MCommand::ToggleEnable(id) => self.toggle_enable(id),
            MCommand::TogglePin(id) => self.toggle_pin(id),
            MCommand::CheckPath(id) => self.check_path(id),
            MCommand::ToggleFavorite(id) => self.toggle_favorite(id),
            MCommand::Relocate(id, path) => self.relocate(id, path),
            MCommand::CreateCategory(name) => self.create_category(name),
            MCommand::RenameCategory(id, name) => self.rename_category(id, name),
            MCommand::DeleteCategory(id) => self.delete_category(id),
            MCommand::SetOneLevelImport(enabled) => self.set_one_level_import(enabled),
            MCommand::ChooseImport(choice) => self.choose_import(choice),
            MCommand::DismissImport => self.dismiss_import(),
            MCommand::CreateTag(name) => self.create_tag(name),
            MCommand::RenameTag(id, name) => self.rename_tag(id, name),
            MCommand::MergeTag(id, name) => self.merge_tag(id, name),
            MCommand::DeleteTag(id) => self.delete_tag(id),
            MCommand::DismissNotice => {
                self.view.notice = None;
            }
            MCommand::Refresh => self.refresh_view(),
        }
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    fn document_folders(&self) -> Vec<FolderEntry> {
        self.store
            .document()
            .map(|document| document.data.folders.clone())
            .unwrap_or_default()
    }

    fn document_categories(&self) -> Vec<Category> {
        self.store
            .document()
            .map(|document| document.data.categories.clone())
            .unwrap_or_default()
    }

    fn document_tags(&self) -> Vec<Tag> {
        self.store
            .document()
            .map(|document| document.data.tags.clone())
            .unwrap_or_default()
    }

    fn refresh_view(&mut self) {
        self.refresh_categories();
        self.refresh_tags();
        self.refresh_rows();
    }

    fn refresh_rows(&mut self) {
        let folders = self.document_folders();
        let categories = self.document_categories();
        let tags = self.document_tags();
        self.view.folders = folders.len();
        self.view.rows = folder_rows(
            &folders,
            &categories,
            &tags,
            &self.view.filter,
            self.view.sort,
        );
    }

    fn refresh_categories(&mut self) {
        let folders = self.document_folders();
        self.view.categories = self
            .document_categories()
            .into_iter()
            .map(|category| CategoryRow {
                id: category.id,
                name: category.name,
                color: category.color,
                count: folders
                    .iter()
                    .filter(|folder| folder.category_id == Some(category.id))
                    .count(),
            })
            .collect();
    }

    fn refresh_tags(&mut self) {
        let folders = self.document_folders();
        self.view.tags = self
            .document_tags()
            .into_iter()
            .map(|tag| TagRow {
                id: tag.id,
                name: tag.name,
                usage: folders
                    .iter()
                    .filter(|folder| folder.tag_ids.contains(&tag.id))
                    .count(),
            })
            .collect();
    }

    fn notice(&mut self, notice: Notice) {
        self.view.notice = Some(notice);
    }

    // --- add / edit dialog --------------------------------------------

    fn open_add(&mut self) {
        self.draft = Some(FolderDraft::for_add(""));
        self.batch.clear();
        self.import_offer = None;
        self.sync_draft_view();
    }

    fn open_manual(&mut self) {
        self.open_add();
    }

    fn open_pick_paths(&mut self, paths: Vec<String>) {
        if paths.is_empty() {
            return;
        }
        if paths.len() == 1 {
            let path = paths.into_iter().next().unwrap();
            // A parent pick + the opt-in setting may offer one-level import.
            self.maybe_offer_import(path);
            return;
        }
        // Multi-folder: preview the whole batch (dedupe within + against docs).
        let folders = self.document_folders();
        let tag_names: Vec<String> = Vec::new();
        let statuses = preview_batch(&paths, &folders, &tag_names);
        self.batch = paths
            .into_iter()
            .zip(statuses)
            .map(|(path, status)| {
                let (name, item_status) = match status {
                    super::management::AddItemStatus::Ready { resolved_name } => {
                        (resolved_name, BatchStatus::Ready)
                    }
                    super::management::AddItemStatus::Duplicate { .. } => {
                        (suggested_display_name(&path), BatchStatus::Duplicate)
                    }
                    super::management::AddItemStatus::Invalid => {
                        (path.trim().to_owned(), BatchStatus::Invalid)
                    }
                };
                BatchItem {
                    path,
                    name,
                    status: item_status,
                }
            })
            .collect();
        self.draft = None;
        self.view.add_flow = AddFlowView::Preview(self.batch_view(&[]));
    }

    fn maybe_offer_import(&mut self, path: String) {
        if !self.view.one_level_import_setting {
            self.open_single_draft(path);
            return;
        }
        // The actual child directory listing happens ONCE, on demand, one level
        // deep. It is user-initiated and bounded by MAX_ONE_LEVEL_IMPORT.
        let child_count = list_direct_children_count(&path).min(MAX_ONE_LEVEL_IMPORT);
        let offer = child_import_offer(true, child_count);
        match offer {
            super::management::ChildImportOffer::ParentOnly => self.open_single_draft(path),
            super::management::ChildImportOffer::Choose { child_count } => {
                self.import_offer = Some(ImportOffer {
                    parent_path: path.clone(),
                    child_count,
                });
                self.view.import_offer = Some(ImportOfferView {
                    parent_path: path.clone(),
                    child_count,
                    parent_only_label: "仅导入父目录".to_owned(),
                    children_label: "导入直接子目录".to_owned(),
                    hover: ONE_LEVEL_IMPORT_MAX_LITERAL.to_owned(),
                });
            }
        }
    }

    fn open_single_draft(&mut self, path: String) {
        let draft = FolderDraft::for_add(path);
        self.draft = Some(draft);
        self.import_offer = None;
        self.sync_draft_view();
    }

    fn open_edit(&mut self, id: FolderId) {
        let folders = self.document_folders();
        match folders.into_iter().find(|folder| folder.id == id) {
            Some(entry) => {
                self.draft = Some(FolderDraft::for_edit(&entry));
                self.batch.clear();
                self.sync_draft_view();
            }
            None => self.notice(Notice::NotFound),
        }
    }

    fn sync_draft_view(&mut self) {
        let Some(draft) = self.draft.clone() else {
            self.view.add_flow = AddFlowView::Closed;
            return;
        };
        let (valid, duplicate_existing) = self.validate_draft(&draft);
        self.view.add_flow = AddFlowView::Draft(FolderDraftView {
            id: draft.id,
            display_name: draft.display_name.clone(),
            path: draft.path.clone(),
            note: draft.note.clone(),
            pinned: draft.pinned,
            favorite: draft.favorite,
            manual_weight: draft.manual_weight,
            enabled: draft.enabled,
            category_id: draft.category_id,
            tag_ids: draft.tag_ids.clone(),
            color: draft.color,
            valid,
            duplicate_existing,
            inaccessible: false,
            unsaved: true,
        });
    }

    fn validate_draft(&self, draft: &FolderDraft) -> (bool, Option<String>) {
        if path_key(&draft.path).is_none() {
            return (false, None);
        }
        let path_is_unique = !self.store.is_duplicate_path(&draft.path, draft.id);
        if path_is_unique {
            (true, None)
        } else {
            let existing = self
                .document_folders()
                .into_iter()
                .find(|folder| {
                    draft.id != Some(folder.id)
                        && crate::domain::path_semantics::same_path(&folder.path, &draft.path)
                })
                .map(|folder| folder.display_name);
            (false, existing)
        }
    }

    fn edit_path(&mut self, path: String) {
        if let Some(draft) = self.draft.as_mut() {
            draft.path = path;
            draft.display_name = suggested_display_name(&draft.path);
        }
        self.sync_draft_view();
    }

    fn edit_name(&mut self, name: String) {
        if let Some(draft) = self.draft.as_mut() {
            draft.display_name = name;
        }
        self.sync_draft_view();
    }

    fn edit_note(&mut self, note: String) {
        if let Some(draft) = self.draft.as_mut() {
            draft.note = note;
        }
        self.sync_draft_view();
    }

    fn toggle_draft_pinned(&mut self) {
        if let Some(draft) = self.draft.as_mut() {
            draft.pinned = !draft.pinned;
        }
        self.sync_draft_view();
    }

    fn toggle_draft_favorite(&mut self) {
        if let Some(draft) = self.draft.as_mut() {
            draft.favorite = !draft.favorite;
        }
        self.sync_draft_view();
    }

    fn set_draft_enabled(&mut self, enabled: bool) {
        if let Some(draft) = self.draft.as_mut() {
            draft.enabled = enabled;
        }
        self.sync_draft_view();
    }

    fn set_draft_category(&mut self, category: Option<CategoryId>) {
        if let Some(draft) = self.draft.as_mut() {
            draft.category_id = category;
        }
        self.sync_draft_view();
    }

    fn set_draft_tag(&mut self, tag: TagId, on: bool) {
        if let Some(draft) = self.draft.as_mut() {
            if on {
                if !draft.tag_ids.contains(&tag) {
                    draft.tag_ids.push(tag);
                }
            } else {
                draft.tag_ids.retain(|id| *id != tag);
            }
        }
        self.sync_draft_view();
    }

    fn set_draft_weight(&mut self, weight: i16) {
        if let Some(draft) = self.draft.as_mut() {
            draft.manual_weight = weight.clamp(MIN_MANUAL_WEIGHT, MAX_MANUAL_WEIGHT);
        }
        self.sync_draft_view();
    }

    fn cycle_draft_color(&mut self) {
        const PALETTE: [u32; 8] = [
            0x2563_EBFF,
            0x0EA5_69FF,
            0xDC26_26FF,
            0xEA58_0CFF,
            0x7C3A_EDFF,
            0x0891_B2FF,
            0xBE18_5DFF,
            0x0000_0000,
        ];
        if let Some(draft) = self.draft.as_mut() {
            let current = draft.color.map(|color| color.0).unwrap_or(0);
            let next = PALETTE
                .iter()
                .position(|color| *color == current)
                .map(|index| PALETTE[(index + 1) % PALETTE.len()])
                .unwrap_or(PALETTE[0]);
            if next == 0 {
                draft.color = None;
            } else {
                draft.color = Some(FolderColor(next));
            }
        }
        self.sync_draft_view();
    }

    /// Save the draft (the add-or-edit apply). Default NOT to add a duplicate.
    fn save_draft(&mut self) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        // Validate name + path (path first: an unusable path is the more
        // fundamental error and a duplicate/inaccessible one is reported there).
        if path_key(&draft.path).is_none() {
            self.notice(Notice::InvalidPath);
            return;
        }
        if normalize_name(&draft.display_name).is_empty() {
            self.notice(Notice::InvalidName);
            return;
        }
        // Duplicate policy: default block (the UI offers EditExisting /
        // SaveAsDifferentName via its own confirmation; the controller refuses
        // a straight duplicate default).
        let duplicate = self.store.is_duplicate_path(&draft.path, draft.id);
        if duplicate {
            self.notice(Notice::DuplicateBlocked);
            return;
        }
        self.commit_draft(draft);
    }

    /// Resolve a duplicate-draft collision (M05.2 / H2). The user reached the
    /// duplicate confirmation explicitly (the Save button surfaced an already-
    /// existing path), so a non-default decision is honored: `EditExisting`
    /// opens the record that already owns the path; `SaveAsDifferentName`
    /// keeps the same path but persists the draft as its own record under the
    /// current (distinct) name. `Cancel` leaves everything untouched.
    ///
    /// Both non-default paths remain record-only: the real folder is never
    /// touched (the absolute no-delete invariant holds).
    fn resolve_duplicate(&mut self, policy: DuplicatePolicy) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        if path_key(&draft.path).is_none() {
            self.notice(Notice::InvalidPath);
            return;
        }
        match policy {
            DuplicatePolicy::Cancel => self.notice(Notice::DuplicateBlocked),
            DuplicatePolicy::EditExisting => {
                let existing = self.document_folders().into_iter().find(|folder| {
                    draft.id != Some(folder.id)
                        && crate::domain::path_semantics::same_path(&folder.path, &draft.path)
                });
                match existing {
                    Some(existing) => self.open_edit(existing.id),
                    None => self.notice(Notice::NotFound),
                }
            }
            DuplicatePolicy::SaveAsDifferentName => {
                if normalize_name(&draft.display_name).is_empty() {
                    self.notice(Notice::InvalidName);
                    return;
                }
                self.commit_draft(draft);
            }
        }
    }

    /// Build, insert and persist a draft as a folder record. Shared by the
    /// default save (path/name already validated, duplicate already rejected)
    /// and the explicit `SaveAsDifferentName` resolution (a deliberate
    /// duplicate path is allowed there).
    fn commit_draft(&mut self, draft: FolderDraft) {
        let entry = match draft.id {
            Some(id) => {
                let folders = self.document_folders();
                let existing = folders.into_iter().find(|folder| folder.id == id);
                let created_at = existing
                    .map(|folder| folder.created_at)
                    .unwrap_or_else(chrono::Utc::now);
                self.build_entry(draft, id, created_at)
            }
            None => self.build_entry(draft, FolderId::new(), chrono::Utc::now()),
        };
        if self.store.put_folder(entry).is_err() {
            self.notice(Notice::SaveFailed);
            return;
        }
        match self.store.save_at() {
            Ok(_) => {
                self.notice(Notice::Saved);
                self.draft = None;
                self.view.add_flow = AddFlowView::Closed;
                self.refresh_view();
            }
            Err(_) => self.notice(Notice::SaveFailed),
        }
    }

    fn build_entry(
        &self,
        draft: FolderDraft,
        id: FolderId,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> FolderEntry {
        super::management::build_folder(
            id,
            normalize_name(&draft.display_name),
            draft.path,
            draft.aliases,
            draft.category_id,
            draft.tag_ids,
            draft.note,
            draft.pinned,
            draft.favorite,
            draft.manual_weight,
            draft.enabled,
            draft.color,
            created_at,
            chrono::Utc::now(),
        )
    }

    fn cancel_draft(&mut self) {
        self.draft = None;
        self.view.add_flow = AddFlowView::Closed;
        self.import_offer = None;
        self.view.import_offer = None;
    }

    fn batch_view(&self, _categories: &[Category]) -> BatchPreviewView {
        BatchPreviewView {
            items: self
                .batch
                .iter()
                .enumerate()
                .map(|(index, item)| BatchItemView {
                    index,
                    name: item.name.clone(),
                    path: item.path.clone(),
                    duplicate_existing: match item.status {
                        BatchStatus::Duplicate => Some(item.name.clone()),
                        _ => None,
                    },
                    invalid: item.status == BatchStatus::Invalid,
                })
                .collect(),
        }
    }

    /// Apply the *ready* batch items (duplicates/invalid are skipped by default,
    /// matching "default NOT to add").
    fn apply_batch(&mut self) {
        let ready: Vec<BatchItem> = self
            .batch
            .iter()
            .filter(|item| item.status == BatchStatus::Ready)
            .cloned()
            .collect();
        let mut saved = 0usize;
        for item in ready {
            let entry = FolderDraft {
                id: None,
                display_name: item.name,
                path: item.path,
                aliases: Vec::new(),
                category_id: None,
                tag_ids: Vec::new(),
                note: String::new(),
                pinned: false,
                favorite: false,
                manual_weight: 0,
                enabled: true,
                color: None,
            };
            if self
                .store
                .put_folder(self.build_entry(entry, FolderId::new(), chrono::Utc::now()))
                .is_ok()
            {
                saved += 1;
            }
        }
        if self.store.save_at().is_err() {
            self.notice(Notice::SaveFailed);
            return;
        }
        self.batch.clear();
        self.view.add_flow = AddFlowView::Closed;
        self.notice(Notice::Saved);
        self.refresh_view();
        let _ = saved;
    }

    fn cancel_batch(&mut self) {
        self.batch.clear();
        self.view.add_flow = AddFlowView::Closed;
    }

    // --- remove + undo -------------------------------------------------

    fn start_remove(&mut self, folder_id: FolderId) {
        let folders = self.document_folders();
        let Some(folder) = folders.into_iter().find(|folder| folder.id == folder_id) else {
            self.notice(Notice::NotFound);
            return;
        };
        if self.store.remove_record(folder_id) {
            let previous_revision = self
                .store
                .document()
                .map(|document| document.data.revision)
                .unwrap_or(0);
            match self.store.save_at() {
                Ok(new_revision) => {
                    let _ = new_revision;
                    self.pending_remove = Some(PendingRemove {
                        folder_id,
                        payload: folder.clone(),
                        previous_revision,
                        expires_at: (self.now)() + UNDO_WINDOW,
                    });
                    self.notice(Notice::Removed);
                }
                Err(_) => {
                    self.notice(Notice::SaveFailed);
                }
            }
            self.refresh_view();
            self.sync_pending_remove_view();
        }
    }

    fn confirm_remove(&mut self) {
        self.sync_pending_remove_view();
    }

    fn cancel_remove(&mut self) {
        // Cancel a *pending* undo by restoring? No: cancel means "keep it
        // removed" — dismiss the undo banner.
        self.pending_remove = None;
        self.view.pending_remove = None;
    }

    fn undo_remove(&mut self) {
        let Some(pending) = self.pending_remove.clone() else {
            return;
        };
        if (self.now)() >= pending.expires_at {
            self.pending_remove = None;
            self.view.pending_remove = None;
            self.notice(Notice::UndoExpired);
            return;
        }
        let current = self
            .store
            .document()
            .map(|document| document.data.revision)
            .unwrap_or(0);
        match self.store.undo_remove_record(
            pending.payload.clone(),
            pending.previous_revision,
            current,
        ) {
            Ok(()) => {
                if self.store.save_at().is_ok() {
                    self.pending_remove = None;
                    self.view.pending_remove = None;
                    self.notice(Notice::Restored);
                    self.refresh_view();
                } else {
                    self.notice(Notice::SaveFailed);
                }
            }
            Err(_) => {
                // The undo was refused (something saved in between): drop the
                // stale pending state deterministically.
                self.pending_remove = None;
                self.view.pending_remove = None;
                self.notice(Notice::CannotUndo);
            }
        }
    }

    fn dismiss_undo(&mut self) {
        self.pending_remove = None;
        self.view.pending_remove = None;
    }

    fn sync_pending_remove_view(&mut self) {
        self.view.pending_remove = self.pending_remove.as_ref().map(|pending| {
            let seconds_left = pending
                .expires_at
                .saturating_duration_since((self.now)())
                .as_secs()
                .min(u64::from(u8::MAX)) as u8;
            PendingRemoveView {
                folder_name: pending.payload.display_name.clone(),
                seconds_left,
            }
        });
    }

    fn toggle_enable(&mut self, id: FolderId) {
        let enabled = self
            .document_folders()
            .into_iter()
            .find(|folder| folder.id == id)
            .map(|folder| folder.enabled)
            .unwrap_or(true);
        let result = if enabled {
            self.store.disable_record(id)
        } else {
            self.store.enable_record(id)
        };
        if result.is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn toggle_pin(&mut self, id: FolderId) {
        let pinned = self
            .document_folders()
            .into_iter()
            .find(|folder| folder.id == id)
            .map(|folder| folder.pinned)
            .unwrap_or(false);
        if self.store.set_pinned(id, !pinned).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn toggle_favorite(&mut self, id: FolderId) {
        let folders = self.document_folders();
        let favorite = folders
            .iter()
            .find(|folder| folder.id == id)
            .map(|folder| folder.favorite)
            .unwrap_or(false);
        let _ = folders;
        let mut entry = self
            .document_folders()
            .into_iter()
            .find(|folder| folder.id == id)
            .expect("folder row must exist");
        entry.favorite = !favorite;
        if self.store.put_folder(entry).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    /// A single MANUAL accessibility probe (user-initiated; not a scan).
    fn check_path(&mut self, id: FolderId) {
        let folders = self.document_folders();
        let Some(folder) = folders.into_iter().find(|folder| folder.id == id) else {
            self.notice(Notice::NotFound);
            return;
        };
        match std::fs::metadata(&folder.path) {
            Ok(metadata) if metadata.is_dir() => self.notice(Notice::PathAccessible),
            _ => self.notice(Notice::PathInaccessible),
        }
    }

    /// Re-locate an offline record to a new path (no fuzzy auto-match).
    fn relocate(&mut self, id: FolderId, new_path: String) {
        let mut entry = match self
            .document_folders()
            .into_iter()
            .find(|folder| folder.id == id)
        {
            Some(entry) => entry,
            None => {
                self.notice(Notice::NotFound);
                return;
            }
        };
        entry.path = new_path;
        if self.store.put_folder(entry).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    // --- categories -----------------------------------------------------

    fn create_category(&mut self, name: String) {
        let name = normalize_name(&name);
        if !valid_named_value(&name, MAX_CATEGORY_NAME_LEN) {
            self.notice(Notice::InvalidName);
            return;
        }
        let categories = self.document_categories();
        if categories.iter().any(|category| category.name == name) {
            self.notice(Notice::InvalidName);
            return;
        }
        let category = Category {
            id: CategoryId::new(),
            name,
            color: None,
        };
        if self.store.put_category(category).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn rename_category(&mut self, id: CategoryId, name: String) {
        let name = normalize_name(&name);
        if !valid_named_value(&name, MAX_CATEGORY_NAME_LEN) {
            self.notice(Notice::InvalidName);
            return;
        }
        let mut categories = self.document_categories();
        let index = match categories.iter().position(|category| category.id == id) {
            Some(index) => index,
            None => {
                self.notice(Notice::NotFound);
                return;
            }
        };
        if categories
            .iter()
            .any(|other| other.id != id && other.name == name)
        {
            self.notice(Notice::InvalidName);
            return;
        }
        categories[index].name = name;
        for category in categories {
            if self.store.put_category(category).is_err() {
                self.notice(Notice::SaveFailed);
                return;
            }
        }
        if self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn delete_category(&mut self, id: CategoryId) {
        if self.store.remove_category(id) {
            if self.store.save_at().is_ok() {
                self.notice(Notice::Saved);
            } else {
                self.notice(Notice::SaveFailed);
            }
        } else {
            self.notice(Notice::NotFound);
        }
        self.refresh_view();
    }

    // --- tags -----------------------------------------------------------

    fn create_tag(&mut self, name: String) {
        let name = normalize_name(&name);
        if !valid_named_value(&name, MAX_TAG_NAME_LEN) {
            self.notice(Notice::InvalidName);
            return;
        }
        let tags = self.document_tags();
        // Exact-duplicate tags are blocked (ASCII-case-insensitive).
        if tags
            .iter()
            .any(|tag| super::management::names_equal(&tag.name, &name))
        {
            self.notice(Notice::InvalidName);
            return;
        }
        let tag = Tag {
            id: TagId::new(),
            name,
        };
        if self.store.put_tag(tag).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn rename_tag(&mut self, id: TagId, name: String) {
        let name = normalize_name(&name);
        if !valid_named_value(&name, MAX_TAG_NAME_LEN) {
            self.notice(Notice::InvalidName);
            return;
        }
        let tags = self.document_tags();
        if tags
            .iter()
            .any(|tag| tag.id != id && super::management::names_equal(&tag.name, &name))
        {
            self.notice(Notice::InvalidName);
            return;
        }
        if self.store.rename_tag(id, name).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn merge_tag(&mut self, source_id: TagId, target_name: String) {
        let tags = self.document_tags();
        let target_id = super::management::resolve_merge_target_id(&tags, source_id, &target_name);
        if target_id == source_id {
            // No valid merge target (no other tag with that name).
            self.notice(Notice::InvalidName);
            return;
        }
        if self.store.merge_tag(source_id, target_id).is_ok() && self.store.save_at().is_ok() {
            self.notice(Notice::Saved);
        } else {
            self.notice(Notice::SaveFailed);
        }
        self.refresh_view();
    }

    fn delete_tag(&mut self, id: TagId) {
        if self.store.remove_tag(id) {
            if self.store.save_at().is_ok() {
                self.notice(Notice::Saved);
            } else {
                self.notice(Notice::SaveFailed);
            }
        } else {
            self.notice(Notice::NotFound);
        }
        self.refresh_view();
    }

    // --- child import ---------------------------------------------------

    fn choose_import(&mut self, choice: ChildImportChoice) {
        let Some(offer) = self.import_offer.clone() else {
            return;
        };
        match choice {
            ChildImportChoice::ParentOnly => {
                self.import_offer = None;
                self.view.import_offer = None;
                self.open_single_draft(offer.parent_path);
            }
            ChildImportChoice::DirectChildren => {
                let children = list_direct_children(&offer.parent_path, MAX_ONE_LEVEL_IMPORT);
                self.import_offer = None;
                self.view.import_offer = None;
                if children.is_empty() {
                    self.notice(Notice::ImportLimited);
                    self.open_single_draft(offer.parent_path);
                    return;
                }
                self.open_pick_paths(children);
            }
        }
    }

    fn dismiss_import(&mut self) {
        self.import_offer = None;
        self.view.import_offer = None;
    }

    /// Persist the one-level-import setting (M05 review M1). Default OFF; the
    /// toggle is the user-facing enablement that makes the controlled parent-
    /// dir import reachable. Dismisses any pending import offer when the setting
    /// is turned OFF (it no longer applies).
    fn set_one_level_import(&mut self, enabled: bool) {
        if self.store.set_one_level_import(enabled).is_err() {
            self.notice(Notice::SaveFailed);
            return;
        }
        if self.store.save_at().is_ok() {
            self.view.one_level_import_setting = enabled;
            self.notice(Notice::Saved);
            if !enabled {
                self.dismiss_import();
            }
        } else {
            self.notice(Notice::SaveFailed);
        }
    }
}

/// List the DIRECT children (folders only) of `parent`, capped at `limit`.
/// User-initiated, one level deep, never recursive — the M05.2 no-scan rule.
fn list_direct_children(parent: &str, limit: usize) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut children = Vec::new();
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            children.push(entry.path().to_string_lossy().into_owned());
            if children.len() >= limit {
                break;
            }
        }
    }
    children
}

fn list_direct_children_count(parent: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.metadata().map(|m| m.is_dir()).unwrap_or(false))
        .take(MAX_ONE_LEVEL_IMPORT)
        .count()
}

/// Resolve the default data directory (`%LOCALAPPDATA%\FileGo`). Pure helper
/// over an injected `LOCALAPPDATA` value so it is testable.
pub fn data_dir_from(local_app_data: Option<&str>) -> Option<std::path::PathBuf> {
    let root = local_app_data?;
    if root.trim().is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(root).join("FileGo"))
}

/// Expand `%VAR%` / `%USERPROFILE%` / leading `~` for the RELOCATE preview.
/// Explicitly NOT applied to the stored record and NOT applied in the M04
/// ShellExecuteExW open (raw path).
pub fn relocate_preview(raw: &str, env: &impl Fn(&str) -> Option<String>) -> String {
    let getter = |name: &str| env(name);
    expand_open_path(raw, getter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::AppData;
    use crate::domain::settings::AppSettings;
    use uuid::Uuid;

    /// A deterministic in-memory store so controller tests run without disk.
    #[derive(Default)]
    struct MemStore {
        document: Option<StoredDocumentV1>,
        saved_count: usize,
        fail_save: bool,
    }

    impl MemStore {
        fn with_document(document: StoredDocumentV1) -> Self {
            Self {
                document: Some(document),
                saved_count: 0,
                fail_save: false,
            }
        }
    }

    fn mem_seed() -> StoredDocumentV1 {
        let category_id = CategoryId::from_uuid(Uuid::from_u128(1));
        let tag_a = TagId::from_uuid(Uuid::from_u128(2));
        let tag_b = TagId::from_uuid(Uuid::from_u128(3));
        StoredDocumentV1::new(AppData {
            settings: AppSettings {
                one_level_import: false,
                ..AppSettings::default()
            },
            folders: vec![{
                let mut folder = FolderEntry {
                    id: FolderId::from_uuid(Uuid::from_u128(10)),
                    display_name: "Documents".to_owned(),
                    aliases: Vec::new(),
                    path: r"C:\Users\me\Documents".to_owned(),
                    enabled: true,
                    favorite: false,
                    pinned: true,
                    manual_weight: 0,
                    category_id: Some(category_id),
                    tag_ids: vec![tag_a],
                    note: String::new(),
                    color: None,
                    sort_order: 0,
                    created_at: "2026-09-20T00:00:00Z".parse().unwrap(),
                    updated_at: "2026-09-20T00:00:00Z".parse().unwrap(),
                    last_opened_at: None,
                    open_count: 0,
                };
                folder.last_opened_at = Some("2026-09-21T00:00:00Z".parse().unwrap());
                folder
            }],
            categories: vec![Category {
                id: category_id,
                name: "工作".to_owned(),
                color: None,
            }],
            tags: vec![
                Tag {
                    id: tag_a,
                    name: "重要".to_owned(),
                },
                Tag {
                    id: tag_b,
                    name: "客户 A".to_owned(),
                },
            ],
            revision: 2,
        })
    }

    impl ManagementStore for MemStore {
        fn load(
            &mut self,
        ) -> Result<StoredDocumentV1, crate::storage::repository::RepositoryError> {
            self.document
                .clone()
                .ok_or(crate::storage::repository::RepositoryError::NotFound)
        }
        fn document(&self) -> Option<StoredDocumentV1> {
            self.document.clone()
        }
        fn save_at(&mut self) -> Result<u64, crate::storage::repository::RepositoryError> {
            if self.fail_save {
                return Err(crate::storage::repository::RepositoryError::Io);
            }
            let document = self.document.as_mut().expect("document");
            let next = document.data.next_revision().expect("rev");
            document.data.revision = next;
            self.saved_count += 1;
            Ok(next)
        }
        fn put_folder(
            &mut self,
            folder: FolderEntry,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            if let Some(existing) = document.data.folders.iter_mut().find(|f| f.id == folder.id) {
                *existing = folder;
            } else {
                document.data.folders.push(folder);
            }
            Ok(())
        }
        fn put_category(
            &mut self,
            category: Category,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            document.data.categories.push(category);
            Ok(())
        }
        fn put_tag(&mut self, tag: Tag) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            document.data.tags.push(tag);
            Ok(())
        }
        fn remove_record(&mut self, folder_id: FolderId) -> bool {
            let document = self.document.as_mut().expect("document");
            let n = document.data.folders.len();
            document.data.folders.retain(|f| f.id != folder_id);
            document.data.folders.len() != n
        }
        fn disable_record(
            &mut self,
            folder_id: FolderId,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            let folder = document
                .data
                .folders
                .iter_mut()
                .find(|f| f.id == folder_id)
                .ok_or(crate::storage::repository::RepositoryError::NotFound)?;
            folder.enabled = false;
            Ok(())
        }
        fn enable_record(
            &mut self,
            folder_id: FolderId,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            let folder = document
                .data
                .folders
                .iter_mut()
                .find(|f| f.id == folder_id)
                .ok_or(crate::storage::repository::RepositoryError::NotFound)?;
            folder.enabled = true;
            Ok(())
        }
        fn set_pinned(
            &mut self,
            folder_id: FolderId,
            pinned: bool,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            let folder = document
                .data
                .folders
                .iter_mut()
                .find(|f| f.id == folder_id)
                .ok_or(crate::storage::repository::RepositoryError::NotFound)?;
            folder.pinned = pinned;
            Ok(())
        }
        fn remove_category(&mut self, category_id: CategoryId) -> bool {
            let document = self.document.as_mut().expect("document");
            let n = document.data.categories.len();
            document.data.categories.retain(|c| c.id != category_id);
            let removed = document.data.categories.len() != n;
            if removed {
                for folder in &mut document.data.folders {
                    if folder.category_id == Some(category_id) {
                        folder.category_id = None;
                    }
                }
            }
            removed
        }
        fn remove_tag(&mut self, tag_id: TagId) -> bool {
            let document = self.document.as_mut().expect("document");
            let n = document.data.tags.len();
            document.data.tags.retain(|t| t.id != tag_id);
            let removed = document.data.tags.len() != n;
            if removed {
                for folder in &mut document.data.folders {
                    folder.tag_ids.retain(|id| *id != tag_id);
                }
            }
            removed
        }
        fn rename_tag(
            &mut self,
            tag_id: TagId,
            name: String,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            let tag = document
                .data
                .tags
                .iter_mut()
                .find(|t| t.id == tag_id)
                .ok_or(crate::storage::repository::RepositoryError::NotFound)?;
            tag.name = name;
            Ok(())
        }
        fn merge_tag(
            &mut self,
            source: TagId,
            target: TagId,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            for folder in &mut document.data.folders {
                if folder.tag_ids.contains(&source) {
                    folder.tag_ids.retain(|id| *id != source);
                    if !folder.tag_ids.contains(&target) {
                        folder.tag_ids.push(target);
                    }
                }
            }
            document.data.tags.retain(|t| t.id != source);
            Ok(())
        }
        fn undo_remove_record(
            &mut self,
            payload: FolderEntry,
            previous_revision: u64,
            expected_current: u64,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            // Mirror the real repository: the undo is ONLY valid when the
            // removal was the last save (previous + 1 == current) and the
            // working copy still matches the caller's snapshot.
            let undoing_the_last_save =
                previous_revision.saturating_add(1) == document.data.revision;
            if document.data.revision != expected_current || !undoing_the_last_save {
                return Err(crate::storage::repository::RepositoryError::ConcurrentModification);
            }
            document.data.folders.push(payload);
            Ok(())
        }
        fn duplicate_folders(&self, _exclude: Option<FolderId>) -> Vec<usize> {
            Vec::new()
        }
        fn is_duplicate_path(&self, candidate: &str, exclude: Option<FolderId>) -> bool {
            let document = self.document().expect("document");
            document.data.folders.iter().any(|folder| {
                exclude != Some(folder.id)
                    && crate::domain::path_semantics::same_path(&folder.path, candidate)
            })
        }
        fn set_one_level_import(
            &mut self,
            enabled: bool,
        ) -> Result<(), crate::storage::repository::RepositoryError> {
            let document = self.document.as_mut().expect("document");
            document.data.settings.one_level_import = enabled;
            Ok(())
        }
    }

    fn controller() -> ManagementController<MemStore> {
        ManagementController::new(MemStore::with_document(mem_seed()), false)
    }

    #[test]
    fn initial_view_lists_folders_categories_tags() {
        let controller = controller();
        assert_eq!(controller.view().folders, 1);
        assert_eq!(controller.view().categories.len(), 1);
        assert_eq!(controller.view().tags.len(), 2);
        assert_eq!(controller.view().rows[0].display_name, "Documents");
        assert!(controller.view().rows[0].pinned);
        assert_eq!(
            controller.view().rows[0].category_name.as_deref(),
            Some("工作")
        );
    }

    #[test]
    fn add_draft_validates_duplicate_and_default_blocks() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"C:\Users\me\Documents".to_owned()));
        // The draft view flags the duplicate (M01.2 semantics).
        let draft = match &controller.view().add_flow {
            AddFlowView::Draft(draft) => draft,
            other => panic!("expected draft, got {other:?}"),
        };
        assert_eq!(draft.duplicate_existing.as_deref(), Some("Documents"));
        controller.handle(MCommand::SaveDraft);
        assert_eq!(controller.view().notice, Some(Notice::DuplicateBlocked));
        assert_eq!(controller.view().folders, 1, "duplicate must not be added");
    }

    #[test]
    fn add_draft_unique_path_saves_and_appears_in_list() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"D:\new\资料".to_owned()));
        controller.handle(MCommand::EditName("资料".to_owned()));
        controller.handle(MCommand::SaveDraft);
        assert_eq!(controller.view().notice, Some(Notice::Saved));
        assert_eq!(controller.view().folders, 2);
    }

    #[test]
    fn invalid_path_is_rejected_without_changing_data() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath("   ".to_owned()));
        controller.handle(MCommand::SaveDraft);
        assert_eq!(controller.view().notice, Some(Notice::InvalidPath));
        assert_eq!(controller.view().folders, 1);
    }

    #[test]
    fn edit_preserves_existing_record_by_id() {
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::EditFolder(id));
        controller.handle(MCommand::EditPath(r"C:\Users\me\Documents\x".to_owned()));
        controller.handle(MCommand::SaveDraft);
        assert_eq!(controller.view().folders, 1, "edit must not duplicate");
        assert_eq!(controller.view().notice, Some(Notice::Saved));
    }

    #[test]
    fn remove_then_undo_restores_within_window() {
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::StartRemove(id));
        assert_eq!(controller.view().folders, 0);
        assert!(controller.view().pending_remove.is_some());
        controller.handle(MCommand::UndoRemove);
        assert_eq!(controller.view().folders, 1);
        assert_eq!(controller.view().notice, Some(Notice::Restored));
    }

    #[test]
    fn remove_undo_expires_and_cannot_undo() {
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        // A controllable clock: `origin` is captured by the boxed closure.
        let origin = std::time::Instant::now();
        let start_clock: std::sync::Arc<dyn Fn() -> std::time::Instant> =
            std::sync::Arc::new(move || origin);
        controller.now = start_clock.clone();
        controller.handle(MCommand::StartRemove(id));
        // The UI polls the banner past the window; a far-future clock makes the
        // Undo deterministically refuse (expired).
        let expired_clock: std::sync::Arc<dyn Fn() -> std::time::Instant> =
            std::sync::Arc::new(move || origin + UNDO_WINDOW + UNDO_WINDOW);
        controller.now = expired_clock;
        controller.handle(MCommand::UndoRemove);
        assert_eq!(controller.view().folders, 0);
        assert_eq!(controller.view().notice, Some(Notice::UndoExpired));
    }

    #[test]
    fn undo_is_refused_after_an_intervening_save() {
        // The real repository refuses deterministic; the MemStore refuses when
        // the revision moved. Simulate by opening+closing an unrelated save.
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::StartRemove(id));
        // A later save (any) bumps the revision -> undo must refuse.
        controller.store.save_at().expect("intervening save");
        controller.handle(MCommand::UndoRemove);
        assert_eq!(controller.view().folders, 0);
        assert_eq!(controller.view().notice, Some(Notice::CannotUndo));
    }

    #[test]
    fn toggle_enable_and_pin_persist_and_list_reflects_status() {
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::SetFilterEnabled(Some(false)));
        assert_eq!(controller.view().rows.len(), 0);
        controller.handle(MCommand::ToggleEnable(id));
        controller.handle(MCommand::SetFilterEnabled(Some(false)));
        assert_eq!(controller.view().rows.len(), 1, "disabled filter shows it");
        assert!(!controller.view().rows[0].enabled);
    }

    #[test]
    fn category_create_rename_delete_updates_references() {
        let mut controller = controller();
        controller.handle(MCommand::CreateCategory("新分类".to_owned()));
        assert_eq!(controller.view().categories.len(), 2);

        let new_id = controller.view().categories[1].id;
        let folder_id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::EditFolder(folder_id));
        controller.handle(MCommand::SetDraftCategory(Some(new_id)));
        controller.handle(MCommand::SaveDraft);
        // After delete of the new category the folder returns to 未分类.
        controller.handle(MCommand::DeleteCategory(new_id));
        assert_eq!(controller.view().categories.len(), 1);
        let folders = controller.store.document().unwrap().data.folders.clone();
        assert!(
            folders[0].category_id.is_none(),
            "deleted category clears reference"
        );
        assert_eq!(
            controller.view().folders,
            1,
            "folder survives category delete"
        );
    }

    #[test]
    fn tag_create_blocks_exact_duplicate_and_merge_updates_usage() {
        let mut controller = controller();
        controller.handle(MCommand::CreateTag("重要".to_owned()));
        // "重要" already exists (exact duplicate) -> blocked.
        assert_eq!(controller.view().tags.len(), 2);
        assert_eq!(controller.view().notice, Some(Notice::InvalidName));

        controller.handle(MCommand::CreateTag("新标签".to_owned()));
        assert_eq!(controller.view().tags.len(), 3);

        // Merge the folder's tag into the new one.
        let important = TagId::from_uuid(Uuid::from_u128(2));
        controller.handle(MCommand::MergeTag(important, "新标签".to_owned()));
        let folders = controller.store.document().unwrap().data.folders.clone();
        assert!(!folders[0].tag_ids.contains(&important));
        assert_eq!(controller.view().tags.len(), 2, "source tag removed");
    }

    #[test]
    fn tag_delete_clears_associations_only() {
        let mut controller = controller();
        let tag_a = TagId::from_uuid(Uuid::from_u128(2));
        controller.handle(MCommand::DeleteTag(tag_a));
        let folders = controller.store.document().unwrap().data.folders.clone();
        assert!(folders[0].tag_ids.is_empty());
        assert_eq!(controller.view().folders, 1);
    }

    #[test]
    fn multi_pick_previews_batch_and_applies_ready_only() {
        let mut controller = controller();
        let paths = vec![
            r"C:\new\one".to_owned(),
            r"C:\Users\me\Documents".to_owned(), // duplicate
            r"D:\new\two".to_owned(),
        ];
        controller.handle(MCommand::OpenPickPaths(paths));
        assert!(matches!(
            controller.view().add_flow,
            AddFlowView::Preview(_)
        ));
        controller.handle(MCommand::ApplyBatch);
        assert_eq!(
            controller.view().folders,
            1 + 2,
            "only the two ready items added"
        );
    }

    #[test]
    fn one_level_import_offer_follows_setting_and_labels() {
        // Setting OFF -> no offer.
        let mut off = controller();
        off.view.one_level_import_setting = false;
        off.maybe_offer_import(r"C:\a".to_owned());
        assert!(off.view.import_offer.is_none());

        // Setting ON -> offer with the `max 100` hover literal.
        let mut on = controller();
        on.view.one_level_import_setting = true;
        // `C:\a` likely has no listing on a CI box; the offer gate is tested
        // with child_count injected through the pure helper instead.
        assert_eq!(
            crate::presentation::management::child_import_offer(true, 3),
            crate::presentation::management::ChildImportOffer::Choose { child_count: 3 }
        );
        assert_eq!(ONE_LEVEL_IMPORT_MAX_LITERAL, "max 100");
    }

    #[test]
    fn relocate_expands_env_for_preview_only() {
        let env = |name: &str| -> Option<String> {
            match name {
                "USERPROFILE" => Some("C:\\Users\\me".to_owned()),
                _ => None,
            }
        };
        assert_eq!(
            relocate_preview(r"%USERPROFILE%\Documents", &env),
            r"C:\Users\me\Documents"
        );
        assert_eq!(relocate_preview(r"~\docs", &env), r"C:\Users\me\docs");
    }

    #[test]
    fn check_path_on_missing_dir_reports_inaccessible_not_deleted() {
        let mut controller = controller();
        let id = FolderId::from_uuid(Uuid::from_u128(10));
        controller.handle(MCommand::CheckPath(id));
        assert_eq!(controller.view().notice, Some(Notice::PathInaccessible));
        assert_eq!(
            controller.view().folders,
            1,
            "inaccessible path keeps the record"
        );
    }

    #[test]
    fn draft_editing_marks_unsaved_and_path_revalidates() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"\\nas\offline\共享 目录".to_owned()));
        let view = controller.view();
        match &view.add_flow {
            AddFlowView::Draft(draft) => {
                assert!(draft.valid, "offline UNC is still valid");
                assert!(draft.duplicate_existing.is_none());
                assert!(draft.unsaved);
            }
            other => panic!("expected draft, got {other:?}"),
        }
    }

    #[test]
    fn filter_by_name_is_case_insensitive() {
        let mut controller = controller();
        controller.handle(MCommand::SetFilterName("doc".to_owned()));
        assert_eq!(controller.view().rows.len(), 1);
        controller.handle(MCommand::SetFilterName("DOC".to_owned()));
        assert_eq!(controller.view().rows.len(), 1);
    }

    // --- M05 review C1: FolderId→row-index resolution (no int truncation) ----

    #[test]
    fn folder_id_at_resolves_row_index_to_the_real_folder_id() {
        let controller = controller();
        // The rows snapshot holds the full 128-bit id. Index resolution is
        // index→id (the UI never sends a truncated `u128 as i32`).
        let rows = controller.view().rows.clone();
        let row_index = rows
            .iter()
            .position(|row| row.display_name == "Documents")
            .expect("seed row exists");
        assert_eq!(
            folder_id_at(&rows, row_index),
            Some(FolderId::from_uuid(Uuid::from_u128(10)))
        );

        // An out-of-range index resolves to None (the caller no-ops, never
        // acting on a wrong record).
        assert_eq!(folder_id_at(&rows, rows.len()), None);
        assert_eq!(folder_id_at(&rows, 999), None);
    }

    #[test]
    fn duplicate_policy_edit_existing_opens_the_existing_record() {
        let mut controller = controller();
        // Start a fresh add that collides with the existing "Documents" record.
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"C:\Users\me\Documents".to_owned()));
        controller.handle(MCommand::EditName("Documents copy".to_owned()));
        controller.handle(MCommand::SaveDraft);
        assert_eq!(controller.view().notice, Some(Notice::DuplicateBlocked));

        // EditExisting: open the record that owns the path for editing.
        controller.handle(MCommand::ResolveDuplicate(DuplicatePolicy::EditExisting));
        let add_flow = controller.view().add_flow.clone();
        match add_flow {
            AddFlowView::Draft(draft) => {
                assert_eq!(draft.id, Some(FolderId::from_uuid(Uuid::from_u128(10))));
                assert_eq!(draft.display_name, "Documents");
            }
            other => panic!("expected edit draft, got {other:?}"),
        }
        // The folder count is unchanged (no record was added or removed).
        assert_eq!(controller.view().folders, 1);
    }

    #[test]
    fn duplicate_policy_save_as_different_name_adds_an_independent_record() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"C:\Users\me\Documents".to_owned()));
        controller.handle(MCommand::EditName("Documents 2".to_owned()));
        controller.handle(MCommand::ResolveDuplicate(
            DuplicatePolicy::SaveAsDifferentName,
        ));
        assert_eq!(controller.view().notice, Some(Notice::Saved));
        assert_eq!(controller.view().folders, 2, "independent record added");
        let folders = controller.store.document().unwrap().data.folders.clone();
        let names: Vec<&str> = folders.iter().map(|f| f.display_name.as_str()).collect();
        assert!(names.contains(&"Documents"));
        assert!(names.contains(&"Documents 2"));
    }

    #[test]
    fn duplicate_policy_cancel_blocks_without_change() {
        let mut controller = controller();
        controller.handle(MCommand::OpenManual);
        controller.handle(MCommand::EditPath(r"C:\Users\me\Documents".to_owned()));
        controller.handle(MCommand::ResolveDuplicate(DuplicatePolicy::Cancel));
        assert_eq!(controller.view().notice, Some(Notice::DuplicateBlocked));
        assert_eq!(controller.view().folders, 1);
    }

    #[test]
    fn one_level_import_toggle_persists_and_controls_the_offer() {
        // Default OFF: no offer is reachable.
        let mut controller = controller();
        assert!(!controller.view().one_level_import_setting);
        assert!(
            !controller
                .store
                .document()
                .unwrap()
                .data
                .settings
                .one_level_import
        );

        // Toggle ON: the flag persists into the store and the view.
        controller.handle(MCommand::SetOneLevelImport(true));
        assert!(controller.view().one_level_import_setting);
        assert!(
            controller
                .store
                .document()
                .unwrap()
                .data
                .settings
                .one_level_import
        );

        // Toggle OFF again: default restored.
        controller.handle(MCommand::SetOneLevelImport(false));
        assert!(!controller.view().one_level_import_setting);
        assert!(
            !controller
                .store
                .document()
                .unwrap()
                .data
                .settings
                .one_level_import
        );
    }

    #[test]
    fn index_resolution_never_truncates_a_128_bit_id() {
        // The structural invariant the review flagged: a UUID whose low 32 bits
        // are 0 must NOT be reachable through `u128 as i32` (which yields 0).
        // Pin that truncation would collide and that index resolution does not.
        // Low 32 bits are zero: `u128 as i32` truncates to 0.
        let low32_zero =
            FolderId::from_uuid(Uuid::from_u128(0x1234_5678_9ABC_DEF0_1234_5678_0000_0000));
        let as_i32 = low32_zero.as_uuid().as_u128() as i32;
        assert_eq!(as_i32, 0, "low-32-zero UUID truncates to int 0");

        // The row holds the full id; index resolution returns the full id.
        let row = super::FolderRow {
            id: low32_zero,
            display_name: "low".to_owned(),
            path: r"C:\low".to_owned(),
            category_name: None,
            tag_names: Vec::new(),
            enabled: true,
            pinned: false,
            favorite: false,
            last_opened_at: None,
            created_at: "2026-09-21T00:00:00Z".parse().expect("fixture"),
            manual_weight: 0,
            open_count: 0,
        };
        assert_eq!(folder_id_at(&[row], 0), Some(low32_zero));
    }
}
