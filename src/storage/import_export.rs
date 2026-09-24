//! Pure import/export planning for the versioned JSON document (M06.5).
//!
//! # Export
//!
//! `export_bytes` reuses `codec::encode` on the current document, so an export
//! is byte-compatible with the on-disk schema-v1 document and round-trips
//! through `codec::decode`.
//!
//! # Import contract (all-or-nothing)
//!
//! `parse_import` decodes AND fully validates the whole file before anything
//! else is attempted: an invalid JSON, a future/unknown schema version, or a
//! document that fails `AppData::validate_and_normalize` yields an error and
//! **no mutation happens** (the caller never calls `apply_import` on a file
//! that failed to parse). This is also enforced inside `apply_import`: the
//! resulting document is validated again and a failure aborts before any write.
//!
//! The import scope is data (folders + categories + tags); the current app
//! settings and revision are preserved unless restored from a backup. Real
//! folders on disk are never touched — every operation is record-level.
//!
//! # Import modes (deterministic diff by M01.2 path semantics + id)
//!
//! - `Overwrite`: the imported record wins on a path/id collision (fields
//!   replaced, `created_at` preserved); a record with no collision is added.
//!   A record whose id matches one current record but whose path collides with
//!   a DIFFERENT current record is classified `Conflict` and the whole apply is
//!   refused (`DuplicatePath`, all-or-nothing) — overwriting by id would leave
//!   two records with the same path (M01.2 path uniqueness). M06 review M3.
//! - `Merge`: add only records whose path is not already present; a colliding
//!   path keeps the current record (skipped).
//! - `SkipDuplicates`: like `Merge`, and a path that appears more than once
//!   within the import file itself is added once then skipped.
//!
//! Categories/tags from the file are always merged in (upsert by id), so every
//! imported folder's category/tag references resolve. `conflicts` guards the
//! pathological cases where the union still would not resolve a reference or
//! would duplicate a path; for a validated file it is empty. `apply_import`
//! re-validates the union (references + path uniqueness) and rejects
//! (`UnresolvedReference` / `DuplicatePath`) before returning, so neither can
//! ever reach the persisted document.

use std::fmt;

use crate::domain::{document::AppData, folder::FolderEntry, path_semantics::same_path};

use super::{codec, codec::StorageError, schema::StoredDocumentV1};

/// Why an import file cannot be used (anonymous, never a path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportErrorKind {
    /// Not valid JSON.
    InvalidJson,
    /// A future schema version we refuse to guess about.
    FutureSchema,
    /// An older schema version that would need migration (unsupported).
    MigrationNeeded,
    /// The document fails structural/reference validation.
    InvalidDocument,
    /// The merged result could not be validated (reference conflict).
    UnresolvedReference,
    /// M06 review M3: applying would leave two records with the same path (the
    /// M01.2 path-uniqueness invariant). All-or-nothing: refused, no mutation.
    DuplicatePath,
    /// The document could not be encoded (export path).
    EncodeFailed,
}

impl std::error::Error for ImportError {}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self.kind {
            ImportErrorKind::InvalidJson => "the file is not a valid FileGo document",
            ImportErrorKind::FutureSchema => {
                "the file uses a newer format this version cannot import"
            }
            ImportErrorKind::MigrationNeeded => {
                "the file uses an older format that needs migration (unsupported)"
            }
            ImportErrorKind::InvalidDocument => "the file does not form a valid FileGo document",
            ImportErrorKind::UnresolvedReference => "the import would leave unresolved references",
            ImportErrorKind::DuplicatePath => "the import would duplicate a folder path",
            ImportErrorKind::EncodeFailed => "the current data could not be encoded",
        };
        formatter.write_str(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportError {
    pub kind: ImportErrorKind,
}

impl ImportError {
    pub const fn new(kind: ImportErrorKind) -> Self {
        Self { kind }
    }
}

impl From<StorageError> for ImportError {
    fn from(error: StorageError) -> Self {
        let kind = match error.kind() {
            codec::StorageErrorKind::InvalidJson => ImportErrorKind::InvalidJson,
            codec::StorageErrorKind::UnsupportedFutureSchema => ImportErrorKind::FutureSchema,
            codec::StorageErrorKind::MigrationRequired => ImportErrorKind::MigrationNeeded,
            codec::StorageErrorKind::InvalidDocument | codec::StorageErrorKind::EncodeFailed => {
                ImportErrorKind::InvalidDocument
            }
        };
        ImportError::new(kind)
    }
}

/// Import resolution policy (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    Overwrite,
    Merge,
    SkipDuplicates,
}

impl ImportMode {
    /// Whether a colliding path yields an update (import wins) or is skipped.
    pub const fn overwrite_wins(self) -> bool {
        matches!(self, ImportMode::Overwrite)
    }
}

/// Per-record classification used by both the preview planner and the apply
/// pass (one source of truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Added,
    Updated,
    Skipped,
    Conflict,
}

impl ItemStatus {
    pub const fn label(self) -> &'static str {
        match self {
            ItemStatus::Added => "added",
            ItemStatus::Updated => "updated",
            ItemStatus::Skipped => "skipped",
            ItemStatus::Conflict => "conflict",
        }
    }
}

/// The pure diff preview: counts plus the display names in each bucket
/// (deterministic order = the imported file's order).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPlan {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub conflicts: Vec<String>,
}

impl ImportPlan {
    pub fn added_count(&self) -> usize {
        self.added.len()
    }
    pub fn updated_count(&self) -> usize {
        self.updated.len()
    }
    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }
    pub fn conflicts_count(&self) -> usize {
        self.conflicts.len()
    }
    pub fn total(&self) -> usize {
        self.added_count() + self.updated_count() + self.skipped_count() + self.conflicts_count()
    }
}

/// parse + validate the whole import file; **no** mutation happens here or
/// later when this fails.
pub fn parse_import(bytes: &[u8]) -> Result<StoredDocumentV1, ImportError> {
    Ok(codec::decode(bytes)?)
}

/// Encode the current document for export (same schema-v1 bytes as the live
/// file).
pub fn export_bytes(document: &StoredDocumentV1) -> Result<Vec<u8>, ImportError> {
    codec::encode(document).map_err(|_| ImportError::new(ImportErrorKind::EncodeFailed))
}

/// Classify one imported folder against the current document + the in-file
/// dedup set (paths already planned as Added in this very file).
///
/// Deterministic priority: (1) same id → `Updated` in Overwrite mode else
/// `Skipped` (unless the id-match would duplicate another record's path →
/// `Conflict`, see M06 review M3); (2) same path (M01.2) → `Updated` in
/// Overwrite mode else `Skipped`; (3) in-file duplicate path → `Skipped`;
/// (4) otherwise `Added`.
fn classify(
    folder: &FolderEntry,
    current: &AppData,
    seen_added_paths: &mut Vec<String>,
    mode: ImportMode,
) -> (ItemStatus, String) {
    let name = folder.display_name.clone();
    // 1. id-match: an import of the SAME record.
    let id_present = current
        .folders
        .iter()
        .any(|existing| existing.id == folder.id);
    if id_present {
        // M06 review M3: an id-match whose incoming path collides with a
        // DIFFERENT record (not the id-matched one) is ambiguous — overwriting
        // by id would leave two records with the same path. Classify as
        // `Conflict` (only in Overwrite mode, the mode that replaces by id) so
        // the preview shows it and the all-or-nothing apply refuses it.
        let id_matched_path_differs = current
            .folders
            .iter()
            .find(|existing| existing.id == folder.id)
            .is_some_and(|existing| !same_path(&existing.path, &folder.path));
        let collides_with_different_record = current
            .folders
            .iter()
            .any(|existing| existing.id != folder.id && same_path(&existing.path, &folder.path));
        if mode.overwrite_wins() && id_matched_path_differs && collides_with_different_record {
            return (ItemStatus::Conflict, name);
        }
        return (
            if mode.overwrite_wins() {
                ItemStatus::Updated
            } else {
                ItemStatus::Skipped
            },
            name,
        );
    }
    // 2. path collision (M01.2).
    let path_present = current
        .folders
        .iter()
        .any(|existing| !existing.id.eq(&folder.id) && same_path(&existing.path, &folder.path));
    if path_present {
        return (
            if mode.overwrite_wins() {
                ItemStatus::Updated
            } else {
                ItemStatus::Skipped
            },
            name,
        );
    }
    // 3. in-file duplicate path (skip-duplicates within the file).
    if seen_added_paths
        .iter()
        .any(|already| same_path(already, &folder.path))
    {
        (ItemStatus::Skipped, name)
    } else {
        seen_added_paths.push(folder.path.clone());
        (ItemStatus::Added, name)
    }
}

/// Pure preview: plan what applying `incoming` onto `current` would do. Does
/// not modify anything.
pub fn plan_import(current: &AppData, incoming: &AppData, mode: ImportMode) -> ImportPlan {
    let mut plan = ImportPlan::default();
    let mut seen_added_paths: Vec<String> = Vec::new();
    for folder in &incoming.folders {
        let (status, name) = classify(folder, current, &mut seen_added_paths, mode);
        match status {
            ItemStatus::Added => plan.added.push(name),
            ItemStatus::Updated => plan.updated.push(name),
            ItemStatus::Skipped => plan.skipped.push(name),
            ItemStatus::Conflict => plan.conflicts.push(name),
        }
    }
    plan
}

/// Apply `incoming` onto `current` per `mode`, producing a NEW document with
/// the current settings and revision preserved and the union validated. This
/// is the all-or-nothing apply: on any error nothing is mutated and `Err` is
/// returned.
///
/// Categories/tags are upserted by id (import wins for the same id, new ones
/// added). Imported folders are applied exactly as `plan_import` classified
/// them (same `classify` — the preview and the apply cannot diverge).
pub fn apply_import(
    current: &AppData,
    incoming: &AppData,
    mode: ImportMode,
) -> Result<(StoredDocumentV1, ImportPlan), ImportError> {
    // Build the union of categories/tags first so references resolve.
    let mut categories = Vec::with_capacity(current.categories.len() + incoming.categories.len());
    for category in &current.categories {
        categories.push(category.clone());
    }
    for category in &incoming.categories {
        if let Some(existing) = categories.iter_mut().find(|c| c.id == category.id) {
            *existing = category.clone();
        } else {
            categories.push(category.clone());
        }
    }
    let mut tags = Vec::with_capacity(current.tags.len() + incoming.tags.len());
    for tag in &current.tags {
        tags.push(tag.clone());
    }
    for tag in &incoming.tags {
        if let Some(existing) = tags.iter_mut().find(|t| t.id == tag.id) {
            *existing = tag.clone();
        } else {
            tags.push(tag.clone());
        }
    }

    // Folders per the same classify pass as plan_import. In Overwrite mode an
    // `Updated` record REPLACES the current record (by id); in Merge mode
    // `Updated` is empty (classify returns Skipped), so nothing is replaced.
    let mut plan = ImportPlan::default();
    let mut seen_added_paths: Vec<String> = Vec::new();
    let mut folders: Vec<FolderEntry> = current.folders.clone();
    for incoming_folder in &incoming.folders {
        let (status, name) = classify(incoming_folder, current, &mut seen_added_paths, mode);
        match status {
            ItemStatus::Updated => {
                // Overwrite of an existing record by id; keep created_at.
                if let Some(index) = folders.iter().position(|f| f.id == incoming_folder.id) {
                    let created_at = folders[index].created_at;
                    let mut incoming_folder = incoming_folder.clone();
                    incoming_folder.created_at = created_at;
                    folders[index] = incoming_folder;
                } else {
                    // Path-collision update in Overwrite mode: replace the
                    // record that owns the path, keeping its id + created_at.
                    let index = folders
                        .iter()
                        .position(|f| same_path(&f.path, &incoming_folder.path))
                        .expect("path collision target found by classify");
                    let id = folders[index].id;
                    let created_at = folders[index].created_at;
                    let mut incoming_folder = incoming_folder.clone();
                    incoming_folder.id = id;
                    incoming_folder.created_at = created_at;
                    folders[index] = incoming_folder;
                }
                plan.updated.push(name);
            }
            ItemStatus::Added => {
                folders.push(incoming_folder.clone());
                plan.added.push(name);
            }
            ItemStatus::Skipped => {
                plan.skipped.push(name);
            }
            ItemStatus::Conflict => {
                // M06 review M3: an ambiguous path collision (overwriting by id
                // would leave two records with the same path). All-or-nothing:
                // the whole import is refused below; nothing is mutated.
                plan.conflicts.push(name);
            }
        }
    }
    // M06 review M3, all-or-nothing: any path-collision `Conflict` is
    // ambiguous, so refuse the ENTIRE import (never apply the non-conflicting
    // records while silently dropping the ambiguous one).
    if !plan.conflicts.is_empty() {
        return Err(ImportError::new(ImportErrorKind::DuplicatePath));
    }

    let mut data = AppData {
        settings: current.settings.clone(),
        folders,
        categories,
        tags,
        revision: current.revision,
    };
    // All-or-nothing guard #1: the union must re-validate (references,
    // settings, ids), or nothing is applied.
    if data.validate_and_normalize().is_err() {
        return Err(ImportError::new(ImportErrorKind::UnresolvedReference));
    }
    // All-or-nothing guard #2 (M06 review M3): the merged document must not
    // contain two records with the same path under M01.2 semantics. This can
    // only arise when an id-match overwrote a record with a path that another
    // current record already owns (classify already turns that case into
    // `Conflict`, but the final invariant is enforced here regardless, so the
    // persisted document can never hold a duplicate path).
    if duplicate_paths(&data.folders) {
        return Err(ImportError::new(ImportErrorKind::DuplicatePath));
    }
    Ok((StoredDocumentV1::new(data), plan))
}

/// Whether any two records in `folders` share a path under M01.2 semantics
/// (the path-uniqueness invariant the repository/maintenance layer maintains).
fn duplicate_paths(folders: &[FolderEntry]) -> bool {
    // Same composite key the repository uses for maintenance duplicate scans
    // (`path_key` class + normalized text), so the import cannot contradict
    // the live uniqueness invariant.
    let mut seen: Vec<String> = Vec::with_capacity(folders.len());
    for folder in folders {
        let Some(key) = crate::domain::path_semantics::path_key(&folder.path) else {
            continue;
        };
        let entry = format!("{:?}\u{1}{}", key.class, key.normalized);
        if seen.contains(&entry) {
            return true;
        }
        seen.push(entry);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        folder::{Category, FolderColor, FolderEntry, Tag},
        ids::{CategoryId, FolderId, TagId},
    };
    use crate::storage::schema::CURRENT_SCHEMA_VERSION;
    use chrono::DateTime;
    use uuid::Uuid;

    fn utc(value: &str) -> DateTime<chrono::Utc> {
        value.parse().expect("fixture parses")
    }

    fn folder(id: u128, name: &str, path: &str) -> FolderEntry {
        FolderEntry {
            id: FolderId::from_uuid(Uuid::from_u128(id)),
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

    fn base_data() -> AppData {
        AppData {
            settings: crate::domain::settings::AppSettings::default(),
            folders: vec![folder(1, "Docs", r"C:\docs")],
            categories: vec![Category {
                id: CategoryId::from_uuid(Uuid::from_u128(100)),
                name: "工作".into(),
                color: None,
            }],
            tags: vec![Tag {
                id: TagId::from_uuid(Uuid::from_u128(200)),
                name: "重要".into(),
            }],
            revision: 2,
        }
    }

    fn doc(data: AppData) -> StoredDocumentV1 {
        StoredDocumentV1 {
            schema_version: CURRENT_SCHEMA_VERSION,
            data,
        }
    }

    #[test]
    fn own_export_round_trips_through_parse() {
        let document = doc(base_data());
        let bytes = export_bytes(&document).expect("export");
        let parsed = parse_import(&bytes).expect("parse");
        assert_eq!(parsed, document);
    }

    #[test]
    fn parse_rejects_garbage_future_and_older_schema_with_no_mutation() {
        // Not JSON.
        assert_eq!(
            parse_import(b"not json").unwrap_err().kind,
            ImportErrorKind::InvalidJson
        );
        // Future schema version.
        let future = r#"{"schema_version":99,"settings":{},"folders":[],"categories":[],"tags":[],"revision":1}"#;
        assert_eq!(
            parse_import(future.as_bytes()).unwrap_err().kind,
            ImportErrorKind::FutureSchema
        );
        // Older schema version.
        let older = r#"{"schema_version":0,"settings":{},"folders":[],"categories":[],"tags":[],"revision":1}"#;
        assert_eq!(
            parse_import(older.as_bytes()).unwrap_err().kind,
            ImportErrorKind::MigrationNeeded
        );
        // Valid structural JSON (full settings) but failing validation: a
        // folder whose category reference is unknown -> InvalidDocument.
        let bad = br#"{"schema_version":1,"settings":{"theme":"system","max_results":8,"window_width":600,"search_paths":true,"search_categories":true,"search_tags":true,"search_notes":false,"fuzzy_matching":true,"search_pinyin":true,"search_english_initials":true,"max_edit_distance":1,"hide_after_open":true,"clear_after_open":true,"hide_on_focus_loss":true,"launch_at_login":false},"folders":[{"id":"00000000-0000-0000-0000-00000000000a","display_name":"A","aliases":[],"path":"C:\\a","enabled":true,"favorite":false,"pinned":false,"manual_weight":0,"category_id":"00000000-0000-0000-0000-0000000000ff","tag_ids":[],"note":"","color":null,"sort_order":0,"created_at":"2026-09-21T00:00:00Z","updated_at":"2026-09-21T00:00:00Z","last_opened_at":null,"open_count":0}],"categories":[],"tags":[],"revision":1}"#;
        assert_eq!(
            parse_import(bad).unwrap_err().kind,
            ImportErrorKind::InvalidDocument
        );
    }

    #[test]
    fn overwrite_mode_updates_path_collisions_and_adds_new_records() {
        let current = base_data();
        // Incoming: same path as Docs (updated), plus a brand-new record.
        let mut incoming_folders = Vec::new();
        let mut same = folder(1, "Docs Renamed", r"C:\docs");
        same.created_at = utc("2099-01-01T00:00:00Z"); // import's own created_at
        incoming_folders.push(same);
        incoming_folders.push(folder(9, "New", r"D:\new"));
        let incoming = AppData {
            folders: incoming_folders,
            ..base_data()
        };

        let plan = plan_import(&current, &incoming, ImportMode::Overwrite);
        assert_eq!(plan.updated, ["Docs Renamed"]);
        assert_eq!(plan.added, ["New"]);
        assert_eq!(plan.skipped_count(), 0);
        assert_eq!(plan.conflicts_count(), 0);

        let (applied, applied_plan) =
            apply_import(&current, &incoming, ImportMode::Overwrite).expect("apply");
        assert_eq!(applied_plan, plan);
        // The renamed record keeps its original created_at.
        let docs = applied
            .data
            .folders
            .iter()
            .find(|f| f.id.as_uuid() == Uuid::from_u128(1))
            .expect("docs kept under id 1");
        assert_eq!(docs.display_name, "Docs Renamed");
        assert_eq!(docs.created_at, utc("2026-09-21T00:00:00Z"));
        // New record present.
        assert!(applied.data.folders.iter().any(|f| f.display_name == "New"));
        // Settings/revision preserved.
        assert_eq!(applied.data.revision, 2);
    }

    #[test]
    fn overwrite_where_id_match_duplicates_another_records_path_is_refused() {
        // M06 review M3: incoming record 1 has id==A.id but path==B.path (A, B
        // are two different current records). Overwriting A by id would leave
        // two records (A and B) with the same path — the M01.2 path-uniqueness
        // invariant. The whole import must be refused (all-or-nothing), with a
        // clear DuplicatePath error, and nothing mutated.
        let mut current = base_data();
        current.folders.push(folder(2, "Work", r"D:\work"));
        let mut incoming = folder(1, "Docs But Work Path", r"D:\work"); // id==A(1), path==B(2)
        incoming.created_at = utc("2099-01-01T00:00:00Z");
        let incoming_data = AppData {
            folders: vec![incoming],
            ..base_data()
        };

        // Preview flags the ambiguous record as a conflict (not "updated").
        let plan = plan_import(&current, &incoming_data, ImportMode::Overwrite);
        assert_eq!(plan.conflicts, ["Docs But Work Path"]);
        assert_eq!(plan.updated_count(), 0);

        // Apply refuses the WHOLE import (all-or-nothing), no mutation.
        let result = apply_import(&current, &incoming_data, ImportMode::Overwrite);
        assert_eq!(
            result.unwrap_err().kind,
            ImportErrorKind::DuplicatePath,
            "ambiguous path-id overwrite must be refused"
        );
        assert_eq!(current.folders.len(), 2, "current untouched");
    }

    #[test]
    fn non_colliding_overwrite_still_applies() {
        // M06 review M3: when the id-match keeps its own path (no collision
        // with any OTHER record) overwrite still works exactly as before.
        let current = base_data(); // single record A: id 1, path C:\docs
        let mut same = folder(1, "Docs Renamed", r"C:\docs"); // id A + A's path
        same.created_at = utc("2099-01-01T00:00:00Z");
        let incoming = AppData {
            folders: vec![same, folder(9, "New", r"D:\new")],
            ..base_data()
        };
        let plan = plan_import(&current, &incoming, ImportMode::Overwrite);
        assert_eq!(plan.updated, ["Docs Renamed"]);
        assert_eq!(plan.added, ["New"]);
        assert_eq!(plan.conflicts_count(), 0);

        let (applied, _) = apply_import(&current, &incoming, ImportMode::Overwrite).expect("apply");
        let docs = applied
            .data
            .folders
            .iter()
            .find(|f| f.id.as_uuid() == Uuid::from_u128(1))
            .expect("docs under id 1");
        assert_eq!(docs.display_name, "Docs Renamed");
        assert_eq!(applied.data.folders.len(), 2);
        // Path uniqueness preserved: exactly one record owns C:\docs.
        assert_eq!(
            applied
                .data
                .folders
                .iter()
                .filter(|f| same_path(&f.path, r"C:\docs"))
                .count(),
            1
        );
    }

    #[test]
    fn own_export_overwrite_round_trip_is_still_accepted() {
        // M06 review M3: re-importing the app's OWN export in Overwrite mode
        // (identical ids AND identical paths — a normal "restore same file"
        // flow) must keep working: no false conflict, no duplicate path.
        let current = base_data();
        let document = doc(current.clone());
        let bytes = export_bytes(&document).expect("export");
        let parsed = parse_import(&bytes).expect("parse own export");
        let plan = plan_import(&current, &parsed.data, ImportMode::Overwrite);
        assert_eq!(plan.updated, ["Docs"]);
        assert_eq!(plan.conflicts_count(), 0);

        let (applied, _) =
            apply_import(&current, &parsed.data, ImportMode::Overwrite).expect("apply");
        assert_eq!(applied.data.folders.len(), 1);
        assert!(
            applied
                .data
                .folders
                .iter()
                .any(|f| f.id == current.folders[0].id)
        );
        assert!(
            applied
                .data
                .folders
                .iter()
                .any(|f| same_path(&f.path, r"C:\docs"))
        );
    }

    #[test]
    fn merge_mode_skips_present_paths_and_adds_missing() {
        let current = base_data();
        let incoming = AppData {
            folders: vec![
                folder(1, "Docs Import", r"C:\docs"),
                folder(9, "New", r"D:\new"),
            ],
            ..base_data()
        };
        let plan = plan_import(&current, &incoming, ImportMode::Merge);
        assert_eq!(plan.skipped, ["Docs Import"]);
        assert_eq!(plan.added, ["New"]);

        let (applied, _) = apply_import(&current, &incoming, ImportMode::Merge).expect("apply");
        // Current Docs is untouched (same id+path kept).
        let docs = applied
            .data
            .folders
            .iter()
            .find(|f| f.id.as_uuid() == Uuid::from_u128(1))
            .expect("docs present");
        assert_eq!(docs.display_name, "Docs");
        assert_eq!(applied.data.folders.len(), 2);
    }

    #[test]
    fn skip_duplicates_dedupes_within_the_file() {
        let current = base_data();
        let incoming = AppData {
            folders: vec![
                folder(1, "A", r"C:\docs"), // path exists -> skipped
                folder(13, "B", r"D:\x"),   // added
                folder(14, "C", r"D:\y"),   // added
                folder(15, "D", r"D:\x"),   // in-file dup of B -> skipped
                folder(16, "E", r"D:\z"),   // added
            ],
            ..base_data()
        };
        let plan = plan_import(&current, &incoming, ImportMode::SkipDuplicates);
        assert_eq!(plan.skipped.len(), 2, "present path + in-file dup");
        assert_eq!(plan.added, ["B", "C", "E"]);
        let (applied, _) =
            apply_import(&current, &incoming, ImportMode::SkipDuplicates).expect("apply");
        assert!(applied.data.folders.iter().any(|f| f.path == r"D:\x"));
        assert_eq!(
            applied
                .data
                .folders
                .iter()
                .filter(|f| f.path == r"D:\x")
                .count(),
            1,
            "in-file duplicate path added exactly once"
        );
    }

    #[test]
    fn categories_and_tags_are_upserted_and_references_resolve() {
        let current = base_data();
        // Incoming reuses the same category id with a new name + a new tag, and
        // a folder referencing both.
        let category_id = CategoryId::from_uuid(Uuid::from_u128(100));
        let tag_id = TagId::from_uuid(Uuid::from_u128(201));
        let mut f = folder(30, "Referenced", r"E:\data");
        f.category_id = Some(category_id);
        f.tag_ids = vec![tag_id];
        let incoming = AppData {
            settings: current.settings.clone(),
            folders: vec![f],
            categories: vec![Category {
                id: category_id,
                name: "工作 (new)".into(),
                color: Some(FolderColor(0x1122_33FF)),
            }],
            tags: vec![Tag {
                id: tag_id,
                name: "新标签".into(),
            }],
            revision: 99,
        };
        let (applied, _) = apply_import(&current, &incoming, ImportMode::Merge).expect("apply");
        // Category renamed (upsert), tag added.
        let cat = applied
            .data
            .categories
            .iter()
            .find(|c| c.id == category_id)
            .expect("category");
        assert_eq!(cat.name, "工作 (new)");
        assert!(applied.data.tags.iter().any(|t| t.id == tag_id));
        // The referenced folder resolved its references.
        assert_eq!(applied.data.folders.len(), 2);
    }

    #[test]
    fn conflict_rejected_when_union_cannot_validate() {
        // The all-or-nothing apply guard: a folder whose category reference is
        // absent from BOTH the current document and the incoming file cannot
        // resolve, so `apply_import` rejects and mutates nothing.
        let current = base_data();
        let mut folder_orphan = folder(40, "Orphan", r"F:\x");
        folder_orphan.category_id = Some(CategoryId::from_uuid(Uuid::from_u128(777)));
        let incoming = AppData {
            settings: current.settings.clone(),
            folders: vec![folder_orphan],
            categories: Vec::new(),
            tags: Vec::new(),
            revision: 1,
        };
        let result = apply_import(&current, &incoming, ImportMode::Merge);
        assert!(result.is_err(), "orphan reference must be rejected");
        assert_eq!(
            result.unwrap_err().kind,
            ImportErrorKind::UnresolvedReference
        );
        // And the current data is untouched.
        assert_eq!(current.folders.len(), 1);
    }
}
