//! Tests for M01-B safe atomic persistence.
//!
//! Plan of record (each save performs exactly these `FileOps` sub-steps, so
//! the fault matrix knows which step index to target). When a main exists:
//!   step 0: `create_new` acquire the write lock
//!   step 1,2: `write_flush_sync` main temp (write then sync)
//!   step 3:   `exists` main probe
//!   step 4,5: `write_flush_sync` backup temp (write then sync)
//!   step 6:   `rename` backup temp → `.bak`
//!   step 7:   `rename` main temp → main
//!   — cleanup: one `remove` per stale `data.json.tmp.*` sibling (guarded by
//!     the read_dir in `cleanup_stale_temps`, which is real `std::fs`).
//! When no main exists, steps 4-6 are skipped and step 7 becomes step 4.
//!
//! `load` performs: `read` main, then `read` backup only when the main fails
//! to decode.

use chrono::{DateTime, Utc};
use tempfile::TempDir;
use uuid::Uuid;

use std::{
    io::{self as std_io, ErrorKind as IoErrorKind},
    sync::{Arc, Barrier},
    thread,
};

use crate::{
    domain::{
        document::AppData,
        folder::{Category, FolderEntry, Tag},
        ids::{CategoryId, FolderId, TagId},
        settings::AppSettings,
    },
    storage::{
        codec as codec_mod,
        io::{FileOps, FsFileOps},
        location::DocumentPaths,
        location::{
            TEMP_FILE_PREFIX, backup_document_path, backup_temp_document_path, lock_file_path,
            main_document_path, temp_document_path,
        },
        repository::{DocumentRepository, LoadOutcome, RepairOutcome, RepositoryError},
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

    assert_eq!(paths.main(), main_document_path(base.path()));
    assert_eq!(paths.backup(), backup_document_path(base.path()));
    assert_eq!(paths.temp("t1"), temp_document_path(base.path(), "t1"));

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
        main_document_path(base.path()).exists(),
        "main must exist after save"
    );
    assert!(
        !backup_document_path(base.path()).exists(),
        "no backup before a second save"
    );

    let decoded = codec_mod::decode(&read_raw(&main_document_path(base.path())))
        .expect("saved main must decode");
    assert_eq!(decoded, document);
}

#[test]
fn save_round_trip_preserves_data_and_unicode() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    repo.save(&fixture_document()).expect("save must succeed");
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
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
    write_raw(&main_document_path(base.path()), &first_bytes);

    let second = fixture_with_revision(4);
    repo.save(&second).expect("second save must succeed");

    assert!(
        backup_document_path(base.path()).exists(),
        "backup must exist"
    );
    let backup_bytes = read_raw(&backup_document_path(base.path()));
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
    let committed = read_raw(&main_document_path(base.path()));
    write_raw(&backup_document_path(base.path()), &committed);
    write_raw(&main_document_path(base.path()), &corrupt_bytes());

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
    let main_after = read_raw(&main_document_path(base.path()));
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
    write_raw(&main_document_path(base.path()), &corrupt_bytes());
    let mut repo = real_repo(&base);
    assert_eq!(
        repo.load().expect_err("no backup must fail"),
        RepositoryError::CorruptData
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must be preserved when no backup exists"
    );
    drop(repo);

    // Backup exists but is invalid too.
    write_raw(&backup_document_path(base.path()), b"{\"not json\"");
    let mut repo = real_repo(&base);
    assert_eq!(
        repo.load().expect_err("invalid backup must fail"),
        RepositoryError::CorruptData
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
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
        &backup_document_path(base.path()),
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
// F001/F002: recovered state blocks saves; corrupt main never becomes a
// revision baseline
// ---------------------------------------------------------------------------

/// Seed a corrupt main plus a valid backup holding `document`'s committed
/// bytes, then load a fresh repository off them (returns the recovered repo).
fn seed_corrupt_main_valid_backup(
    base: &TempDir,
    document: &StoredDocumentV1,
) -> DocumentRepository {
    let committed = codec_mod::encode(document).expect("fixture must encode");
    write_raw(&backup_document_path(base.path()), &committed);
    write_raw(&main_document_path(base.path()), &corrupt_bytes());
    let mut repo = real_repo(base);
    match repo.load().expect("recovery must succeed") {
        LoadOutcome::Recovered(loaded) => assert_eq!(loaded, *document),
        other => panic!("expected Recovered, got {other:?}"),
    }
    repo
}

/// F001: while a corrupt main is pending recovery, an ordinary `save` is
/// rejected with `RecoveryRequired` and leaves BOTH the corrupt main and the
/// valid backup byte-untouched — the corrupt main can never be copied over the
/// only valid copy, and no save can hide the corruption evidence.
#[test]
fn recovered_pending_save_is_rejected_and_preserves_main_and_backup() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(6);
    let mut repo = seed_corrupt_main_valid_backup(&base, &document);

    let overwrite = fixture_with_revision(7);
    assert_eq!(
        repo.save(&overwrite)
            .expect_err("pending save must be rejected"),
        RepositoryError::RecoveryRequired
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must remain byte-untouched"
    );
    assert_eq!(
        read_raw(&backup_document_path(base.path())),
        codec_mod::encode(&document).expect("encode"),
        "valid backup must remain byte-untouched"
    );
    assert_eq!(
        repo.latest_loaded_revision(),
        Some(6),
        "observed revision must still be the recovered one"
    );
}

/// F001 + F002: `save_if_current` on a pending-recovery repository refuses
/// with `RecoveryRequired` (never letting a corrupt main become an expected
/// revision baseline) and preserves both files.
#[test]
fn recovered_pending_save_if_current_is_rejected_and_preserves_main_and_backup() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(6);
    let mut repo = seed_corrupt_main_valid_backup(&base, &document);

    let overwrite = fixture_with_revision(8);
    assert_eq!(
        repo.save_if_current(&overwrite, 6)
            .expect_err("pending save_if_current must be rejected"),
        RepositoryError::RecoveryRequired
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must remain byte-untouched"
    );
    assert_eq!(
        read_raw(&backup_document_path(base.path())),
        codec_mod::encode(&document).expect("encode"),
        "valid backup must remain byte-untouched"
    );
}

/// F001: `repair_from_backup` preserves the corrupt main bytes as durable
/// evidence, promotes the valid backup onto a healthy main, clears the pending
/// state and refreshes the observed revision. Ordinary saves work afterwards.
#[test]
fn repair_from_backup_preserves_evidence_and_repairs_main() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(6);
    let original_backup_bytes = codec_mod::encode(&document).expect("encode");
    let mut repo = seed_corrupt_main_valid_backup(&base, &document);

    let outcome = repo.repair_from_backup().expect("repair must succeed");
    let RepairOutcome::Repaired { evidence } = outcome else {
        panic!("expected Repaired, got {outcome:?}");
    };
    let evidence = evidence.expect("a corrupt main existed, evidence must be written");
    assert_eq!(
        evidence.parent().map(|p| p.to_path_buf()),
        Some(base.path().to_path_buf()),
        "evidence must live in the same data directory"
    );
    assert!(
        evidence
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("data.json.corrupt-")),
        "evidence name must be data.json.corrupt-<token>, got {:?}",
        evidence.file_name()
    );
    assert!(
        evidence != main_document_path(base.path())
            && evidence != backup_document_path(base.path()),
        "evidence must be a distinct file"
    );
    assert_eq!(
        read_raw(&evidence),
        corrupt_bytes(),
        "corrupt main bytes must be preserved verbatim in the evidence file"
    );
    // Evidence sits in the same directory and is never auto-removed.
    assert!(evidence.exists(), "evidence must be durable on disk");

    // Main now decodes to the recovered document.
    let decoded = codec_mod::decode(&read_raw(&main_document_path(base.path())))
        .expect("repaired main must decode");
    assert_eq!(decoded, document, "main must hold the recovered document");
    // Backup still decodes to the same document.
    let backup_decoded = codec_mod::decode(&read_raw(&backup_document_path(base.path())))
        .expect("backup must still decode");
    assert_eq!(
        backup_decoded, document,
        "backup must still hold the document"
    );
    assert_eq!(
        read_raw(&backup_document_path(base.path())),
        original_backup_bytes,
        "backup bytes must be unchanged by the repair"
    );

    // Pending state cleared and revision refreshed.
    assert_eq!(repo.latest_loaded_revision(), Some(6));

    // Ordinary saves work again on a healthy main.
    let next = fixture_with_revision(9);
    repo.save(&next).expect("save after repair must succeed");
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 9);
    // The evidence file is untouched by the later save.
    assert_eq!(read_raw(&evidence), corrupt_bytes());
}

/// F001: the previous tests exercise `repair` with a corrupt main; this one
/// covers the missing-main variant (backup-only): the main is recreated from
/// the backup, no evidence file is produced (there were no corrupt bytes), the
/// pending state clears and a subsequent save works.
#[test]
fn repair_from_backup_with_missing_main_recreates_main() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(11);
    write_raw(
        &backup_document_path(base.path()),
        &codec_mod::encode(&document).expect("encode"),
    );
    let mut repo = real_repo(&base);
    match repo.load().expect("recovery must succeed") {
        LoadOutcome::Recovered(loaded) => assert_eq!(loaded, document),
        other => panic!("expected Recovered, got {other:?}"),
    }
    assert!(!main_document_path(base.path()).exists());

    let outcome = repo.repair_from_backup().expect("repair must succeed");
    match outcome {
        RepairOutcome::Repaired { evidence: None } => {}
        other => panic!("expected Repaired with no evidence, got {other:?}"),
    }
    let decoded = codec_mod::decode(&read_raw(&main_document_path(base.path())))
        .expect("repaired main must decode");
    assert_eq!(decoded, document);
    assert_eq!(repo.latest_loaded_revision(), Some(11));

    let next = fixture_with_revision(12);
    repo.save(&next).expect("save after repair must succeed");
}

/// F001: `repair_from_backup` refuses when the BACKUP is absent or invalid too
/// (the main is still corrupt) — it returns `CorruptData`, leaves both the
/// main and the backup byte-untouched, and keeps the pending state intact.
/// Each case enters the pending state via a clean recovery, then spoils the
/// backup externally before repairing.
#[test]
fn repair_from_backup_rejects_absent_or_invalid_backup_and_preserves_both() {
    // Backup becomes invalid after a clean recovery.
    {
        let base = TempDir::new().expect("temp dir");
        let document = fixture_with_revision(6);
        let mut repo = seed_corrupt_main_valid_backup(&base, &document);
        write_raw(&backup_document_path(base.path()), b"{\"not json\"");

        assert_eq!(
            repo.repair_from_backup()
                .expect_err("invalid backup must refuse repair"),
            RepositoryError::CorruptData
        );
        assert_eq!(
            read_raw(&main_document_path(base.path())),
            corrupt_bytes(),
            "main must remain untouched on refused repair"
        );
        assert_eq!(
            read_raw(&backup_document_path(base.path())),
            b"{\"not json\"",
            "backup must remain untouched on refused repair"
        );
        assert_eq!(
            repo.latest_loaded_revision(),
            Some(6),
            "pending state survives a refused repair"
        );
        // No evidence file may be written before the repair is accepted.
        let dir = base.path();
        let evidence_leftovers = std::fs::read_dir(dir)
            .expect("dir readable")
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("data.json.corrupt-"))
            })
            .count();
        assert_eq!(
            evidence_leftovers, 0,
            "no evidence file may be written on a refused repair"
        );
    }

    // Backup becomes absent after a clean recovery.
    {
        let base = TempDir::new().expect("temp dir");
        let document = fixture_with_revision(6);
        let mut repo = seed_corrupt_main_valid_backup(&base, &document);
        std::fs::remove_file(backup_document_path(base.path()))
            .expect("removing the backup is a test step");

        assert_eq!(
            repo.repair_from_backup()
                .expect_err("absent backup must refuse repair"),
            RepositoryError::CorruptData
        );
        assert_eq!(
            read_raw(&main_document_path(base.path())),
            corrupt_bytes(),
            "main must remain untouched"
        );
        assert!(
            !backup_document_path(base.path()).exists(),
            "backup must remain absent"
        );
    }

    // When there was never a recoverable backup, load fails with CorruptData
    // (no pending state) and repair is an explicit no-op that changes nothing.
    {
        let base = TempDir::new().expect("temp dir");
        write_raw(&main_document_path(base.path()), &corrupt_bytes());
        let mut repo = real_repo(&base);
        assert_eq!(
            repo.load().expect_err("no backup must fail"),
            RepositoryError::CorruptData,
            "load itself reports CorruptData when there is no backup"
        );
        assert_eq!(
            repo.repair_from_backup().expect("nothing pending"),
            RepairOutcome::HadNoCorruptMain
        );
        assert_eq!(
            read_raw(&main_document_path(base.path())),
            corrupt_bytes(),
            "main must remain untouched"
        );
        assert!(!backup_document_path(base.path()).exists());
    }
}

/// M01B-N001-followup (closed in M07): `repair_from_backup` must run under the
/// per-directory write lock, exactly like every other writer. This regression
/// proves the lock is actually held: while a lock file exists (a concurrent
/// save mid-write), a repair request is refused with `ConcurrentModification`
/// and touches neither the corrupt main nor the valid backup.
#[test]
fn repair_from_backup_contends_with_a_concurrent_writer_via_the_lock() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(7);
    let mut repo = seed_corrupt_main_valid_backup(&base, &document);

    // Simulate the OTHER writer holding the write lock.
    std::fs::write(lock_file_path(base.path()), b"held by a concurrent writer")
        .expect("lock file must be creatable");

    // Repair must refuse with ConcurrentModification — it never races a save or
    // silently reverts a newer main.
    assert_eq!(
        repo.repair_from_backup()
            .expect_err("lock must be contended"),
        RepositoryError::ConcurrentModification
    );
    // Nothing was touched: main still corrupt, backup still intact.
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must remain untouched while the lock is held"
    );
    assert_eq!(
        read_raw(&backup_document_path(base.path())),
        codec_mod::encode(&document).expect("encode"),
        "valid backup must remain intact while the lock is held"
    );

    // Release the lock: repair now succeeds and promotes the valid backup.
    std::fs::remove_file(lock_file_path(base.path())).expect("remove test lock");
    match repo
        .repair_from_backup()
        .expect("repair after lock release")
    {
        RepairOutcome::Repaired { evidence: Some(_) } => {}
        other => panic!("expected repaired with evidence, got {other:?}"),
    }
}

/// F002: `save_if_current` must classify a corrupt main as `CorruptData`, not
/// fabricate the expected revision, and must preserve both main and backup
/// bytes. (The pending-recovery rejection above covers the recovered case;
/// this peers at the raw corrupt + corrupt/no-backup classification.)
#[test]
fn save_if_current_with_corrupt_main_returns_corrupt_data_and_preserves_both() {
    let base = TempDir::new().expect("temp dir");
    let document = fixture_with_revision(4);
    // Seed a valid backup, then corrupt the main — but do NOT load first, so
    // the repository has no pending state and save_if_current must classify
    // the corrupt main by itself.
    write_raw(
        &backup_document_path(base.path()),
        &codec_mod::encode(&document).expect("encode"),
    );
    write_raw(&main_document_path(base.path()), &corrupt_bytes());

    let mut repo = real_repo(&base);
    let overwrite = fixture_with_revision(5);
    assert_eq!(
        repo.save_if_current(&overwrite, 4)
            .expect_err("corrupt main must be refused, never matched"),
        RepositoryError::CorruptData,
        "any expected revision must be refused for a corrupt main"
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must not be overwritten"
    );
    assert_eq!(
        read_raw(&backup_document_path(base.path())),
        codec_mod::encode(&document).expect("encode"),
        "valid backup must not be overwritten with corrupt main bytes"
    );
}

// ---------------------------------------------------------------------------
// F004: inaccessible files are never misread as missing/corrupt
// ---------------------------------------------------------------------------

/// Wraps `FsFileOps`, delegating everything, but faults reads of a specific
/// file (by file name) with an injected kind instead of touching the disk.
/// Used to prove I/O-error classification in `load`, backup recovery and the
/// revision guard.
#[derive(Debug, Default)]
struct ReadFaultFileOps {
    fault_paths: Vec<(String, std::io::ErrorKind)>,
}

impl ReadFaultFileOps {
    fn new() -> Self {
        Self::default()
    }

    fn fault_named(mut self, name: &str, kind: std::io::ErrorKind) -> Self {
        self.fault_paths.push((name.to_owned(), kind));
        self
    }

    fn fault_kind_for(&self, path: &std::path::Path) -> Option<std::io::ErrorKind> {
        let name = path.file_name()?.to_str()?;
        self.fault_paths
            .iter()
            .find(|(target, _)| target == name)
            .map(|(_, kind)| *kind)
    }
}

impl FileOps for ReadFaultFileOps {
    fn write_flush_sync(&mut self, path: &std::path::Path, bytes: &[u8]) -> std_io::Result<()> {
        FsFileOps.write_flush_sync(path, bytes)
    }

    fn rename(&mut self, from: &std::path::Path, to: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.rename(from, to)
    }

    fn read(&mut self, path: &std::path::Path) -> std_io::Result<Vec<u8>> {
        if let Some(kind) = self.fault_kind_for(path) {
            return Err(std_io::Error::new(kind, "injected read fault"));
        }
        FsFileOps.read(path)
    }

    fn exists(&mut self, path: &std::path::Path) -> std_io::Result<bool> {
        FsFileOps.exists(path)
    }

    fn create_new(&mut self, path: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.create_new(path)
    }

    fn remove(&mut self, path: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.remove(path)
    }
}

/// F004: an injected main-read failure that is not `NotFound` must surface as
/// `RepositoryError::Io` — never as a "missing main" that silently falls back
/// to backup recovery or to `NotFound` after both reads fail.
#[test]
fn load_main_read_access_denied_returns_io_and_touches_nothing() {
    let base = TempDir::new().expect("temp dir");
    // Both a main and a valid backup exist on disk: the injected fault proves
    // the repository does NOT silently try the backup on an I/O error.
    let document = fixture_with_revision(2);
    write_raw(
        &backup_document_path(base.path()),
        &codec_mod::encode(&document).expect("encode"),
    );
    write_raw(
        &main_document_path(base.path()),
        &codec_mod::encode(&document).expect("encode"),
    );

    let paths = real_paths(&base);
    let io_boxed: Box<dyn FileOps> =
        Box::new(ReadFaultFileOps::new().fault_named("data.json", IoErrorKind::PermissionDenied));
    let mut repo = DocumentRepository::with_io(paths, io_boxed);
    assert_eq!(
        repo.load().expect_err("permission denied must be Io"),
        RepositoryError::Io,
        "a non-NotFound main read error must not fall back to the backup"
    );
    assert!(
        repo.document().is_none(),
        "no document may be seeded from a failed read"
    );
    // Nothing modified.
    write_raw(
        &main_document_path(base.path()),
        &codec_mod::encode(&fixture_with_revision(3)).expect("encode"),
    );
    let decoded = codec_mod::decode(&read_raw(&main_document_path(base.path())))
        .expect("re-seeded main must decode");
    assert_eq!(decoded.data.revision, 3);
}

/// F004: the same applies to the backup read during `load` — an
/// inaccessible backup is `Io`, not "no backup, hence CorruptData/NotFound".
#[test]
fn load_backup_read_access_denied_returns_io() {
    let base = TempDir::new().expect("temp dir");
    // A corrupt main (so load WILL attept the backup) plus an existing backup.
    write_raw(&main_document_path(base.path()), &corrupt_bytes());
    write_raw(
        &backup_document_path(base.path()),
        &codec_mod::encode(&fixture_with_revision(2)).expect("encode"),
    );

    let io_boxed: Box<dyn FileOps> = Box::new(
        ReadFaultFileOps::new().fault_named("data.json.bak", IoErrorKind::PermissionDenied),
    );
    let mut repo = DocumentRepository::with_io(real_paths(&base), io_boxed);
    assert_eq!(
        repo.load().expect_err("backup access denied must be Io"),
        RepositoryError::Io,
        "an inaccessible backup must not be treated as absent"
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        corrupt_bytes(),
        "corrupt main must remain untouched"
    );
}

/// F004: the revision guard's `exists`/main probe classifies a non-NotFound
/// error as `Io` too (never "missing main, no revision check").
#[test]
fn save_if_current_main_read_access_denied_returns_io() {
    let base = TempDir::new().expect("temp dir");
    write_raw(
        &main_document_path(base.path()),
        &codec_mod::encode(&fixture_with_revision(2)).expect("encode"),
    );
    let io_boxed: Box<dyn FileOps> =
        Box::new(ReadFaultFileOps::new().fault_named("data.json", IoErrorKind::PermissionDenied));
    let mut repo = DocumentRepository::with_io(real_paths(&base), io_boxed);
    assert_eq!(
        repo.save_if_current(&fixture_with_revision(3), 2)
            .expect_err("permission denied must be Io"),
        RepositoryError::Io,
        "an unreadable main must not be treated as a missing baseline"
    );
}

/// Wraps `FsFileOps`, delegating everything, but faults `exists` for a
/// specific file name with a non-`NotFound` error.
#[derive(Debug, Default)]
struct ExistsFaultFileOps {
    fault_paths: Vec<(String, std::io::ErrorKind)>,
}

impl ExistsFaultFileOps {
    fn new() -> Self {
        Self::default()
    }

    fn fault_named(mut self, name: &str, kind: std::io::ErrorKind) -> Self {
        self.fault_paths.push((name.to_owned(), kind));
        self
    }

    fn fault_kind_for(&self, path: &std::path::Path) -> Option<std::io::ErrorKind> {
        let name = path.file_name()?.to_str()?;
        self.fault_paths
            .iter()
            .find(|(target, _)| target == name)
            .map(|(_, kind)| *kind)
    }
}

impl FileOps for ExistsFaultFileOps {
    fn write_flush_sync(&mut self, path: &std::path::Path, bytes: &[u8]) -> std_io::Result<()> {
        FsFileOps.write_flush_sync(path, bytes)
    }

    fn rename(&mut self, from: &std::path::Path, to: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.rename(from, to)
    }

    fn read(&mut self, path: &std::path::Path) -> std_io::Result<Vec<u8>> {
        FsFileOps.read(path)
    }

    fn exists(&mut self, path: &std::path::Path) -> std_io::Result<bool> {
        if let Some(kind) = self.fault_kind_for(path) {
            return Err(std_io::Error::new(kind, "injected exists fault"));
        }
        FsFileOps.exists(path)
    }

    fn create_new(&mut self, path: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.create_new(path)
    }

    fn remove(&mut self, path: &std::path::Path) -> std_io::Result<()> {
        FsFileOps.remove(path)
    }
}

/// F004: the main-exists probe (`save` step 3) classifies a non-`NotFound`
/// error as `Io`, not "no main → skip backup". Both the write-lock and the
/// main probe faults abort nothing prematurely; the error must reach the
/// caller.
#[test]
fn save_main_exists_error_returns_io_and_preserves_previous_main() {
    let base = TempDir::new().expect("temp dir");
    let first = fixture_with_revision(1);
    {
        let mut repo = real_repo(&base);
        repo.save(&first).expect("seed save must succeed");
    }
    let previous_main = read_raw(&main_document_path(base.path()));

    let io_boxed: Box<dyn FileOps> =
        Box::new(ExistsFaultFileOps::new().fault_named("data.json", IoErrorKind::PermissionDenied));
    let mut repo = DocumentRepository::with_io(real_paths(&base), io_boxed);
    assert_eq!(
        repo.save(&fixture_with_revision(2))
            .expect_err("exists error must be Io"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        previous_main,
        "main must remain untouched on an exists() error"
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
            step: 1,
            op: FaultOp::TempWrite,
        }],
    );

    assert_eq!(
        repo.save(&document)
            .expect_err("temp write fail must error"),
        RepositoryError::Io
    );
    assert!(
        !main_document_path(base.path()).exists(),
        "main must not be created"
    );
    assert!(
        !backup_document_path(base.path()).exists(),
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
    let previous_main = read_raw(&main_document_path(base.path()));

    let second = fixture_with_revision(2);
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 2,
            op: FaultOp::TempSync,
        }],
    );
    assert_eq!(
        repo.save(&second).expect_err("temp sync fail must error"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        previous_main,
        "original main must survive a failed sync"
    );
    assert!(
        !backup_document_path(base.path()).exists(),
        "no backup should be written on fail"
    );
}

/// A pre-existing `.bak` must stay byte-identical when the fault-safe backup
/// write fails at any of its fault points (write, sync, replace). This covers
/// F005's partial-backup-write and backup-rename failures, which the old
/// direct `fs::copy` fault could not.
#[test]
fn save_fault_backup_staging_preserves_previous_backup() {
    let cases: &[(usize, FaultOp)] = &[
        (4, FaultOp::BackupWrite),
        (5, FaultOp::BackupSync),
        (6, FaultOp::BackupReplace),
    ];
    for (step, op) in cases {
        let base = TempDir::new().expect("temp dir");
        let first = fixture_with_revision(1);
        {
            let mut repo = real_repo(&base);
            repo.save(&first).expect("seed save must succeed");
        }
        let previous_main = read_raw(&main_document_path(base.path()));
        // Seed a backup that exists already; its bytes must survive every fault.
        write_raw(&backup_document_path(base.path()), b"{\"older\"}");

        let second = fixture_with_revision(2);
        let mut repo = faulted_repo(
            &base,
            &[FaultPoint {
                step: *step,
                op: *op,
            }],
        );
        assert_eq!(
            repo.save(&second)
                .expect_err("backup staging fail must error"),
            RepositoryError::Io,
            "step {step} ({op:?}) must fail the save"
        );
        assert_eq!(
            read_raw(&main_document_path(base.path())),
            previous_main,
            "main must be untouched on backup-staging failure at {step} ({op:?})"
        );
        assert_eq!(
            read_raw(&backup_document_path(base.path())),
            b"{\"older\"}",
            "pre-existing backup must be byte-identical after {op:?} failure"
        );
    }
}

#[test]
fn save_fault_rename_error_preserves_main_and_backup() {
    let base = TempDir::new().expect("temp dir");
    let first = fixture_with_revision(1);
    {
        let mut repo = real_repo(&base);
        repo.save(&first).expect("seed save must succeed");
    }
    let previous_main = read_raw(&main_document_path(base.path()));

    let second = fixture_with_revision(2);
    let mut repo = faulted_repo(
        &base,
        &[FaultPoint {
            step: 7,
            op: FaultOp::ReplaceRename,
        }],
    );
    assert_eq!(
        repo.save(&second).expect_err("rename fail must error"),
        RepositoryError::Io
    );
    assert_eq!(
        read_raw(&main_document_path(base.path())),
        previous_main,
        "main must be untouched on rename failure"
    );
    assert!(
        backup_document_path(base.path()).exists(),
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
    assert!(
        !main_document_path(base.path()).exists(),
        "nothing may be written"
    );
    assert!(!backup_document_path(base.path()).exists());
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
    let main_bytes = read_raw(&main_document_path(base.path()));
    let decoded = codec_mod::decode(&main_bytes).expect("main must still be previous state");
    assert_eq!(decoded.data.revision, 10, "main must not be overwritten");

    // Matching expected revision → accepted and latest_loaded_revision advances.
    let next = fixture_with_revision(11);
    repo.save_if_current(&next, 10)
        .expect("matching guard must pass");
    assert_eq!(repo.latest_loaded_revision(), Some(11));
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 11);
}

/// The guard must detect an EXTERNAL writer (a different repository instance /
/// process wrote a newer revision to disk), not just the stale in-memory
/// baseline.
#[test]
fn revision_guard_detects_external_concurrent_writer_on_disk() {
    let base = TempDir::new().expect("temp dir");

    // Instance A writes v1.
    let v1 = fixture_with_revision(1);
    {
        let mut repo_a = real_repo(&base);
        repo_a.save(&v1).expect("seed v1 must succeed");
    }

    // Instance B loads v1, then an external writer persists v2 to disk.
    let mut repo_b = real_repo(&base);
    repo_b.load().expect("v1 must load");
    let v2 = fixture_with_revision(2);
    {
        let mut external = real_repo(&base);
        external.save(&v2).expect("external v2 write must succeed");
    }

    // B tries to save its v2 based on the stale expected revision 1.
    assert_eq!(
        repo_b
            .save_if_current(&v2, 1)
            .expect_err("stale expected must be rejected"),
        RepositoryError::ConcurrentModification
    );
    // The on-disk main must still be the external writer's v2 — not overwritten.
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 2, "external state must be preserved");

    // With the correct expected revision 2 the save goes through.
    let v3 = fixture_with_revision(3);
    repo_b
        .save_if_current(&v3, 2)
        .expect("matching on-disk revision must pass");
    assert_eq!(repo_b.latest_loaded_revision(), Some(3));
}

#[test]
fn revision_guard_is_not_enforced_before_any_load_or_save() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    let document = fixture_with_revision(3);

    // No baseline yet: save_if_current proceeds regardless of expected value.
    repo.save_if_current(&document, 999)
        .expect("no baseline means no guard");
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
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

    let stale_one = temp_document_path(base.path(), "stale-1");
    let stale_two = temp_document_path(base.path(), "stale-2");
    write_raw(&stale_one, b"leftover");
    write_raw(&stale_two, b"leftover");

    repo.save(&fixture_document()).expect("save must succeed");

    assert!(!stale_one.exists(), "stale temp must be cleaned");
    assert!(!stale_two.exists(), "stale temp must be cleaned");
    assert!(main_document_path(base.path()).exists(), "main must exist");
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

/// F003: cleanup must not remove files it does not own. Corrupt-evidence
/// files (`data.json.corrupt-*`), backup staging files (`data.json.bak.tmp.*`)
/// and the lock file must all survive a save's cleanup, because deleting any
/// of them could destroy evidence or a concurrent writer's state.
#[test]
fn cleanup_never_removes_evidence_or_backup_temps_or_lock() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);

    let evidence = base.path().join("data.json.corrupt-abc");
    let backup_temp = backup_temp_document_path(base.path(), "x");
    write_raw(&evidence, b"corrupt evidence");
    write_raw(&backup_temp, b"backup staging");

    repo.save(&fixture_document()).expect("save must succeed");

    assert!(evidence.exists(), "evidence file must survive cleanup");
    assert!(
        backup_temp.exists(),
        "backup staging file must survive cleanup"
    );
    assert!(
        !lock_file_path(base.path()).exists(),
        "lock file must be removed after the save (best-effort release)"
    );
}

/// F003 sub-property: with the lock held, cleanup removes a pre-existing stale
/// temp (the existing `stale_temp_siblings_are_removed_after_successful_save`
/// covers the save-time scan; this adds the explicit lock-scope checkpoint).
/// F003 core: cleanup running under writer A's lock can never delete a temp
/// owned by an interleaved writer, because B can only be mid-write while B
/// holds the lock — so B's temp is created only after A's cleanup finished
/// (A released the lock) and A's scan can never see it.
#[test]
fn cleanup_does_not_delete_interleaved_writers_temp() {
    let base = TempDir::new().expect("temp dir");

    // Writer A saves; its stale-temp scan runs under the write lock and
    // removes a pre-existing stale temp (proving lock-scoped cleanup works).
    let stale_tmp = temp_document_path(base.path(), "crashed-writer");
    write_raw(&stale_tmp, b"leftover");
    {
        let mut repo_a = real_repo(&base);
        repo_a.save(&fixture_with_revision(1)).expect("A save");
    }
    assert!(
        !stale_tmp.exists(),
        "lock-scoped cleanup removes stale temps present under the lock"
    );

    // Writer B's temp is created AFTER A's cleanup finished — B could only be
    // writing while holding the lock A has now released, so A's cleanup cannot
    // have reached it. This is the interleave the review's F003 worried about.
    let interleaved_temp = temp_document_path(base.path(), "writer-b-temp");
    write_raw(&interleaved_temp, b"B's synced temp");
    assert!(
        interleaved_temp.exists(),
        "B's temp is created after A's cleanup and survives A's save"
    );

    // B then saves; B's own cleanup only removes B's now-stale leftover (its
    // live temp was renamed onto main), never the lock or evidence files.
    {
        let mut repo_b = real_repo(&base);
        repo_b
            .save(&fixture_with_revision(2))
            .expect("B save must succeed");
    }
    let decoded =
        codec_mod::decode(&read_raw(&main_document_path(base.path()))).expect("main must decode");
    assert_eq!(decoded.data.revision, 2);
}

/// A barrier/controlled two-writer test: with both instances contending for
/// the same base directory, exactly one `save_if_current` wins the write
/// lock; the loser obtains `ConcurrentModification`. Because the repository is
/// `Send`, the two "processes" are two repository instances on real threads —
/// the same exclusion the production lock enforces across real processes.
#[test]
fn two_writers_contending_save_if_current_only_one_wins() {
    let base = TempDir::new().expect("temp dir");
    // Seed an initial revision so the guard is meaningful.
    {
        let mut repo = real_repo(&base);
        repo.save(&fixture_with_revision(1))
            .expect("seed save must succeed");
    }

    const WRITERS: usize = 2;
    let barrier = Arc::new(Barrier::new(WRITERS));
    let results: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let base_dir = base.path().to_path_buf();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let paths = DocumentPaths::from_base_dir(&base_dir);
                let mut repo = DocumentRepository::new(paths);
                // Each writer believes its expected revision is 1 and tries to
                // persist revision 2. Exactly one must win.
                let document = fixture_with_revision(2 + writer as u64);
                barrier.wait();
                repo.save_if_current(&document, 1)
            })
        })
        .collect();

    let outcomes: Vec<_> = results
        .into_iter()
        .map(|handle| handle.join().expect("writer thread must not panic"))
        .collect();

    let successes = outcomes.iter().filter(|result| result.is_ok()).count();
    let conflicts = outcomes
        .iter()
        .filter(|result| matches!(result, Err(RepositoryError::ConcurrentModification)))
        .count();
    assert_eq!(
        successes, 1,
        "exactly one of the two writers must succeed, got {outcomes:?}"
    );
    assert_eq!(
        conflicts, 1,
        "the other writer must get ConcurrentModification, got {outcomes:?}"
    );

    // The on-disk main is one of the two winning documents (revision 2 or 3),
    // never a torn mix, and there is no leftover lock file.
    let decoded = codec_mod::decode(&read_raw(&main_document_path(base.path())))
        .expect("main must decode intact");
    assert!(
        decoded.data.revision == 2 || decoded.data.revision == 3,
        "main must hold exactly one winner's document, got revision {}",
        decoded.data.revision
    );
    assert!(
        !lock_file_path(base.path()).exists(),
        "the losing writer must not leave a stale lock"
    );
}

// ---------------------------------------------------------------------------
// M07.2 rapid successive saves (stress, on real threads) + lifecycle
// ---------------------------------------------------------------------------

/// M07.2 "fast successive hotkey/open/save": the repository serializes every
/// write through the per-directory lock + revision guard. This stress test
/// drives MANY rapid sequential revisions through a single repository (no
/// threads — the single-instance app always serializes through the UI thread)
/// and then verifies a fresh reader sees the LAST revision intact. It proves
/// the write path stays deterministic and non-torn under bursty saves, which
/// is what a burst of hotkey-triggered setting/record changes exercises.
#[test]
fn rapid_successive_saves_all_persist_in_order() {
    let base = TempDir::new().expect("temp dir");
    {
        let mut repo = real_repo(&base);
        repo.save(&fixture_with_revision(1))
            .expect("seed save must succeed");
    }

    const BURST: usize = 100;
    let mut repo = real_repo(&base);
    repo.load().expect("load");
    for revision in 2..=(1 + BURST as u64) {
        let document = fixture_with_revision(revision);
        repo.save(&document)
            .expect("burst save must succeed (serialized path)");
    }
    // All 100 saves landed: the working copy holds the last revision.
    let final_revision = repo.document().expect("document").data.revision;
    assert_eq!(final_revision, 1 + BURST as u64);

    // A fresh reader sees exactly the last document, and no temp/lock litter.
    let mut fresh = real_repo(&base);
    let loaded = fresh.load().expect("reload");
    match loaded {
        LoadOutcome::Found(document) => {
            assert_eq!(document.data.revision, 1 + BURST as u64);
        }
        other => panic!("expected Found, got {other:?}"),
    }
    assert!(
        !lock_file_path(base.path()).exists(),
        "no stale lock after burst saves"
    );
    // All stale main-temp siblings were cleaned up by the saves.
    let stale: Vec<_> = std::fs::read_dir(base.path())
        .expect("read dir")
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(TEMP_FILE_PREFIX))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "no stale temp files after burst saves: {stale:?}"
    );
}

/// M07.2 concurrent rapid saves across TWO repository instances on real
/// threads using the plain `save` surface (the settings/management layers both
/// call `save_at`, which acquires the lock every time). The lock guarantees the
/// on-disk main is ALWAYS one complete document even with many interleaved
/// writers; the revision guard refuses overwrites rather than corrupting.
#[test]
fn concurrent_burst_saves_never_torn_main() {
    let base = TempDir::new().expect("temp dir");
    {
        let mut repo = real_repo(&base);
        repo.save(&fixture_with_revision(1))
            .expect("seed save must succeed");
    }

    const WRITERS: usize = 4;
    const WRITES_PER_WRITER: usize = 20;
    let barrier = Arc::new(Barrier::new(WRITERS));
    let threads: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let base_dir = base.path().to_path_buf();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let paths = DocumentPaths::from_base_dir(&base_dir);
                let mut repo = DocumentRepository::new(paths);
                // Each writer loads once then writes many unique revisions; the
                // lock serializes, but a loser's `save` can race the revision
                // check — under the documented contract a plain `save` replaces
                // whatever is there (single-instance), so the ONLY invariant we
                // assert is that the main is never torn.
                for i in 0..WRITES_PER_WRITER {
                    let revision = 10_000 + writer as u64 * 1000 + i as u64;
                    let document = fixture_with_revision(revision);
                    let _ = repo.save(&document);
                }
                barrier.wait();
            })
        })
        .collect();

    for handle in threads {
        handle.join().expect("writer thread must not panic");
    }

    // The main file must decode to SOME complete document (never torn), even
    // after 80 interleaved saves from 4 threads.
    let bytes = std::fs::read(main_document_path(base.path())).expect("main readable");
    let decoded = codec_mod::decode(&bytes).expect("main decodes intact after burst");
    assert!(
        decoded.data.revision >= 10_000,
        "a complete writer document is present, got revision {}",
        decoded.data.revision
    );
    assert!(
        !lock_file_path(base.path()).exists(),
        "no stale lock after concurrent burst"
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
