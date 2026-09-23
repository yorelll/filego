//! Pure folder/category/tag management logic (M05) — no Slint, no I/O.
//!
//! This module owns the *decision* layer for the settings management page and
//! dialogs: add/edit validation, duplicate policy, record removal with a
//! deterministic undo, controlled one-level child import, name policy, and the
//! management list projection (filter/sort). Everything is unit-tested without
//! a window. The Slint adapter is a thin shell that forwards gestures and
//! re-renders pushed state; the repository (`crate::storage::repository`) does
//! the persistence.
//!
//! # Privacy
//!
//! Errors are anonymous: no full path is ever carried in an error enum carried
//! up to the UI. Dialog descriptions that *do* show path-ish labels are
//! presentation state, never serialized and never logged.

use crate::domain::{
    folder::{Category, FolderColor, FolderEntry, MAX_FAVORITES, Tag},
    ids::{CategoryId, FolderId, TagId},
    path_semantics::{leaf_name, path_key, same_path},
};

/// Upper bound for the controlled one-level child import (M05.2). A parent
/// pick yields at most this many direct children. The value is part of the
/// user-facing hover string (`max 100`) and exercised by tests.
pub const MAX_ONE_LEVEL_IMPORT: usize = 100;

/// How long the single-item remove undo window stays open (wall-clock hint for
/// the UI; the undo logic itself is deterministic on revision, see
/// [`crate::storage::repository::DocumentRepository::undo_remove_record`]).
pub const UNDO_WINDOW: std::time::Duration = std::time::Duration::from_secs(8);

/// Display level of a management page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagePage {
    Folders,
    Categories,
    Tags,
}

/// Filter dimensions for the folder management list (M05.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderFilter {
    /// Case/accent-preserving name substring (original case; matching is
    /// ASCII-case-insensitive).
    pub name: String,
    /// `Some(id)` keeps only that category; `Some(CategoryId 0)` = 未分类
    /// (`None` category). `None` = no category filter.
    pub category: Option<CategoryId>,
    /// `Some(false)` keeps disabled; `Some(true)` keeps enabled; `None` = all.
    pub enabled: Option<bool>,
}

/// Sort key for the folder management list (M05.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderSort {
    Name,
    RecentlyUsed,
    AddedTime,
}

/// One row of the management list (already resolved from the document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    pub id: FolderId,
    pub display_name: String,
    pub path: String,
    pub category_name: Option<String>,
    pub tag_names: Vec<String>,
    pub enabled: bool,
    pub pinned: bool,
    pub favorite: bool,
    pub last_opened_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub manual_weight: i16,
    pub open_count: u64,
}

/// Outcome of evaluating one prospective record (add or edit) against the
/// current document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateVerdict {
    /// No problem; the candidate can be saved.
    Ok,
    /// The classification could not be derived (empty/whitespace path). The
    /// candidate is not saved.
    InvalidPath,
    /// The candidate duplicates an existing record under M01.2 semantics.
    Duplicate { existing_name: String },
}

/// What the user chose when a duplicate was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatePolicy {
    /// Abort (default: not added).
    Cancel,
    /// Open the existing record for editing.
    EditExisting,
    /// Keep the same path but store it under a different name.
    SaveAsDifferentName,
}

/// The pre-apply result of one folder candidate in a multi-add import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddItemStatus {
    /// Ready to be added (no duplicate, valid path, display name derived).
    Ready { resolved_name: String },
    /// Duplicate of the existing record `existing_name`.
    Duplicate { existing_name: String },
    /// Unusable input (empty path etc.) — reported per item, never fatal.
    Invalid,
}

/// The controlled one-level import offer (M05.2). `None` when the setting is
/// OFF; `Some` when the user picked a parent and the setting allows offering
/// direct-child import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildImportOffer {
    /// The parent itself (only the picked directory is imported).
    ParentOnly,
    /// Offer a choice between parent-only and its direct children.
    Choose { child_count: usize },
}

/// What the user selected for a parent pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildImportChoice {
    ParentOnly,
    DirectChildren,
}

/// Anonymous, actionable errors for management operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageError {
    /// No document is loaded in the repository.
    NoData,
    /// The requested record does not exist (already removed).
    NotFound,
    /// The document/tag/category name violates the naming policy.
    InvalidName,
    /// The path fails classification (empty/whitespace).
    InvalidPath,
    /// The merge target is missing.
    MergeTargetMissing,
    /// Saving failed at the repository layer.
    SaveFailed,
    /// The path duplicates an existing record.
    Duplicate,
}

/// A closure-free, deterministic name-policy guard (M05.4: create/rename).
/// "Exact duplicate" is defined as name-equal under ASCII case folding with
/// surrounding whitespace trimmed; the *displayed* original is never changed.
#[derive(Debug, Clone, Default)]
pub struct NamingPolicy;

/// The normalized (trimmed, whitespace-internal preserved) name.
pub fn normalize_name(name: &str) -> String {
    name.trim().to_owned()
}

/// Whether two names are "exactly duplicate" under the documented policy.
pub fn names_equal(a: &str, b: &str) -> bool {
    normalize_name(a).eq_ignore_ascii_case(&normalize_name(b))
}

/// Build the management list rows (M05.1) with filter + sort applied.
pub fn folder_rows(
    folders: &[FolderEntry],
    categories: &[Category],
    tags: &[Tag],
    filter: &FolderFilter,
    sort: FolderSort,
) -> Vec<FolderRow> {
    let category_name = |id: Option<CategoryId>| -> Option<String> {
        categories
            .iter()
            .find(|category| Some(category.id) == id)
            .map(|category| category.name.clone())
    };
    let tag_name = |ids: &[TagId]| -> Vec<String> {
        tags.iter()
            .filter(|tag| ids.contains(&tag.id))
            .map(|tag| tag.name.clone())
            .collect()
    };

    let mut rows: Vec<FolderRow> = folders
        .iter()
        .filter(|folder| {
            let name_matches = filter.name.is_empty()
                || folder
                    .display_name
                    .to_ascii_lowercase()
                    .contains(&filter.name.to_ascii_lowercase());
            let category_matches = match filter.category {
                None => true,
                Some(expected) => {
                    // Some(CategoryId::from_uuid(0)) means "未分类" (None).
                    let is_uncategorized = expected == CategoryId::from_uuid(uuid::Uuid::nil());
                    if is_uncategorized {
                        folder.category_id.is_none()
                    } else {
                        folder.category_id == Some(expected)
                    }
                }
            };
            let status_matches = filter
                .enabled
                .is_none_or(|enabled| folder.enabled == enabled);
            name_matches && category_matches && status_matches
        })
        .map(|folder| FolderRow {
            id: folder.id,
            display_name: folder.display_name.clone(),
            path: folder.path.clone(),
            category_name: category_name(folder.category_id),
            tag_names: tag_name(&folder.tag_ids),
            enabled: folder.enabled,
            pinned: folder.pinned,
            favorite: folder.favorite,
            last_opened_at: folder.last_opened_at,
            created_at: folder.created_at,
            manual_weight: folder.manual_weight,
            open_count: folder.open_count,
        })
        .collect();

    match sort {
        FolderSort::Name => rows.sort_by(|a, b| {
            a.display_name
                .to_ascii_lowercase()
                .cmp(&b.display_name.to_ascii_lowercase())
                .then_with(|| a.display_name.cmp(&b.display_name))
        }),
        FolderSort::RecentlyUsed => rows.sort_by(|a, b| {
            b.last_opened_at
                .cmp(&a.last_opened_at)
                .then_with(|| b.open_count.cmp(&a.open_count))
        }),
        FolderSort::AddedTime => rows.sort_by_key(|row| row.created_at),
    }
    rows
}

/// How many favorites a change would use, and whether the cap
/// `MAX_FAVORITES` would be exceeded.
///
/// `folders` are the current records; `nominee_ids` are the ids that will end
/// up with `favorite == true` after the change (a fresh batch to set favorite,
/// or a toggle to favorite). The result is the size of the union of the
/// already-favorite set and the nominee set — each record counts once, whether
/// it is an existing favorite, a favorite-to-be-toggled-off target, or a new
/// record.
pub fn favorite_room(folders: &[FolderEntry], nominee_ids: &[FolderId]) -> (usize, bool) {
    let favorite_ids: Vec<FolderId> = folders
        .iter()
        .filter(|folder| folder.favorite)
        .map(|folder| folder.id)
        .collect();
    let used = favorite_ids.len()
        + nominee_ids
            .iter()
            .filter(|id| !favorite_ids.contains(id))
            .count();
    (used, used > MAX_FAVORITES)
}

/// Locate duplicates in a batch of candidate paths (multi-folder add /
/// drag-drop preview), M01.2-aware and deduplicated *within* the batch.
/// Returns per-candidate status preserving input order.
pub fn preview_batch(
    candidates: &[String],
    existing_folders: &[FolderEntry],
    existing_names: &[String],
) -> Vec<AddItemStatus> {
    let mut seen_normalized: Vec<String> = Vec::new();
    candidates
        .iter()
        .map(|raw| {
            let Some(key) = path_key(raw) else {
                return AddItemStatus::Invalid;
            };
            let marker = format!("{:?}\u{1}{}", key.class, key.normalized);
            let duplicates_record = existing_folders
                .iter()
                .any(|folder| same_path(&folder.path, raw));
            let duplicates_batch = seen_normalized.contains(&marker);
            if duplicates_record || duplicates_batch {
                let existing_name = existing_folders
                    .iter()
                    .find(|folder| same_path(&folder.path, raw))
                    .map(|folder| folder.display_name.clone())
                    .unwrap_or_else(|| existing_names.first().cloned().unwrap_or_default());
                return AddItemStatus::Duplicate { existing_name };
            }
            seen_normalized.push(marker);
            let name = leaf_name(raw).unwrap_or_else(|| raw.trim().to_owned());
            AddItemStatus::Ready {
                resolved_name: name,
            }
        })
        .collect()
}

/// Evaluate a single add/edit candidate against the document.
pub fn evaluate_candidate(
    path: &str,
    folders: &[FolderEntry],
    exclude_id: Option<FolderId>,
) -> CandidateVerdict {
    let Some(_) = path_key(path) else {
        return CandidateVerdict::InvalidPath;
    };
    let duplicate = folders
        .iter()
        .find(|folder| exclude_id != Some(folder.id) && same_path(&folder.path, path))
        .map(|folder| folder.display_name.clone());
    match duplicate {
        Some(existing_name) => CandidateVerdict::Duplicate { existing_name },
        None => CandidateVerdict::Ok,
    }
}

/// Derive the display name suggestion for a raw path (auto leaf name).
pub fn suggested_display_name(path: &str) -> String {
    leaf_name(path).unwrap_or_else(|| path.trim().to_owned())
}

/// Build a fresh saved folder record from dialog inputs (M05.2). `id` is
/// supplied by the caller: a new `FolderId` for an add, the edited record's id
/// for an edit. `created_at` is the original `created_at` for an edit and the
/// current time for an add.
#[allow(clippy::too_many_arguments)]
pub fn build_folder(
    id: FolderId,
    display_name: String,
    path: String,
    aliases: Vec<String>,
    category_id: Option<CategoryId>,
    tag_ids: Vec<TagId>,
    note: String,
    pinned: bool,
    favorite: bool,
    manual_weight: i16,
    enabled: bool,
    color: Option<FolderColor>,
    created_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> FolderEntry {
    FolderEntry {
        id,
        display_name,
        aliases,
        path,
        enabled,
        favorite,
        pinned,
        manual_weight,
        category_id,
        tag_ids,
        note,
        color,
        sort_order: 0,
        created_at,
        updated_at: now,
        last_opened_at: None,
        open_count: 0,
    }
}

/// Decide whether a parent pick offers the controlled one-level import
/// (setting ON + the pick has at least one direct child).
pub fn child_import_offer(setting_enabled: bool, direct_child_count: usize) -> ChildImportOffer {
    if !setting_enabled {
        return ChildImportOffer::ParentOnly;
    }
    if direct_child_count == 0 {
        ChildImportOffer::ParentOnly
    } else {
        ChildImportOffer::Choose {
            child_count: direct_child_count,
        }
    }
}

/// Clamp an import to `MAX_ONE_LEVEL_IMPORT` direct children (a pre-preview
/// bound used by the UI before building the explicit child list). The import
/// itself never enumerates recursively — the caller's `read_dir` is explicit
/// one-level and user-initiated.
pub fn clamp_child_import(count: usize, offer: ChildImportChoice) -> usize {
    match offer {
        ChildImportChoice::ParentOnly => 1,
        ChildImportChoice::DirectChildren => count.min(MAX_ONE_LEVEL_IMPORT),
    }
}

/// The human hover string that must literally contain `max 100`.
pub fn child_import_hover(limit: usize) -> String {
    format!("max {limit}")
}

/// Whether `name` is acceptable as a category/tag name (trimmed, non-empty,
/// within limit).
pub fn valid_named_value(name: &str, max_len: usize) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.chars().count() <= max_len
}

/// Merge target resolution for tags: a tag named exactly like the merge
/// target (case-insensitive ASCII) is located, else the target id.
pub fn resolve_merge_target_id(tags: &[Tag], target_id: TagId, target_name: &str) -> TagId {
    tags.iter()
        .find(|tag| tag.id != target_id && names_equal(&tag.name, target_name))
        .map(|tag| tag.id)
        .unwrap_or(target_id)
}

/// Usage count of a tag (folders referencing it).
pub fn tag_usage(folders: &[FolderEntry], tag_id: TagId) -> usize {
    folders
        .iter()
        .filter(|folder| folder.tag_ids.contains(&tag_id))
        .count()
}

/// The `max 100` literal for the import hover (kept as a constant so tests
/// and the UI share it).
pub const ONE_LEVEL_IMPORT_MAX_LITERAL: &str = "max 100";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{folder::MAX_TAG_NAME_LEN, ids::FolderId};
    use uuid::Uuid;

    fn utc(s: &str) -> chrono::DateTime<chrono::Utc> {
        s.parse().unwrap()
    }

    fn tid(n: u128) -> TagId {
        TagId::from_uuid(Uuid::from_u128(n))
    }
    fn cid(n: u128) -> CategoryId {
        CategoryId::from_uuid(Uuid::from_u128(n))
    }
    fn fid(n: u128) -> FolderId {
        FolderId::from_uuid(Uuid::from_u128(n))
    }

    fn folder(id: u128, name: &str, path: &str) -> FolderEntry {
        let now = utc("2026-09-21T00:00:00Z");
        FolderEntry {
            id: fid(id),
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
            created_at: now,
            updated_at: now,
            last_opened_at: None,
            open_count: 0,
        }
    }

    #[test]
    fn batch_preview_dedupes_within_batch_and_against_records_m01_2() {
        let existing = vec![folder(1, "Documents", r"C:\Users\me\Documents")];
        let candidates = vec![
            r"C:\Users\me\document".to_owned(), // different component → not dup
            r"C:\Users\me\Documents\".to_owned(), // trailing slash → dup of record
            r"C:\users\me\documents".to_owned(), // case → dup of record
            r"D:\new\资料".to_owned(),          // new, CJK leaf
        ];
        let mut statuses = preview_batch(&candidates, &existing, &[]);
        assert!(matches!(statuses[0], AddItemStatus::Ready { .. }));
        assert!(matches!(statuses[1], AddItemStatus::Duplicate { .. }));
        assert!(matches!(statuses[2], AddItemStatus::Duplicate { .. }));
        assert!(
            matches!(statuses[3], AddItemStatus::Ready { ref resolved_name } if resolved_name == "资料")
        );
        // Deterministic: same input → same result.
        assert_eq!(preview_batch(&candidates, &existing, &[]), statuses);
        let _ = &mut statuses;
    }

    #[test]
    fn duplicate_policy_defaults_to_not_add_and_offers_edit() {
        let folders = vec![folder(1, "Docs", r"C:\a\b")];
        // Default: duplicate blocks the add.
        assert_eq!(
            evaluate_candidate(r"C:\a\b", &folders, None),
            CandidateVerdict::Duplicate {
                existing_name: "Docs".to_owned()
            }
        );
        // Edit-existing: excluding the edited record lets it save.
        assert_eq!(
            evaluate_candidate(r"C:\a\b", &folders, Some(fid(1))),
            CandidateVerdict::Ok
        );
    }

    #[test]
    fn inaccessible_but_classifiable_paths_are_valid() {
        // Offline ≠ invalid: an unreachable UNC still validates.
        let folders = Vec::new();
        assert_eq!(
            evaluate_candidate(r"\\nas\offline\共享 目录", &folders, None),
            CandidateVerdict::Ok
        );
        assert_eq!(
            evaluate_candidate("", &folders, None),
            CandidateVerdict::InvalidPath
        );
        assert_eq!(
            evaluate_candidate("   ", &folders, None),
            CandidateVerdict::InvalidPath
        );
    }

    #[test]
    fn suggested_name_derives_leaf() {
        assert_eq!(
            suggested_display_name(r"C:\Users\me\设计 资料"),
            "设计 资料"
        );
        assert_eq!(
            suggested_display_name(r"C:\Users\me\Documents\"),
            "Documents"
        );
    }

    #[test]
    fn name_policy_is_ascii_fold_and_whitespace_trim_exact() {
        assert!(names_equal("Work", "work"));
        assert!(names_equal(" Work ", "work"));
        assert!(names_equal("客户A", "客户A"));
        // Display original is preserved (normalize only trims).
        assert_eq!(normalize_name("  Work  "), "Work");
        assert!(!names_equal("Work", "Works"));
        // Full-width / accented are NOT ASCII-folded.
        assert!(!names_equal("Ä", "ä"));
    }

    #[test]
    fn favorite_cap_is_respected() {
        let folders = vec![
            {
                let mut f = folder(1, "a", r"C:\a");
                f.favorite = true;
                f
            },
            {
                let mut f = folder(2, "b", r"C:\b");
                f.favorite = true;
                f
            },
            {
                let mut f = folder(3, "c", r"C:\c");
                f.favorite = true;
                f
            },
            {
                let mut f = folder(4, "d", r"C:\d");
                f.favorite = true;
                f
            },
            {
                let mut f = folder(5, "e", r"C:\e");
                f.favorite = true;
                f
            },
        ];
        // 5 are already favorites; adding id 6 pushes over the cap.
        assert_eq!(favorite_room(&folders, &[fid(6)]).0, 6);
        assert!(favorite_room(&folders, &[fid(6)]).1);
        // Toggling an existing favorite stays at 5.
        assert_eq!(favorite_room(&folders, &[fid(1)]).0, 5);
        assert!(!favorite_room(&folders, &[fid(1)]).1);
        // Unfavoriting one frees a slot: with 4 remaining favorites, one new
        // nominee fits; nominating BOTH the unfavorited id and a new one
        // pushes over again.
        let unfavorited: Vec<FolderEntry> = folders
            .iter()
            .map(|f| {
                let mut f = f.clone();
                if f.id == fid(1) {
                    f.favorite = false;
                }
                f
            })
            .collect();
        let (used, over) = favorite_room(&unfavorited, &[fid(6)]);
        assert_eq!(used, 5);
        assert!(!over);
        let (used, over) = favorite_room(&unfavorited, &[fid(1), fid(6)]);
        assert_eq!(used, 6);
        assert!(over);
    }

    #[test]
    fn one_level_import_offer_defaults_off_and_clamps_at_100() {
        assert_eq!(child_import_offer(false, 5), ChildImportOffer::ParentOnly);
        assert_eq!(child_import_offer(true, 0), ChildImportOffer::ParentOnly);
        assert_eq!(
            child_import_offer(true, 5),
            ChildImportOffer::Choose { child_count: 5 }
        );
        assert_eq!(
            clamp_child_import(250, ChildImportChoice::DirectChildren),
            100
        );
        assert_eq!(clamp_child_import(3, ChildImportChoice::DirectChildren), 3);
        assert_eq!(clamp_child_import(250, ChildImportChoice::ParentOnly), 1);
        assert_eq!(ONE_LEVEL_IMPORT_MAX_LITERAL, "max 100");
        assert!(child_import_hover(100).contains("max 100"));
        assert_eq!(MAX_ONE_LEVEL_IMPORT, 100);
    }

    #[test]
    fn management_rows_filter_and_sort() {
        let category_a = cid(1);
        let now = utc("2026-09-20T00:00:00Z");
        let mut docs = folder(1, "Zeta", r"C:\z");
        docs.category_id = Some(category_a);
        docs.pinned = true;
        let mut alpha_disabled = folder(2, "alpha", r"C:\a");
        alpha_disabled.enabled = false;
        let alpha_created = folder(3, "alpha2", r"C:\a2");
        let mut alpha_recent = folder(4, "alpha3", r"C:\a3");
        alpha_recent.last_opened_at = Some(now);

        let categories = vec![Category {
            id: category_a,
            name: "工作".to_owned(),
            color: None,
        }];
        let tags = vec![Tag {
            id: tid(9),
            name: "重要".to_owned(),
        }];

        let all = [
            docs.clone(),
            alpha_disabled.clone(),
            alpha_created,
            alpha_recent.clone(),
        ];

        // Name filter (case-insensitive).
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter {
                name: "ALPHA".into(),
                ..Default::default()
            },
            FolderSort::Name,
        );
        assert_eq!(rows.len(), 3);

        // Status filter.
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter {
                enabled: Some(false),
                ..Default::default()
            },
            FolderSort::Name,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].display_name, "alpha");

        // Category filter + 未分类 sentinel.
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter {
                category: Some(category_a),
                ..Default::default()
            },
            FolderSort::Name,
        );
        assert_eq!(rows.len(), 1);
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter {
                category: Some(CategoryId::from_uuid(Uuid::nil())),
                ..Default::default()
            },
            FolderSort::Name,
        );
        assert_eq!(rows.len(), 3);

        // Sort by name (ASCII-case-insensitive, then original).
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter::default(),
            FolderSort::Name,
        );
        assert_eq!(rows[0].display_name, "alpha");
        assert_eq!(rows[rows.len() - 1].display_name, "Zeta");

        // Sort by recently-used: the only one with last_opened_at first.
        let rows = folder_rows(
            &all,
            &categories,
            &tags,
            &FolderFilter::default(),
            FolderSort::RecentlyUsed,
        );
        assert_eq!(rows[0].display_name, "alpha3");
    }

    #[test]
    fn tag_usage_counts_folders() {
        let mut a = folder(1, "a", r"C:\a");
        a.tag_ids = vec![tid(9), tid(8)];
        let mut b = folder(2, "b", r"C:\b");
        b.tag_ids = vec![tid(9)];
        let folders = vec![a, b, folder(3, "c", r"C:\c")];
        assert_eq!(tag_usage(&folders, tid(9)), 2);
        assert_eq!(tag_usage(&folders, tid(8)), 1);
        assert_eq!(tag_usage(&folders, tid(7)), 0);
    }

    #[test]
    fn build_folder_keeps_id_and_created_at_for_edits() {
        let created = utc("2026-09-01T00:00:00Z");
        let now = utc("2026-09-21T00:00:00Z");
        let entry = build_folder(
            fid(5),
            "Name".to_owned(),
            r"C:\x".to_owned(),
            vec![],
            None,
            vec![],
            String::new(),
            true,
            false,
            10,
            true,
            None,
            created,
            now,
        );
        assert_eq!(entry.id, fid(5));
        assert_eq!(entry.created_at, created);
        assert_eq!(entry.updated_at, now);
        assert!(entry.pinned);
    }

    #[test]
    fn valid_named_value_checks_trim_and_limit() {
        assert!(valid_named_value("Work", MAX_TAG_NAME_LEN));
        assert!(!valid_named_value("   ", MAX_TAG_NAME_LEN));
        assert!(!valid_named_value("", MAX_TAG_NAME_LEN));
        assert!(!valid_named_value(
            "x".repeat(MAX_TAG_NAME_LEN + 1).as_str(),
            MAX_TAG_NAME_LEN
        ));
    }
}
