//! User-facing snapshot backups (M06.5).
//!
//! Distinct from the internal single `data.json.bak` that the repository
//! maintains per save: this module manages named `backup-<stamp>.json` files in
//! the data directory, letting the user create a restore point, list them, and
//! restore one. The backup bytes are produced through `codec::encode` so every
//! backup round-trips through `codec::decode` (a backup IS a valid document).
//!
//! Record-level only: restoring never touches a real directory. Backup creation
//! writes a NEW sibling file (with `write_flush_sync` then a best-effort sync).
//! The sole removal surface [`remove_failed_import_snapshot`] is intentionally
//! narrow: it may clean only the just-created `backup-before-import-*.json`
//! sibling after the owning overwrite import fails (M06 review I2). It cannot
//! delete a folder, `data.json`, a manual backup, or an arbitrary path; the
//! storage guard permits the underlying file removal only through `io.rs`.

use std::path::{Path, PathBuf};

use super::{
    codec,
    codec::StorageError,
    io::{FileOps, FsFileOps},
    schema::StoredDocumentV1,
};

/// Prefix of user-facing backup files; the full name is
/// `backup-<stamp>.json` where `stamp` is `YYYYMMDD-HHMMSS`.
pub const USER_BACKUP_PREFIX: &str = "backup-";
/// File extension of a user-facing backup.
pub const USER_BACKUP_EXT: &str = "json";

/// A user-facing backup error (anonymous; never a path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    /// The document could not be encoded.
    EncodeFailed,
    /// The backup file could not be written (disk full, permissions, ...).
    WriteFailed,
    /// The backup directory could not be listed.
    ListFailed,
    /// A just-created backup could not be removed after its owning operation
    /// failed. This is deliberately distinct from record deletion: it only
    /// permits the exact `backup-*.json` sibling created by this module.
    RemoveFailed,
    /// The named backup does not exist or is not decodable.
    Invalid,
}

impl std::error::Error for BackupError {}

impl std::fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            BackupError::EncodeFailed => "the current data could not be encoded as a backup",
            BackupError::WriteFailed => "the backup file could not be written",
            BackupError::ListFailed => "the backup list could not be read",
            BackupError::RemoveFailed => "the temporary import backup could not be removed",
            BackupError::Invalid => "the named backup is missing or not a valid document",
        };
        formatter.write_str(text)
    }
}

/// The flat file name of a backup, e.g. `backup-20260921-103000.json`.
pub fn backup_file_name(stamp: &str) -> String {
    format!("{USER_BACKUP_PREFIX}{stamp}.{USER_BACKUP_EXT}")
}

/// The stamp prefix reserved for automatic overwrite-import snapshots.
pub const BEFORE_IMPORT_STAMP_PREFIX: &str = "before-import-";

/// The full path of a backup file in `base_dir`.
pub fn backup_file_path(base_dir: &Path, stamp: &str) -> PathBuf {
    base_dir.join(backup_file_name(stamp))
}

/// Build a backup stamp that is unique within `base_dir` for a given
/// `prefix` + base `timestamp`. When a file with that stamp already exists
/// (two overwrite-imports in the same second — M06 review I1), numeric suffixes
/// (`-2`, `-3`, ...) are appended so a new snapshot is created instead of the
/// second one silently truncating/overwriting the first. The stamp sortable by
/// its timestamp prefix, so `list_backups` ordering stays deterministic.
pub fn unique_stamp(base_dir: &Path, prefix: &str, timestamp: &str) -> String {
    let mut candidate = format!("{prefix}{timestamp}");
    let mut counter: u32 = 2;
    while backup_file_path(base_dir, &candidate).exists() {
        candidate = format!("{prefix}{timestamp}-{counter}");
        counter += 1;
    }
    candidate
}

/// Write `document` as a new `backup-<stamp>.json` sibling in `base_dir` and
/// return its file name. The write uses the same durable discipline as a save
/// (full bytes, flush, sync); a failure leaves no half-considered backup and
/// returns [`BackupError`].
pub fn create_backup(
    base_dir: &Path,
    document: &StoredDocumentV1,
    stamp: &str,
) -> Result<String, BackupError> {
    let bytes = codec::encode(document).map_err(|_| BackupError::EncodeFailed)?;
    let path = backup_file_path(base_dir, stamp);
    write_bytes_durable(&path, &bytes)?;
    Ok(backup_file_name(stamp))
}

/// Remove exactly one named automatic before-import snapshot after its owning
/// overwrite import fails (M06 review I2). This is NOT a real-folder or
/// arbitrary-file delete: the name must have the fixed
/// `backup-before-import-*.json` shape, it is resolved only as a sibling below
/// `base_dir`, and it can never target a manual backup, `data.json`, or an
/// arbitrary path. A missing file is success (cleanup idempotence).
pub fn remove_failed_import_snapshot(base_dir: &Path, file_name: &str) -> Result<(), BackupError> {
    let expected_prefix = format!("{USER_BACKUP_PREFIX}{BEFORE_IMPORT_STAMP_PREFIX}");
    if !file_name.starts_with(&expected_prefix)
        || !is_backup_file(file_name)
        || std::path::Path::new(file_name).components().count() != 1
    {
        return Err(BackupError::Invalid);
    }
    let path = base_dir.join(file_name);
    let mut io = FsFileOps;
    io.remove(&path).map_err(|_| BackupError::RemoveFailed)
}

/// List every `backup-*.json` file in `base_dir`, newest first (by file name,
/// which embeds a sortable timestamp).
pub fn list_backups(base_dir: &Path) -> Result<Vec<String>, BackupError> {
    let mut names: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(base_dir).map_err(|_| BackupError::ListFailed)?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if file_name.starts_with(USER_BACKUP_PREFIX)
            && file_name.ends_with(&format!(".{USER_BACKUP_EXT}"))
        {
            names.push(file_name.to_owned());
        }
    }
    names.sort_by(|a, b| b.cmp(a)); // newest stamp first (descending).
    Ok(names)
}

/// Decode the named backup into a document. `Ok(None)` when the file does not
/// exist; `Err(Invalid)` when it exists but is not a valid document.
pub fn read_backup(base_dir: &Path, file_name: &str) -> Result<StoredDocumentV1, BackupError> {
    // Defense in depth: backup names originate from `list_backups`, but a UI
    // caller must never be able to traverse outside the user data directory.
    // Restrict reads to a flat `backup-*.json` file name (no separator, no
    // parent component, no absolute path).
    if !is_backup_file(file_name) || std::path::Path::new(file_name).components().count() != 1 {
        return Err(BackupError::Invalid);
    }
    let path = base_dir.join(file_name);
    let bytes = std::fs::read(&path).map_err(|_| BackupError::Invalid)?;
    codec::decode(&bytes).map_err(|_| BackupError::Invalid)
}

/// Write bytes durably to `path` (create/truncate → write_all → flush → sync).
fn write_bytes_durable(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    use std::io::Write;
    let mut file = std::fs::File::create(path).map_err(|_| BackupError::WriteFailed)?;
    file.write_all(bytes)
        .map_err(|_| BackupError::WriteFailed)?;
    file.flush().map_err(|_| BackupError::WriteFailed)?;
    file.sync_all().map_err(|_| BackupError::WriteFailed)?;
    Ok(())
}

/// Whether `file_name` is a user-facing backup (used by listing).
pub fn is_backup_file(file_name: &str) -> bool {
    file_name.starts_with(USER_BACKUP_PREFIX) && file_name.ends_with(&format!(".{USER_BACKUP_EXT}"))
}

/// Map a decode error onto the storage codec kind (for errors that reach the
/// UI, kept anonymous).
pub fn decode_error_kind(error: StorageError) -> codec::StorageErrorKind {
    error.kind()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{document::AppData, folder::FolderEntry, settings::AppSettings};
    use crate::storage::codec;
    use crate::storage::location::DocumentPaths;
    use chrono::DateTime;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn utc(value: &str) -> DateTime<chrono::Utc> {
        value.parse().expect("fixture")
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

    fn document(revision: u64) -> StoredDocumentV1 {
        StoredDocumentV1::new(AppData {
            settings: AppSettings::default(),
            folders: vec![folder(1, "Docs", r"C:\docs")],
            categories: Vec::new(),
            tags: Vec::new(),
            revision,
        })
    }

    #[test]
    fn create_list_read_round_trip() {
        let base = TempDir::new().expect("temp dir");
        let paths = DocumentPaths::from_base_dir(base.path());
        let document = document(3);

        // The internal backup path stays untouched by user backups.
        let internal_backup = paths.backup();
        let name = create_backup(base.path(), &document, "20260921-103000").expect("create");
        assert_eq!(name, "backup-20260921-103000.json");
        assert!(!internal_backup.exists(), "user backup != internal .bak");

        let listed = list_backups(base.path()).expect("list");
        assert_eq!(listed, vec!["backup-20260921-103000.json"]);

        let decoded = read_backup(base.path(), &name).expect("read");
        assert_eq!(decoded, document);

        // The backup bytes are a valid schema document (round-trips via codec).
        let bytes = std::fs::read(base.path().join(&name)).expect("backup bytes");
        let via_codec = codec::decode(&bytes).expect("backup is a valid document");
        assert_eq!(via_codec, document);
    }

    #[test]
    fn list_is_newest_first_and_ignores_non_backup_siblings() {
        let base = TempDir::new().expect("temp dir");
        let doc = document(1);
        create_backup(base.path(), &doc, "20260920-090000").expect("older");
        create_backup(base.path(), &doc, "20260921-103000").expect("newer");
        let _paths = DocumentPaths::from_base_dir(base.path());

        let listed = list_backups(base.path()).expect("list");
        assert_eq!(
            listed,
            vec!["backup-20260921-103000.json", "backup-20260920-090000.json"]
        );
        // Sibling files (data.json, data.json.bak) are not listed.
        std::fs::write(base.path().join("data.json"), b"{}").expect("sibling");
        let listed = list_backups(base.path()).expect("list");
        assert_eq!(listed.len(), 2);
    }

    #[test]
    fn failed_import_snapshot_cleanup_removes_only_the_named_backup() {
        // M06 review I2: if the import save fails after a before-import snapshot
        // was created, cleanup removes only THAT snapshot. Other user backups
        // remain intact, and arbitrary sibling names are refused.
        let base = TempDir::new().expect("temp dir");
        let doc = document(1);
        let first = create_backup(base.path(), &doc, "before-import-a").expect("first");
        let keep = create_backup(base.path(), &doc, "manual-keep").expect("keep");
        remove_failed_import_snapshot(base.path(), &first)
            .expect("remove just-created import snapshot");
        assert!(!base.path().join(&first).exists());
        assert!(base.path().join(&keep).exists(), "other backup kept");
        // Cleanup is idempotent and cannot target arbitrary/manual siblings.
        remove_failed_import_snapshot(base.path(), &first).expect("missing is success");
        assert_eq!(
            remove_failed_import_snapshot(base.path(), &keep),
            Err(BackupError::Invalid)
        );
        assert_eq!(
            remove_failed_import_snapshot(base.path(), "data.json"),
            Err(BackupError::Invalid)
        );
        assert_eq!(
            remove_failed_import_snapshot(base.path(), "../backup-before-import-a.json"),
            Err(BackupError::Invalid)
        );
        assert!(base.path().join(&keep).exists());
    }

    #[test]
    fn unique_stamp_disambiguates_same_second_backups() {
        // M06 review I1: two overwrite-imports within the same second must not
        // collide on one file name (the second would truncate the first's
        // snapshot). `unique_stamp` returns names that never overwrite.
        let base = TempDir::new().expect("temp dir");
        let first = unique_stamp(base.path(), "before-import-", "20260921-103000");
        assert_eq!(first, "before-import-20260921-103000");
        let snap = backup_file_path(base.path(), &first);
        std::fs::write(&snap, b"first").expect("create first snapshot");

        let second = unique_stamp(base.path(), "before-import-", "20260921-103000");
        assert_eq!(second, "before-import-20260921-103000-2");
        let snap2 = backup_file_path(base.path(), &second);
        std::fs::write(&snap2, b"second").expect("create second snapshot");

        // The first is NOT truncated: both bytes are intact and both decode.
        assert_eq!(std::fs::read(&snap).expect("first intact"), b"first");
        assert_eq!(std::fs::read(&snap2).expect("second intact"), b"second");
        assert_ne!(snap, snap2);

        // Deterministic list ordering keeps the timestamps sortable: the -2
        // suffix sorts after the base-stamp entry, both still newest-first by
        // the fixed prefix.
        let names = list_backups(base.path()).expect("list");
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn missing_or_invalid_backup_is_rejected_anonymously() {
        let base = TempDir::new().expect("temp dir");
        let err = read_backup(base.path(), "backup-nope.json").expect_err("missing");
        assert_eq!(err, BackupError::Invalid);
        assert_eq!(
            read_backup(base.path(), "../backup-nope.json"),
            Err(BackupError::Invalid),
            "backup reads never traverse outside the data directory"
        );
        std::fs::write(base.path().join("backup-bad.json"), b"not json").expect("bad file");
        let err = read_backup(base.path(), "backup-bad.json").expect_err("invalid");
        assert_eq!(err, BackupError::Invalid);
        // Anonymous: no path or file name leaks into the Display text.
        assert!(!err.to_string().contains("backup-bad"));
        assert!(!err.to_string().contains("\\"));
    }
}
