//! Tests for M01-B safe atomic persistence.
//!
//! Plan of record (each save performs exactly these `FileOps` sub-steps, so
//! the fault matrix knows which step index to target):
//!   step 0,1: `write_flush_sync` (write then sync)
//!   step 2:   `exists` main probe
//!   step 3:   `copy` main → backup
//!   step 4:   `rename` temp → main
//!   — cleanup: one `remove` per stale temp sibling (guarded by the read_dir
//!     in `cleanup_stale_temps`, which is real `std::fs` and not faulted).
//!
//! `load` performs: `read` main, then `read` backup only when the main fails
//! to decode.

use chrono::{DateTime, Utc};
use tempfile::TempDir;
use uuid::Uuid;

use crate::{
    domain::{
        document::AppData,
        folder::{Category, FolderEntry, Tag},
        ids::{CategoryId, FolderId, TagId},
        settings::AppSettings,
    },
    storage::{
        codec as codec_mod,
        location::{DocumentPaths, backup_path, main_path, temp_path},
        repository::{DocumentRepository, LoadOutcome, RepositoryError},
        schema::StoredDocumentV1,
    },
};

use crate::storage::io::fault::{FaultOp, FaultPoint, FaultyFileOps};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().expect("fixed RFC3339 fixture must parse")
}

fn fixture_document() -> StoredDocumentV1 {
    let category_id = CategoryId::from_uuid(Uuid::from_u128(1));
    let first_tag_id = TagId::from_uuid(Uuid::from_u128(2));
    let second_tag_id = TagId::from_uuid(Uuid::from_u128(3));

    StoredDocumentV1::new(AppData {
        settings: AppSettings::default(),
        folders: vec![FolderEntry {
            id: FolderId::from_uuid(Uuid::from_u128(4)),
            display_name: "项目 文档 🗂️".to_owned(),
            aliases: vec!["项目资料".to_owned(), "docs".to_owned()],
            path: r"\\sensitive-server\private-share\机密 文件夹".to_owned(),
            enabled: true,
            favorite: true,
            pinned: true,
            manual_weight: 25,
            category_id: Some(category_id),
            tag_ids: vec![first_tag_id, second_tag_id],
            note: "DO-NOT-LEAK-JSON-CONTENT".to_owned(),
            color: Some(crate::domain::folder::FolderColor(0x2563_EBFF)),
            sort_order: -7,
            created_at: utc("2026-09-21T00:00:00Z"),
            updated_at: utc("2026-09-21T00:01:00Z"),
            last_opened_at: Some(utc("2026-09-21T00:02:00Z")),
            open_count: 4,
        }],
        categories: vec![Category {
            id: category_id,
            name: "工作".to_owned(),
            color: Some(crate::domain::folder::FolderColor(0x2563_EBFF)),
        }],
        tags: vec![
            Tag {
                id: first_tag_id,
                name: "重要".to_owned(),
            },
            Tag {
                id: second_tag_id,
                name: "客户 A".to_owned(),
            },
        ],
        revision: 2,
    })
}

fn fixture_with_revision(revision: u64) -> StoredDocumentV1 {
    let mut document = fixture_document();
    document.data.revision = revision;
    document
}

/// Write arbitrary bytes straight to disk (real fs; not faulted).
fn write_raw(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("test must be able to seed raw file");
}

fn read_raw(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).expect("test must be able to read raw file")
}

fn real_paths(base: &TempDir) -> DocumentPaths {
    DocumentPaths::from_base_dir(base.path())
}

fn real_repo(base: &TempDir) -> DocumentRepository {
    DocumentRepository::new(real_paths(base))
}

/// Build a repository whose `FileOps` is a `FaultyFileOps` configured to fail
/// the given point on the planned step.
fn faulted_repo(base: &TempDir, points: &[FaultPoint]) -> DocumentRepository {
    let mut faulty = FaultyFileOps::new();
    for point in points {
        faulty = faulty.inject(*point);
    }
    DocumentRepository::with_io(real_paths(base), Box::new(faulty))
}

fn corrupt_bytes() -> Vec<u8> {
    br#"{"schema_version":1,"revision":99,"settings"{"truncated""#.to_vec()
}

// ---------------------------------------------------------------------------
// location.rs
// ---------------------------------------------------------------------------

#[test]
fn paths_resolve_same_dir_with_expected_names() {
    let base = TempDir::new().expect("temp dir");
    let paths = real_paths(&base);

    assert_eq!(paths.main(), main_path(base.path()));
    assert_eq!(paths.backup(), backup_path(base.path()));
    assert_eq!(paths.temp("t1"), temp_path(base.path(), "t1"));

    assert_eq!(
        paths.main().file_name().unwrap().to_str().unwrap(),
        "data.json"
    );
    assert_eq!(
        paths.backup().file_name().unwrap().to_str().unwrap(),
        "data.json.bak"
    );
    assert_eq!(
        paths.temp("abc").file_name().unwrap().to_str().unwrap(),
        "data.json.tmp.abc"
    );

    let main = paths.main();
    let backup = paths.backup();
    let temp = paths.temp("x");
    for path in [&main, &backup, &temp] {
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            base.path().file_name().unwrap(),
            "all document files must resolve into the base dir"
        );
    }
}

// ---------------------------------------------------------------------------
// save / load happy paths
// ---------------------------------------------------------------------------

#[test]
fn save_success_creates_main_and_temp_is_cleaned() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    let document = fixture_document();

    repo.save(&document).expect("first save must succeed");

    assert!(
        main_path(base.path()).exists(),
        "main must exist after save"
    );
    assert!(
        !backup_path(base.path()).exists(),
        "no backup before a second save"
    );

    let decoded =
        codec_mod::decode(&read_raw(&main_path(base.path()))).expect("saved main must decode");
    assert_eq!(decoded, document);
}

#[test]
fn save_round_trip_preserves_data_and_unicode() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    repo.save(&fixture_document()).expect("save must succeed");
    let decoded = codec_mod::decode(&read_raw(&main_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.folders[0].display_name, "项目 文档 🗂️");
    assert!(decoded.data.folders[0].path.contains("机密 文件夹"));
}

#[test]
fn backup_from_previous_save_holds_first_revision_bytes() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);

    let first = fixture_with_revision(2);
    // Seeding with a raw file matching what save would have produced proves the
    // backup carries the exact previous committed bytes.
    let first_bytes = codec_mod::encode(&first).expect("fixture must encode");
    write_raw(&main_path(base.path()), &first_bytes);

    let second = fixture_with_revision(4);
    repo.save(&second).expect("second save must succeed");

    assert!(backup_path(base.path()).exists(), "backup must exist");
    let backup_bytes = read_raw(&backup_path(base.path()));
    assert_eq!(backup_bytes, first_bytes, "backup must equal previous main");
    let backup_decoded = codec_mod::decode(&backup_bytes).expect("backup must decode");
    assert_eq!(backup_decoded.data.revision, 2);
}

#[test]
fn load_found_after_save_round_trips() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    let document = fixture_with_revision(3);
    repo.save(&document).expect("save must succeed");

    let mut fresh = real_repo(&base);
    let outcome = fresh.load().expect("load must succeed");
    match outcome {
        LoadOutcome::Found(loaded) => assert_eq!(loaded, document),
        other => panic!("expected Found, got {other:?}"),
    }
    assert_eq!(fresh.latest_loaded_revision(), Some(3));
}

// ---------------------------------------------------------------------------
// load failure modes
// ---------------------------------------------------------------------------

#[test]
fn load_missing_main_is_not_found_without_silent_default() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);

    assert_eq!(
        repo.load().expect_err("missing main must fail"),
        RepositoryError::NotFound
    );
}

#[test]
fn load_corrupt_main_with_valid_backup_recovers_and_preserves_main() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    let document = fixture_with_revision(6);
    repo.save(&document).expect("seed save must succeed");

    // Seed a backup holding the committed bytes, then corrupt the main file.
    let committed = read_raw(&main_path(base.path()));
    write_raw(&backup_path(base.path()), &committed);
    write_raw(&main_path(base.path()), &corrupt_bytes());

    let mut repo = real_repo(&base);
    let outcome = repo.load().expect("recovery must succeed");
    match outcome {
        LoadOutcome::Recovered(loaded) => {
            assert_eq!(
                loaded, document,
                "recovered doc must equal the committed one"
            );
        }
        other => panic!("expected Recovered, got {other:?}"),
    }
    let main_after = read_raw(&main_path(base.path()));
    assert_eq!(
        main_after,
        corrupt_bytes(),
        "corrupt main must be byte-preserved on disk after recovery"
    );
    assert_eq!(repo.latest_loaded_revision(), Some(6));
}

#[test]
fn load_corrupt_main_no_or_invalid_backup_returns_corrupt_data_and_preserves_main() {
    let base = TempDir::new().expect("temp dir");

    // No backup at all.
    write_raw(&main_path(base.path()), &corrupt_bytes());
    let mut repo = real_repo(&base);
    assert_eq!(
        repo.load().expect_err("no backup must fail"),
        RepositoryError::CorruptData
    );
    assert_eq!(
        read_raw(&main_path(base.path())),
        corrupt_bytes(),
        "corrupt main must be preserved when no backup exists"
    );
    drop(repo);

    // Backup exists but is invalid too.
    write_raw(&backup_path(base.path()), b"{\"not json\"");
    let mut repo = real_repo(&base);
    assert_eq!(
        repo.load().expect_err("invalid backup must fail"),
        RepositoryError::CorruptData
    );
    assert_eq!(
        read_raw(&main_path(base.path())),
        corrupt_bytes(),
        "corrupt main must still be preserved"
    );
}

#[test]
fn load_main_missing_with_valid_backup_recovers_from_backup() {
    let base = TempDir::new().expect("temp dir");
    // No main file at all, only a valid backup.
    let document = fixture_with_revision(5);
    write_raw(
        &backup_path(base.path()),
        &codec_mod::encode(&document).expect("encode"),
    );
    let mut repo = real_repo(&base);
    let outcome = repo.load().expect("recovery from backup must succeed");
    match outcome {
        LoadOutcome::Recovered(loaded) => assert_eq!(loaded, document),
        other => panic!("expected Recovered, got {other:?}"),
    }
    assert_eq!(repo.latest_loaded_revision(), Some(5));
    // NotFound remains reserved for the truly empty directory.
    let empty = TempDir::new().expect("temp dir");
    let mut empty_repo = real_repo(&empty);
    assert_eq!(
        empty_repo.load().expect_err("empty dir must be NotFound"),
        RepositoryError::NotFound
    );
}

// ---------------------------------------------------------------------------
// save fault matrix
// ---------------------------------------------------------------------------

#[test]
fn save_fault_temp_write_error_leaves_no_main_or_backup() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_document();
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 0,
            op: FaultOp::TempWrite,
        }],
    );

    assert_eq!(
        repo.save(&document)
            .expect_err("temp write fail must error"),
        RepositoryError::Io
    );
    assert!(!main_path(base.path()).exists(), "main must not be created");
    assert!(
        !backup_path(base.path()).exists(),
        "backup must not be created"
    );
    // A partial temp file may be left — that is safe and cleaned on next save.
}

#[test]
fn save_fault_temp_sync_error_returns_and_preserves_previous_main() {
    let base = TempDir::new().expect("temp dir");
    let first = fixture_with_revision(1);

    let mut repo = real_repo(&base);
    repo.save(&first).expect("seed save must succeed");
    let previous_main = read_raw(&main_path(base.path()));

    let second = fixture_with_revision(2);
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 1,
            op: FaultOp::TempSync,
        }],
    );
    assert_eq!(
        repo.save(&second).expect_err("temp sync fail must error"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_path(base.path())),
        previous_main,
        "original main must survive a failed sync"
    );
    assert!(
        !backup_path(base.path()).exists(),
        "no backup should be written on fail"
    );
}

#[test]
fn save_fault_backup_copy_error_preserves_main_and_backup() {
    let base = TempDir::new().expect("temp dir");
    let first = fixture_with_revision(1);
    {
        let mut repo = real_repo(&base);
        repo.save(&first).expect("seed save must succeed");
    }
    let previous_main = read_raw(&main_path(base.path()));
    // Seed a backup that exists already.
    write_raw(&backup_path(base.path()), b"{\"older\"}");

    let second = fixture_with_revision(2);
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 3,
            op: FaultOp::BackupCopy,
        }],
    );
    assert_eq!(
        repo.save(&second).expect_err("backup copy fail must error"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_path(base.path())),
        previous_main,
        "main must be untouched on backup-copy failure"
    );
    assert_eq!(
        read_raw(&backup_path(base.path())),
        b"{\"older\"}",
        "pre-existing backup must be untouched"
    );
}

#[test]
fn save_fault_rename_error_preserves_main_and_backup() {
    let base = TempDir::new().expect("temp dir");
    let first = fixture_with_revision(1);
    {
        let mut repo = real_repo(&base);
        repo.save(&first).expect("seed save must succeed");
    }
    let previous_main = read_raw(&main_path(base.path()));

    let second = fixture_with_revision(2);
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 4,
            op: FaultOp::ReplaceRename,
        }],
    );
    assert_eq!(
        repo.save(&second).expect_err("rename fail must error"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_path(base.path())),
        previous_main,
        "main must be untouched on rename failure"
    );
    assert!(
        backup_path(base.path()).exists(),
        "backup from the first save must remain readable"
    );
}

#[test]
fn save_invalid_data_returns_invalid_data_without_touching_disk() {
    let base = TempDir::new().expect("temp dir");
    let mut invalid = fixture_document();
    invalid.data.revision = 0; // invalid per codec validation
    let mut repo = real_repo(&base);

    assert_eq!(
        repo.save(&invalid)
            .expect_err("invalid revision must be rejected"),
        RepositoryError::InvalidData
    );
    assert!(!main_path(base.path()).exists(), "nothing may be written");
    assert!(!backup_path(base.path()).exists());
}

// ---------------------------------------------------------------------------
// revision guard
// ---------------------------------------------------------------------------

#[test]
fn revision_guard_rejects_stale_expected_and_accepts_matching() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(10);
    {
        let mut repo = real_repo(&base);
        repo.save(&document).expect("seed save must succeed");
    }

    let mut repo = real_repo(&base);
    repo.load().expect("load must succeed");

    // Stale expected revision → ConcurrentModification, nothing overwritten.
    let stale = fixture_with_revision(11);
    assert_eq!(
        repo.save_if_current(&stale, 9)
            .expect_err("stale expected must be rejected"),
        RepositoryError::ConcurrentModification
    );
    let main_bytes = read_raw(&main_path(base.path()));
    let decoded = codec_mod::decode(&main_bytes).expect("main must still be previous state");
    assert_eq!(decoded.data.revision, 10, "main must not be overwritten");

    // Matching expected revision → accepted and latest_loaded_revision advances.
    let next = fixture_with_revision(11);
    repo.save_if_current(&next, 10)
        .expect("matching guard must pass");
    assert_eq!(repo.latest_loaded_revision(), Some(11));
    let decoded = codec_mod::decode(&read_raw(&main_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 11);
}

#[test]
fn revision_guard_is_not_enforced_before_any_load_or_save() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    let document = fixture_with_revision(3);

    // No baseline yet: save_if_current proceeds regardless of expected value.
    repo.save_if_current(&document, 999)
        .expect("no baseline means no guard");
    let decoded = codec_mod::decode(&read_raw(&main_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 3);
}

// ---------------------------------------------------------------------------
// remove_folder (deletion invariant)
// ---------------------------------------------------------------------------

#[test]
fn remove_folder_removes_record_and_keeps_real_folder_on_disk() {
    let base = TempDir::new().expect("temp dir");
    let mut document = fixture_document();
    let folder_id = document.data.folders[0].id;
    document.data.revision = 7;
    let real_subdir = base.path().join("real-folder-that-stays");
    std::fs::create_dir(&real_subdir).expect("test helper real dir");
    document.data.folders[0].path = real_subdir.to_string_lossy().into_owned();

    let mut repo = real_repo(&base);
    repo.save(&document).expect("seed save must succeed");
    assert_eq!(repo.folder_count(), 1);

    let mut repo = real_repo(&base);
    repo.load().expect("load must succeed");
    let removed = repo.remove_folder(folder_id);
    assert!(removed, "record must be removed");
    assert_eq!(repo.folder_count(), 0);
    assert!(
        !repo.remove_folder(folder_id),
        "second removal must be a no-op"
    );

    let snapshot = repo.document().expect("loaded working copy").clone();
    repo.save(&snapshot).expect("persist after removal");

    // Reload from disk: record gone, real folder untouched.
    let mut fresh = real_repo(&base);
    let outcome = fresh.load().expect("load must succeed");
    match outcome {
        LoadOutcome::Found(loaded) => assert!(loaded.data.folders.is_empty()),
        other => panic!("expected Found, got {other:?}"),
    }
    assert!(
        real_subdir.exists(),
        "real folder must still physically exist on disk"
    );
}

// ---------------------------------------------------------------------------
// stale temp cleanup
// ---------------------------------------------------------------------------

#[test]
fn stale_temp_siblings_are_removed_after_successful_save() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);

    let stale_one = temp_path(base.path(), "stale-1");
    let stale_two = temp_path(base.path(), "stale-2");
    write_raw(&stale_one, b"leftover");
    write_raw(&stale_two, b"leftover");

    repo.save(&fixture_document()).expect("save must succeed");

    assert!(!stale_one.exists(), "stale temp must be cleaned");
    assert!(!stale_two.exists(), "stale temp must be cleaned");
    assert!(main_path(base.path()).exists(), "main must exist");
    // The current save temp must not remain behind either (renamed to main).
    let leftovers = std::fs::read_dir(base.path())
        .expect("dir readable")
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("data.json.tmp."))
        })
        .count();
    assert_eq!(
        leftovers, 0,
        "no temp files may remain after a successful save"
    );
}

// ---------------------------------------------------------------------------
// error Display / output hygiene
// ---------------------------------------------------------------------------

#[test]
fn repository_error_display_contains_no_stored_content() {
    let sensitive = "DO-NOT-LEAK-JSON-CONTENT";
    let display = RepositoryError::Io.to_string();
    assert!(!display.contains(sensitive));
    assert!(!display.contains("data.json") && !display.contains("BaseDir"));
    let display = RepositoryError::CorruptData.to_string();
    assert!(!display.contains(sensitive));
}
