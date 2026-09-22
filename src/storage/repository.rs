//! Safe atomic persistence for the versioned JSON document.
//!
//! M01-B sits behind the pure codec (M01-A): it resolves file names from an
//! injected base directory, encodes via `codec::encode`, and writes through
//! `FileOps` using the sequence temp write + flush/sync → backup previous main
//! → atomic rename onto main → best-effort post-replace sync. A failed step
//! never deletes the original document or its backup; a corrupt main is
//! preserved (never silently overwritten) while a valid backup is promoted for
//! recovery; removing a folder record mutates the repository's in-memory
//! document only and never touches real folder or file paths.
//!
//! Ordering contract for [`DocumentRepository::save`]:
//! 1. `codec::encode` (pure; on failure nothing touches the disk);
//! 2. write the temp file (open/create, `write_all`, `flush`, `sync_all`);
//! 3. if a main exists, copy main → backup (crash before rename loses nothing);
//! 4. atomically replace main with temp (`std::fs::rename` on Windows replaces
//!    the destination);
//! 5. best-effort `sync_path` on the main file after the rename for
//!    directory-consistency; this step is deliberately non-fatal.
//!
//! Any failing step returns an error without deleting the original main file
//! or its backup, and leaves the temp for the next save's cleanup.

use std::fmt;

use crate::domain::ids::FolderId;

use super::{
    codec,
    io::{FileOps, FsFileOps},
    location::{DocumentPaths, TEMP_FILE_PREFIX},
    schema::StoredDocumentV1,
};

/// Outcome of [`DocumentRepository::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// Main file was missing or not decodable while an intact backup decoded
    /// successfully. The document is reconstructed from the backup; the caller
    /// decides whether to auto-repair on the next save. If a corrupt main
    /// exists its bytes remain on disk untouched.
    Recovered(StoredDocumentV1),
    /// The main file was read and decoded successfully.
    Found(StoredDocumentV1),
}

/// Typed persistence error. `Display` is intentionally fixed and anonymous:
/// it never carries paths, folder names, JSON content or any stored user data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    Io,
    NotFound,
    CorruptData,
    ConcurrentModification,
    InvalidData,
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            RepositoryError::Io => "stored data could not be written or read",
            RepositoryError::NotFound => "no stored data exists",
            RepositoryError::CorruptData => "stored data is corrupt and no backup is recoverable",
            RepositoryError::ConcurrentModification => {
                "stored data was changed concurrently and was not overwritten"
            }
            RepositoryError::InvalidData => "stored data does not form a valid document",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RepositoryError {}

impl From<codec::StorageError> for RepositoryError {
    fn from(_: codec::StorageError) -> Self {
        RepositoryError::InvalidData
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(_: std::io::Error) -> Self {
        RepositoryError::Io
    }
}

/// Persistence boundary over the encoded document.
///
/// The repository owns an in-memory working copy of the document (seeded by
/// [`DocumentRepository::load`]); mutating helpers such as `remove_folder`
/// edit that copy, and `save` persists it atomically. No data-directory
/// resolution happens here (that is the presenter's job at M03/M05), and no
/// Windows API beyond `std::fs` is used.
pub struct DocumentRepository {
    paths: DocumentPaths,
    io: Box<dyn FileOps>,
    /// In-memory working copy. `None` until `load` succeeds.
    document: Option<StoredDocumentV1>,
    /// The document revision this repository most recently observed on disk
    /// (from a load or a successful save). `None` means no baseline is
    /// established, so the revision guard is not enforced.
    latest_loaded_revision: Option<u64>,
}

impl DocumentRepository {
    pub fn new(paths: DocumentPaths) -> Self {
        Self {
            paths,
            io: Box::new(FsFileOps),
            document: None,
            latest_loaded_revision: None,
        }
    }

    fn main(&self) -> std::path::PathBuf {
        self.paths.main()
    }

    fn backup(&self) -> std::path::PathBuf {
        self.paths.backup()
    }

    fn temp(&self, token: &str) -> std::path::PathBuf {
        self.paths.temp(token)
    }

    /// Encode and atomically persist `document`.
    ///
    /// See the module docs for the ordering contract. On success the
    /// repository's known revision becomes `document.data.revision`.
    pub fn save(&mut self, document: &StoredDocumentV1) -> Result<(), RepositoryError> {
        let bytes = codec::encode(document)?;

        // A fresh unique temp name per save keeps concurrent saves from
        // colliding on one fixed token and makes stale-temp cleanup safe.
        let token = Self::new_temp_token();
        let temp = self.temp(&token);
        self.io.write_flush_sync(&temp, &bytes)?;

        let main = self.main();
        if self.io.exists(&main) {
            let backup = self.backup();
            self.io.copy(&main, &backup)?;
        }

        // Atomic replace on Windows: std::fs::rename moves temp over main.
        self.io.rename(&temp, &main)?;

        // Optional directory-consistency step: syncing the replaced main is a
        // best effort. It must not turn a logical success into a reported
        // failure, so an error here is deliberately ignored.
        let _ = self.io.sync_path(&main);

        self.cleanup_stale_temps();

        self.document = Some(document.clone());
        self.latest_loaded_revision = Some(document.data.revision);
        Ok(())
    }

    /// Best-effort removal of `data.json.tmp.*` siblings left behind by a
    /// crashed or interrupted earlier save. The just-completed save's temp has
    /// already been renamed away, so anything matching the prefix is stale.
    /// Errors are ignored: cleanup is best-effort and never blocks a save.
    fn cleanup_stale_temps(&mut self) {
        let Ok(entries) = std::fs::read_dir(self.paths.base_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(TEMP_FILE_PREFIX) {
                let _ = self.io.remove(&entry.path());
            }
        }
    }

    /// Load the main document, recovering from an intact backup when the main
    /// file is missing or corrupt.
    ///
    /// - no main and no backup → [`RepositoryError::NotFound`] (never a silent
    ///   default, never an auto-created document);
    /// - main valid → `LoadOutcome::Found`;
    /// - main missing/corrupt + valid backup → `LoadOutcome::Recovered`, with
    ///   a corrupt main preserved on disk (the caller decides whether to
    ///   auto-repair on the next save);
    /// - main corrupt + missing/invalid backup → [`RepositoryError::CorruptData`],
    ///   corrupt main still preserved on disk.
    pub fn load(&mut self) -> Result<LoadOutcome, RepositoryError> {
        let main = self.main();
        let main_bytes = self.io.read(&main);

        let main_decoded = match &main_bytes {
            Ok(bytes) => codec::decode(bytes).ok(),
            Err(_) => None,
        };

        match main_decoded {
            Some(document) => {
                let revision = document.data.revision;
                self.document = Some(document.clone());
                self.latest_loaded_revision = Some(revision);
                Ok(LoadOutcome::Found(document))
            }
            None => {
                // Main missing (unreadable) or corrupt: try the backup. The
                // corrupt/absent main is left untouched on disk as evidence.
                match self.try_recover_backup()? {
                    Some(document) => {
                        let revision = document.data.revision;
                        self.document = Some(document.clone());
                        self.latest_loaded_revision = Some(revision);
                        Ok(LoadOutcome::Recovered(document))
                    }
                    None => {
                        if main_bytes.is_ok() {
                            // There is a main file but it is corrupt and no
                            // backup is recoverable.
                            Err(RepositoryError::CorruptData)
                        } else {
                            // No main readable and no recoverable backup.
                            Err(RepositoryError::NotFound)
                        }
                    }
                }
            }
        }
    }

    /// Decode the backup file; `Ok(None)` when backup is absent or invalid.
    fn try_recover_backup(&mut self) -> Result<Option<StoredDocumentV1>, RepositoryError> {
        let backup = self.backup();
        let backup_bytes = match self.io.read(&backup) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Ok(None),
        };
        Ok(codec::decode(&backup_bytes).ok())
    }

    /// Save `document` only when it is based on the revision this repository
    /// most recently read on disk; otherwise refuse to overwrite newer state.
    ///
    /// The guard reads the current main's revision directly, so an external
    /// concurrent writer (not this repository instance) is detected too.
    /// After a successful save, the known revision becomes the saved revision.
    pub fn save_if_current(
        &mut self,
        document: &StoredDocumentV1,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        let on_disk = match self.on_disk_revision()? {
            Some(revision) => revision,
            None => expected_revision,
        };

        if on_disk != expected_revision {
            return Err(RepositoryError::ConcurrentModification);
        }

        self.save(document)
    }

    /// Revision currently stored in the main file, if readable and decodable.
    fn on_disk_revision(&mut self) -> Result<Option<u64>, RepositoryError> {
        let main = self.main();
        let bytes = match self.io.read(&main) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Ok(None),
        };
        Ok(codec::decode(&bytes).ok().map(|d| d.data.revision))
    }

    /// Remove the in-memory folder record for `folder_id`. This is the only
    /// deletion surface for folder records: the real folder and every path it
    /// references are never touched. Returns `false` when no record was found.
    pub fn remove_folder(&mut self, folder_id: FolderId) -> bool {
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let initial_count = document.data.folders.len();
        document
            .data
            .folders
            .retain(|folder| folder.id != folder_id);
        document.data.folders.len() != initial_count
    }

    /// Document currently held in memory, if a load or save established one.
    pub fn document(&self) -> Option<&StoredDocumentV1> {
        self.document.as_ref()
    }

    /// Revision of the document most recently observed on disk.
    pub fn latest_loaded_revision(&self) -> Option<u64> {
        self.latest_loaded_revision
    }

    /// Number of folder records currently in the working copy.
    pub fn folder_count(&self) -> usize {
        self.document
            .as_ref()
            .map_or(0, |document| document.data.folders.len())
    }

    /// Generate a unique temp-file token for one save. Uses a fresh UUID so
    /// concurrent saves in the same process never collide on a temp name; the
    /// token is used only within a single save call and never persisted.
    fn new_temp_token() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Test-only constructor that injects a custom `FileOps` (used by the
    /// fault-injection matrix). Gate keeps the seam out of release builds.
    #[cfg(test)]
    pub fn with_io(paths: DocumentPaths, io: Box<dyn FileOps>) -> Self {
        Self {
            paths,
            io,
            document: None,
            latest_loaded_revision: None,
        }
    }
}
