//! M05 repository CRUD + canary tests.
//!
//! Focus: record-level semantics (never real-directory deletion), deterministic
//! undo across save boundaries, category/tag integrity, and the canary that
//! real folders and their contents survive remove/disable/clear.

use chrono::{DateTime, Utc};
use tempfile::TempDir;
use uuid::Uuid;

use crate::{
    domain::{
        document::AppData,
        folder::{Category, FolderColor, FolderEntry, Tag},
        ids::{CategoryId, FolderId, TagId},
        settings::AppSettings,
    },
    storage::{
        location::DocumentPaths,
        repository::{DocumentRepository, LoadOutcome},
        schema::StoredDocumentV1,
    },
};

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().expect("fixed RFC3339 fixture must parse")
}

fn real_repo(base: &TempDir) -> DocumentRepository {
    DocumentRepository::new(DocumentPaths::from_base_dir(base.path()))
}

fn folder(
    id: u128,
    name: &str,
    path: &str,
    category_id: Option<CategoryId>,
    tag_ids: Vec<TagId>,
) -> FolderEntry {
    let now = utc("2026-09-21T00:00:00Z");
    FolderEntry {
        id: FolderId::from_uuid(Uuid::from_u128(id)),
        display_name: name.to_owned(),
        aliases: Vec::new(),
        path: path.to_owned(),
        enabled: true,
        favorite: false,
        pinned: false,
        manual_weight: 0,
        category_id,
        tag_ids,
        note: String::new(),
        color: None,
        sort_order: 0,
        created_at: now,
        updated_at: now,
        last_opened_at: None,
        open_count: 0,
    }
}

fn seed_document(paths: &[(&str, &str)]) -> StoredDocumentV1 {
    let category_id = CategoryId::from_uuid(Uuid::from_u128(1));
    let tag_a = TagId::from_uuid(Uuid::from_u128(2));
    let tag_b = TagId::from_uuid(Uuid::from_u128(3));
    let mut folders = Vec::new();
    for (index, (name, path)) in paths.iter().enumerate() {
        folders.push(folder(
            u128::from(index as u32) + 10,
            name,
            path,
            if index == 0 { Some(category_id) } else { None },
            if index == 0 {
                vec![tag_a, tag_b]
            } else {
                Vec::new()
            },
        ));
    }
    StoredDocumentV1::new(AppData {
        settings: AppSettings::default(),
        folders,
        categories: vec![Category {
            id: category_id,
            name: "工作".to_owned(),
            color: Some(FolderColor(0x2563_EBFF)),
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

// ---------------------------------------------------------------------------
// record semantics + canary
// ---------------------------------------------------------------------------

#[test]
fn canary_remove_and_disable_keep_real_dir_and_contents() {
    let base = TempDir::new().expect("temp dir");
    let real_dir = base.path().join("real-keep").to_path_buf();
    std::fs::create_dir(&real_dir).expect("canary helper creates real dir");
    let subfile = real_dir.join("sub.txt");
    std::fs::write(&subfile, b"keep me").expect("canary helper writes subfile");
    let subdir = real_dir.join("sub");
    std::fs::create_dir(&subdir).expect("canary helper creates subdir");

    let folder_id = FolderId::from_uuid(Uuid::from_u128(10));
    let doc = seed_document(&[("Real", real_dir.to_str().unwrap())]);
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    // remove_record: record gone, everything on disk stays.
    let removed = repo.remove_record(folder_id);
    assert!(removed);
    assert_eq!(repo.folder_count(), 0);
    repo.save_at().expect("save after remove");
    assert_survives(&real_dir, &subfile, &subdir);

    // Reload fresh: still gone, still physically present.
    let mut fresh = real_repo(&base);
    let loaded = fresh.load().expect("load");
    match loaded {
        LoadOutcome::Found(loaded) => assert!(loaded.data.folders.is_empty()),
        other => panic!("expected Found, got {other:?}"),
    }
    assert_survives(&real_dir, &subfile, &subdir);
}

#[test]
fn canary_clear_all_records_keeps_every_real_dir() {
    let base = TempDir::new().expect("temp dir");
    let real_a = base.path().join("real-a").to_path_buf();
    let real_b = base.path().join("真实 乙").to_path_buf();
    std::fs::create_dir(&real_a).expect("canary helper");
    std::fs::create_dir(&real_b).expect("canary helper");
    let file_a = real_a.join("a.txt");
    std::fs::write(&file_a, b"a").expect("canary helper");

    let doc = seed_document(&[
        ("A", real_a.to_str().unwrap()),
        ("B", real_b.to_str().unwrap()),
    ]);
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    // Clear every folder record (the 0.0.1 "clear all" analog is a loop of
    // remove_record; there is no bulk API that touches real paths).
    let all_ids: Vec<FolderId> = repo
        .document()
        .expect("document")
        .data
        .folders
        .iter()
        .map(|folder| folder.id)
        .collect();
    for id in all_ids {
        assert!(repo.remove_record(id));
    }
    assert_eq!(repo.folder_count(), 0);
    repo.save_at().expect("save after clear");

    assert!(real_a.exists(), "real dir A must survive");
    assert!(real_b.exists(), "real dir B must survive");
    assert!(file_a.exists(), "file inside real dir A must survive");
}

fn assert_survives(
    real_dir: &std::path::Path,
    subfile: &std::path::Path,
    subdir: &std::path::Path,
) {
    assert!(real_dir.exists(), "real dir must physically exist");
    assert!(
        subfile.exists(),
        "file inside real dir must survive remove/disable"
    );
    assert!(subdir.exists(), "subdir inside real dir must survive");
}

#[test]
fn disable_and_enable_toggle_only_the_record_flag() {
    let base = TempDir::new().expect("temp dir");
    let real_dir = base.path().join("real").to_path_buf();
    std::fs::create_dir(&real_dir).expect("canary helper");
    let folder_id = FolderId::from_uuid(Uuid::from_u128(10));
    let doc = seed_document(&[("Real", real_dir.to_str().unwrap())]);

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    repo.disable_record(folder_id).expect("disable");
    assert!(
        !repo.document().unwrap().data.folders[0].enabled,
        "record disabled"
    );
    repo.save_at().expect("save after disable");
    assert!(real_dir.exists(), "disabled record keeps the real dir");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    assert!(!repo.document().unwrap().data.folders[0].enabled);
    repo.enable_record(folder_id).expect("enable");
    assert!(repo.document().unwrap().data.folders[0].enabled);
    assert!(real_dir.exists());
}

// ---------------------------------------------------------------------------
// undo across save boundaries
// ---------------------------------------------------------------------------

#[test]
fn undo_restores_record_when_it_was_the_last_save() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("Real", r"C:\real\path")]);
    let folder_id = doc.data.folders[0].id;
    let previous_revision = doc.data.revision;
    let removed_payload = doc.data.folders[0].clone();

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let expected_current = repo.document().unwrap().data.revision;
    assert!(repo.remove_record(folder_id));
    let new_rev = repo.save_at().expect("save removal");
    assert!(new_rev > expected_current);

    // Undo right after the removal save.
    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let current = repo.document().unwrap().data.revision;
    repo.undo_remove_record(removed_payload.clone(), previous_revision, current)
        .expect("undo must succeed");
    assert_eq!(repo.folder_count(), 1);
}

#[test]
fn undo_is_refused_after_any_later_save() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("Real", r"C:\real\path")]);
    let folder_id = doc.data.folders[0].id;
    let previous_revision = doc.data.revision;
    let removed_payload = doc.data.folders[0].clone();

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    assert!(repo.remove_record(folder_id));
    repo.save_at().expect("save removal");

    // Another unrelated save bumps the revision: the undo must refuse.
    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let _ = repo.save_at().expect("an unrelated save");
    let current = repo.document().unwrap().data.revision;
    assert!(
        repo.undo_remove_record(removed_payload.clone(), previous_revision, current)
            .is_err(),
        "undo across a save boundary must refuse deterministically"
    );
    assert_eq!(repo.folder_count(), 0);
}

#[test]
fn undo_persists_and_survives_reload() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("Real", r"C:\real\path")]);
    let folder_id = doc.data.folders[0].id;
    let previous_revision = doc.data.revision;
    let removed_payload = doc.data.folders[0].clone();

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    assert!(repo.remove_record(folder_id));
    repo.save_at().expect("remove save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let current = repo.document().unwrap().data.revision;
    repo.undo_remove_record(removed_payload, previous_revision, current)
        .expect("undo");
    repo.save_at().expect("undo save");

    let mut fresh = real_repo(&base);
    let loaded = fresh.load().expect("load");
    match loaded {
        LoadOutcome::Found(loaded) => {
            assert_eq!(loaded.data.folders.len(), 1);
            assert_eq!(loaded.data.folders[0].display_name, "Real");
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// category / tag integrity
// ---------------------------------------------------------------------------

#[test]
fn category_delete_clears_references_and_keeps_folders() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("A", r"C:\a"), ("B", r"C:\b")]);
    let category_id = doc.data.categories[0].id;
    let first_folder_id = doc.data.folders[0].id;
    let second_folder_id = doc.data.folders[1].id;

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    assert!(repo.remove_category(category_id));
    assert_eq!(repo.document().unwrap().data.categories.len(), 0);
    // Both folders remain; the categorized one becomes 未分类.
    assert_eq!(repo.folder_count(), 2);
    let folders = &repo.document().unwrap().data.folders;
    assert_eq!(folders[0].id, first_folder_id);
    assert_eq!(folders[0].category_id, None);
    assert_eq!(folders[1].id, second_folder_id);
    assert_eq!(folders[1].category_id, None);
}

#[test]
fn tag_delete_clears_only_associations_and_keeps_folders() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("A", r"C:\a"), ("B", r"C:\b")]);
    let tag_a = doc.data.tags[0].id;
    let tag_b = doc.data.tags[1].id;

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    assert!(repo.remove_tag(tag_b));
    assert_eq!(repo.document().unwrap().data.tags.len(), 1);
    // The first folder keeps tag_a, loses tag_b.
    let folders = &repo.document().unwrap().data.folders;
    assert_eq!(folders[0].tag_ids, vec![tag_a]);
    assert_eq!(repo.folder_count(), 2);

    // Second removal of the same tag is a no-op.
    assert!(!repo.remove_tag(tag_b));
}

#[test]
fn tag_merge_updates_all_references_atomically() {
    let base = TempDir::new().expect("temp dir");
    // Folder A carries both tags; folder B carries only the source tag.
    let mut doc = seed_document(&[("A", r"C:\a"), ("B", r"C:\b")]);
    let tag_a = doc.data.tags[0].id;
    let tag_b = doc.data.tags[1].id;
    let mut b = doc.data.folders[1].clone();
    b.tag_ids = vec![tag_b];
    doc.data.folders[1] = b;

    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    repo.merge_tag(tag_b, tag_a).expect("merge");
    // Source tag record gone; every reference now points at the target.
    let document = repo.document().unwrap();
    assert_eq!(document.data.tags.len(), 1);
    assert!(!document.data.tags.iter().any(|tag| tag.id == tag_b));
    for folder in &document.data.folders {
        assert!(!folder.tag_ids.contains(&tag_b));
        assert!(folder.tag_ids.contains(&tag_a));
    }
    // Usage after merge: both folders use tag_a.
    assert_eq!(repo.tag_usage_count(tag_a), 2);

    // Atomic: one save lands the merged document.
    let revision = repo.save_at().expect("merge save");
    let mut fresh = real_repo(&base);
    fresh.load().expect("load");
    match fresh.document().unwrap().data.revision == revision {
        true => {}
        false => panic!("revision mismatch"),
    }
    let merged_folders = &fresh.document().unwrap().data.folders;
    assert_eq!(merged_folders[0].tag_ids, vec![tag_a]);
    assert_eq!(merged_folders[1].tag_ids, vec![tag_a]);
}

// ---------------------------------------------------------------------------
// CRUD basic + duplicates
// ---------------------------------------------------------------------------

#[test]
fn put_folder_edits_preserve_created_at_and_deduplicate_by_edit() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("A", r"C:\a")]);
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    let original = repo.document().unwrap().data.folders[0].clone();
    let mut edited = original.clone();
    edited.display_name = "A (edited)".to_owned();
    edited.path = r"C:\a\moved".to_owned();
    repo.put_folder(edited).expect("put (edit)");
    let after = &repo.document().unwrap().data.folders[0];
    assert_eq!(after.display_name, "A (edited)");
    assert_eq!(
        after.created_at, original.created_at,
        "edit preserves created_at"
    );
    assert_eq!(
        repo.folder_count(),
        1,
        "edit must not create a second record"
    );
}

#[test]
fn duplicates_are_located_by_m01_2_semantics() {
    let base = TempDir::new().expect("temp dir");
    let mut doc = seed_document(&[]);
    doc.data.folders = vec![
        folder(10, "A", r"C:\Users\Me\docs", None, Vec::new()),
        folder(11, "B", r"c:\users\me\docs\", None, Vec::new()), // dup (case+trailing slash)
        folder(12, "C", r"D:\other", None, Vec::new()),
        folder(13, "D", r"\\srv\share\Data", None, Vec::new()),
        folder(14, "E", r"\\SRV\SHARE\data", None, Vec::new()), // dup UNC
    ];
    doc.data.revision = 5;
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    let duplicates = repo.duplicate_folders(None);
    let duplicate_ids: Vec<FolderId> = duplicates
        .iter()
        .map(|index| repo.document().unwrap().data.folders[*index].id)
        .collect();
    assert!(
        duplicate_ids.contains(&FolderId::from_uuid(Uuid::from_u128(11))),
        "trailing slash + case-folded local dup must be flagged"
    );
    for id in duplicate_ids.iter() {
        assert_ne!(
            id,
            &FolderId::from_uuid(Uuid::from_u128(10)),
            "first occurrence is not a duplicate"
        );
    }
    assert!(
        duplicate_ids.contains(&FolderId::from_uuid(Uuid::from_u128(14))),
        "UNC dup must be flagged"
    );
    assert_eq!(duplicates.len(), 2);

    // Excluding the edited record removes its duplicate status.
    let with_exclusion = repo.duplicate_folders(Some(FolderId::from_uuid(Uuid::from_u128(11))));
    assert!(
        !with_exclusion
            .iter()
            .any(|index| repo.document().unwrap().data.folders[*index].id
                == FolderId::from_uuid(Uuid::from_u128(11)))
    );
}

#[test]
fn record_open_bumps_count_and_stamp() {
    let base = TempDir::new().expect("temp dir");
    let doc = seed_document(&[("A", r"C:\a")]);
    let folder_id = doc.data.folders[0].id;
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");

    repo.record_open(folder_id).expect("record open");
    let folder = &repo.document().unwrap().data.folders[0];
    assert_eq!(folder.open_count, 1);
    assert!(folder.last_opened_at.is_some());
    assert!(
        repo.record_open(FolderId::from_uuid(Uuid::from_u128(999)))
            .is_err()
    );
}
