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

/// `base_dir/data.json`
pub fn main_path(base_dir: &Path) -> PathBuf {
    base_dir.join(MAIN_FILE_NAME)
}

/// `base_dir/data.json.bak`
pub fn backup_path(base_dir: &Path) -> PathBuf {
    base_dir.join(BACKUP_FILE_NAME)
}

/// `base_dir/data.json.tmp.<token>` where `token` is caller-provided (a UUID
/// or process-id string). A unique token keeps concurrent/overlapping saves
/// from stepping on the same temp name and lets stale-temp cleanup tell old
/// attempts apart.
pub fn temp_path(base_dir: &Path, token: &str) -> PathBuf {
    base_dir.join(format!("{TEMP_FILE_PREFIX}{token}"))
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
        main_path(&self.base_dir)
    }

    pub fn backup(&self) -> PathBuf {
        backup_path(&self.base_dir)
    }

    pub fn temp(&self, token: &str) -> PathBuf {
        temp_path(&self.base_dir, token)
    }
}
