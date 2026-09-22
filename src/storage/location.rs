//! Data-file location helpers.
//!
//! Pure path derivation over an injected base directory. These helpers never
//! touch the environment, the registry, or the filesystem: the base directory
//! is supplied by the caller (the presenter layer resolves `%LOCALAPPDATA%\FileGo`
//! at M03/M05), which keeps the persistence layer fully testable and free of
//! magic paths.
//!
//! All sibling files live in the same directory as the main document so the
//! atomic rename (`temp` → `main`) stays on one volume.

use std::path::{Path, PathBuf};

/// File name of the live document.
pub const MAIN_FILE_NAME: &str = "data.json";
/// File name of the previous-revision backup.
pub const BACKUP_FILE_NAME: &str = "data.json.bak";
/// Prefix of per-save temp files; the full name is `data.json.tmp.<token>`.
pub const TEMP_FILE_PREFIX: &str = "data.json.tmp.";
/// Prefix of fault-safe backup temp files; the full name is
/// `data.json.bak.tmp.<token>`. Cleanup and the corrupt-evidence scanner must
/// never treat these as main-save temps.
pub const BACKUP_TEMP_FILE_PREFIX: &str = "data.json.bak.tmp.";
/// Prefix of durable corrupt-main evidence files; the full name is
/// `data.json.corrupt-<token>`. These are written once during
/// `repair_from_backup` and are never removed automatically.
pub const CORRUPT_EVIDENCE_PREFIX: &str = "data.json.corrupt-";
/// Name of the per-directory write-lock file (`data.json.lock`). It is
/// exclusive (create-new semantics) and held for the whole write window.
pub const LOCK_FILE_NAME: &str = "data.json.lock";

/// `base_dir/data.json`
pub fn main_document_path(base_dir: &Path) -> PathBuf {
    base_dir.join(MAIN_FILE_NAME)
}

/// `base_dir/data.json.bak`
pub fn backup_document_path(base_dir: &Path) -> PathBuf {
    base_dir.join(BACKUP_FILE_NAME)
}

/// `base_dir/data.json.tmp.<token>` where `token` is caller-provided (a UUID
/// or process-id string). A unique token keeps concurrent/overlapping saves
/// from stepping on the same temp name and lets stale-temp cleanup tell old
/// attempts apart.
pub fn temp_document_path(base_dir: &Path, token: &str) -> PathBuf {
    base_dir.join(format!("{TEMP_FILE_PREFIX}{token}"))
}

/// `base_dir/data.json.bak.tmp.<token>` — fault-safe backup staging file. The
/// new backup bytes are fully written and synced here before an atomic rename
/// onto the live backup, so a partial/failed backup write never corrupts the
/// currently valid `.bak`.
pub fn backup_temp_document_path(base_dir: &Path, token: &str) -> PathBuf {
    base_dir.join(format!("{BACKUP_TEMP_FILE_PREFIX}{token}"))
}

/// `base_dir/data.json.corrupt-<token>` — durable evidence file that preserves
/// the corrupt main bytes reported by `repair_from_backup` before the valid
/// backup is promoted onto main. Each repair produces a distinct token so no
/// evidence file is ever overwritten.
pub fn corrupt_evidence_path(base_dir: &Path, token: &str) -> PathBuf {
    base_dir.join(format!("{CORRUPT_EVIDENCE_PREFIX}{token}"))
}

/// `base_dir/data.json.lock` — the per-directory write lock.
pub fn lock_file_path(base_dir: &Path) -> PathBuf {
    base_dir.join(LOCK_FILE_NAME)
}

/// The three persistence paths for one base directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPaths {
    base_dir: PathBuf,
}

impl DocumentPaths {
    pub fn from_base_dir(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Borrow the injected base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn main(&self) -> PathBuf {
        main_document_path(&self.base_dir)
    }

    pub fn backup(&self) -> PathBuf {
        backup_document_path(&self.base_dir)
    }

    pub fn temp(&self, token: &str) -> PathBuf {
        temp_document_path(&self.base_dir, token)
    }

    pub fn backup_temp(&self, token: &str) -> PathBuf {
        backup_temp_document_path(&self.base_dir, token)
    }

    pub fn corrupt_evidence(&self, token: &str) -> PathBuf {
        corrupt_evidence_path(&self.base_dir, token)
    }

    pub fn lock(&self) -> PathBuf {
        lock_file_path(&self.base_dir)
    }
}
