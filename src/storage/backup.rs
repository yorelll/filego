//! User-facing snapshot backups (M06.5).
//!
//! Distinct from the internal single `data.json.bak` that the repository
//! maintains per save: this module manages named `backup-<stamp>.json` files in
//! the data directory, letting the user create a restore point, list them, and
//! restore one. The backup bytes are produced through `codec::encode` so every
//! backup round-trips through `codec::decode` (a backup IS a valid document).
//!
//! Record-level only: restoring never touches a real directory. Backup creation
//! writes a NEW sibling file (with `write_flush_sync` then a best-effort sync);
//! nothing here deletes files, so the storage no-delete guard stays satisfied.

use std::path::{Path, PathBuf};

use super::{codec, codec::StorageError, schema::StoredDocumentV1};

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
            BackupError::Invalid => "the named backup is missing or not a valid document",
        };
        formatter.write_str(text)
    }
}

/// The flat file name of a backup, e.g. `backup-20260921-103000.json`.
pub fn backup_file_name(stamp: &str) -> String {
    format!("{USER_BACKUP_PREFIX}{stamp}.{USER_BACKUP_EXT}")
}

/// The full path of a backup file in `base_dir`.
pub fn backup_file_path(base_dir: &Path, stamp: &str) -> PathBuf {
    base_dir.join(backup_file_name(stamp))
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
    fn missing_or_invalid_backup_is_rejected_anonymously() {
        let base = TempDir::new().expect("temp dir");
        let err = read_backup(base.path(), "backup-nope.json").expect_err("missing");
        assert_eq!(err, BackupError::Invalid);
        std::fs::write(base.path().join("backup-bad.json"), b"not json").expect("bad file");
        let err = read_backup(base.path(), "backup-bad.json").expect_err("invalid");
        assert_eq!(err, BackupError::Invalid);
        // Anonymous: no path or file name leaks into the Display text.
        assert!(!err.to_string().contains("backup-bad"));
        assert!(!err.to_string().contains("\\"));
    }
}
