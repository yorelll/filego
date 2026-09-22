//! Safe atomic persistence for the versioned JSON document.
//!
//! M01-B sits behind the pure codec (M01-A): it resolves file names from an
//! injected base directory, encodes via `codec::encode`, and writes through
//! `FileOps` using the sequence temp write + flush/sync → fault-safe backup
//! replace → atomic rename onto main → best-effort post-replace sync. A failed
//! step never deletes the original document or its backup; a corrupt main is
//! preserved (never silently overwritten) while a valid backup is promoted for
//! explicit, evidence-preserving repair; removing a folder record mutates the
//! repository's in-memory document only and never touches real folder or file
//! paths.
//!
//! # Ordering contract for [`DocumentRepository::save`]
//! 1. `codec::encode` (pure; on failure nothing touches the disk);
//! 2. acquire the per-directory write lock (`data.json.lock`);
//! 3. write the main temp file (open/create, `write_all`, `flush`, `sync_all`);
//! 4. if a main exists, fault-safely replace the backup: write the new backup
//!    bytes to a backup temp (`data.json.bak.tmp.<token>`), sync them, then
//!    atomic-rename onto the live `data.json.bak` — a partial or failed backup
//!    write never damages the previous `.bak`;
//! 5. atomically replace main with the main temp (`std::fs::rename` on Windows
//!    replaces the destination);
//! 6. best-effort `sync_path` on the main file after the rename;
//! 7. while still holding the lock, remove stale `data.json.tmp.*` siblings;
//! 8. release the lock (the lock file is removed best-effort on drop).
//!
//! Any failing step returns an error without deleting the original main file
//! or its backup, and leaves the temp for a later save's cleanup.
//!
//! # Recovery contract
//! `load` reconstructs a missing/corrupt main from a valid backup and returns
//! [`LoadOutcome::Recovered`], leaving the corrupt/absent main untouched and
//! marking the repository as *pending repair*. While pending repair, both
//! [`DocumentRepository::save`] and [`DocumentRepository::save_if_current`]
//! return [`RepositoryError::RecoveryRequired`] WITHOUT touching the main or
//! the backup, so the only valid recovered copy can never be overwritten by the
//! still-corrupt main (F001).
//!
//! [`DocumentRepository::repair_from_backup`] resolves the state: it re-reads
//! the main; if it is still corrupt (or absent) while the backup is valid, it
//! first persists the corrupt main bytes to a distinct durable evidence file
//! (`data.json.corrupt-<uuid>`), then promotes the valid backup onto the main
//! using the same write-then-rename discipline, clears the pending state, and
//! updates the observed revision. After repair, ordinary saves operate on a
//! healthy main. If an external actor already repaired the main, the pending
//! state is simply cleared.
//!
//! # Concurrency contract
//! 0.0.1 runs a single process, but every write is serialized by the
//! per-directory `data.json.lock` (exclusive create-new semantics), which is
//! held for the whole revision check → backup → rename → cleanup window and
//! covers both `save` and `save_if_current` (F003). Two writers sharing a base
//! directory therefore cannot both pass a check-then-rename: the loser gets
//! [`RepositoryError::ConcurrentModification`] at lock acquisition (or later
//! from the revision guard). The revision guard itself distinguishes a missing
//! main (no baseline), a corrupt main (an explicit `CorruptData` refusal — it
//! never fakes an expected revision), and a readable main (F002); non-`NotFound`
//! I/O errors are surfaced as `Io` and never misread as "missing" or "corrupt"
//! (F004).
//!
//! Stale-temp cleanup runs ONLY while the lock is held, so any
//! `data.json.tmp.*` present at that moment is provably abandoned by its
//! writer; evidence files (`data.json.corrupt-*`) and backup staging files
//! (`data.json.bak.tmp.*`) are never removed. A crashed writer's leftover lock
//! is deliberately NOT auto-expired (auto-expiry would silently break the
//! exclusion); removing a stale lock manually is a documented recovery step
//! that is out of scope for 0.0.1.

use std::fmt;

use crate::domain::ids::FolderId;

use super::{
    codec,
    io::{FileOps, FsFileOps, WriteLock},
    location::{DocumentPaths, TEMP_FILE_PREFIX},
    schema::StoredDocumentV1,
};

/// Outcome of [`DocumentRepository::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// Main file was missing or not decodable while an intact backup decoded
    /// successfully. The document is reconstructed from the backup and the
    /// repository enters *pending repair*: any save is rejected until
    /// [`DocumentRepository::repair_from_backup`] resolves the state. If a
    /// corrupt main exists its bytes remain on disk untouched.
    Recovered(StoredDocumentV1),
    /// The main file was read and decoded successfully.
    Found(StoredDocumentV1),
}

/// Outcome of [`DocumentRepository::repair_from_backup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairOutcome {
    /// A corrupt/absent main was repaired from the valid backup. When a corrupt
    /// main existed, its exact bytes were preserved in the returned durable
    /// evidence file (never lost, never overwritten); the backup still holds
    /// the promoted document's bytes.
    Repaired {
        /// Path of the durable corrupt-main evidence file, or `None` when the
        /// main file was absent (there were no corrupt bytes to preserve).
        evidence: Option<std::path::PathBuf>,
    },
    /// There was nothing to repair: either the repository was not pending
    /// recovery, or the main already decoded on re-read (an external actor
    /// repaired it). In the latter case the pending state is cleared and the
    /// observed revision is refreshed from the healthy main.
    HadNoCorruptMain,
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
    /// A corrupt main was recovered from backup and `repair_from_backup` has
    /// not run yet; saves are rejected so the only valid copy is protected.
    RecoveryRequired,
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
            RepositoryError::RecoveryRequired => {
                "stored data is corrupt and requires an explicit repair before it can be saved"
            }
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
    /// Set when `load` recovered a missing/corrupt main from a valid backup
    /// and `repair_from_backup` has not run yet. While set, saves are rejected
    /// with `RecoveryRequired` and no write touches the main or the backup.
    pending_recovery: bool,
}

impl DocumentRepository {
    pub fn new(paths: DocumentPaths) -> Self {
        Self {
            paths,
            io: Box::new(FsFileOps),
            document: None,
            latest_loaded_revision: None,
            pending_recovery: false,
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

    fn backup_temp(&self, token: &str) -> std::path::PathBuf {
        self.paths.backup_temp(token)
    }

    fn corrupt_evidence(&self, token: &str) -> std::path::PathBuf {
        self.paths.corrupt_evidence(token)
    }

    fn lock(&self) -> std::path::PathBuf {
        self.paths.lock()
    }

    fn reject_if_pending_recovery(&self) -> Result<(), RepositoryError> {
        if self.pending_recovery {
            Err(RepositoryError::RecoveryRequired)
        } else {
            Ok(())
        }
    }

    /// Acquire the per-directory write lock. Another writer holding the lock
    /// (or a stale lock left by a crashed writer) yields
    /// [`RepositoryError::ConcurrentModification`]; any other failure yields
    /// `RepositoryError::Io`.
    fn acquire_write_lock(&mut self) -> Result<WriteLock, RepositoryError> {
        let lock_path = self.lock();
        match self.io.create_new(&lock_path) {
            Ok(()) => Ok(WriteLock::new(lock_path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(RepositoryError::ConcurrentModification)
            }
            Err(_) => Err(RepositoryError::Io),
        }
    }

    /// Encode and atomically persist `document`.
    ///
    /// See the module docs for the ordering and concurrency contract. Rejects
    /// with [`RepositoryError::RecoveryRequired`] while the repository is
    /// pending repair (a corrupt main recovered from backup).
    pub fn save(&mut self, document: &StoredDocumentV1) -> Result<(), RepositoryError> {
        self.reject_if_pending_recovery()?;
        let bytes = codec::encode(document)?;
        let _lock = self.acquire_write_lock()?;
        self.save_locked(document, &bytes)
    }

    /// Persist `document` only when the on-disk main matches `expected_revision`
    /// (or there is no main yet at all); otherwise refuse to overwrite newer
    /// state.
    ///
    /// The whole check-and-save runs under the write lock, so a concurrent
    /// writer cannot slip a check-then-rename between our check and our
    /// rename. A corrupt main is refused with `CorruptData` and never treated
    /// as a matching baseline; pending-repair repositories are refused with
    /// `RecoveryRequired`.
    pub fn save_if_current(
        &mut self,
        document: &StoredDocumentV1,
        expected_revision: u64,
    ) -> Result<(), RepositoryError> {
        self.reject_if_pending_recovery()?;
        let bytes = codec::encode(document)?;
        let _lock = self.acquire_write_lock()?;
        match self.on_disk_revision()? {
            OnDiskRevision::Missing => {}
            OnDiskRevision::Corrupt => return Err(RepositoryError::CorruptData),
            OnDiskRevision::Ok(revision) if revision != expected_revision => {
                return Err(RepositoryError::ConcurrentModification);
            }
            OnDiskRevision::Ok(_) => {}
        }
        self.save_locked(document, &bytes)
    }

    /// Classify the main file's on-disk state for the revision guard. Only an
    /// explicit `NotFound` means missing; a readable-but-undecodable main is
    /// `Corrupt`; any other read error is surfaced as [`RepositoryError::Io`]
    /// instead of being mistaken for missing or corrupt (F002, F004).
    fn on_disk_revision(&mut self) -> Result<OnDiskRevision, RepositoryError> {
        let main = self.main();
        let bytes = match self.io.read(&main) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OnDiskRevision::Missing);
            }
            Err(_) => return Err(RepositoryError::Io),
        };
        match codec::decode(&bytes) {
            Ok(document) => Ok(OnDiskRevision::Ok(document.data.revision)),
            Err(_) => Ok(OnDiskRevision::Corrupt),
        }
    }

    /// The write steps shared by `save` and `save_if_current`. The caller must
    /// hold the write lock and have passed all guard checks.
    fn save_locked(
        &mut self,
        document: &StoredDocumentV1,
        bytes: &[u8],
    ) -> Result<(), RepositoryError> {
        // A fresh unique temp name per save keeps concurrent saves from
        // colliding on one fixed token and makes stale-temp cleanup safe.
        let token = Self::new_temp_token();
        let temp = self.temp(&token);
        self.io.write_flush_sync(&temp, bytes)?;

        let main = self.main();
        if self.io.exists(&main)? {
            self.replace_backup_with_current_main()?;
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
        self.pending_recovery = false;
        Ok(())
    }

    /// Fault-safe replacement of the live backup bytes with the current main's
    /// bytes (F005). The new backup is first written to a backup temp and
    /// synced, then atomic-renamed onto `data.json.bak`. A partial or failed
    /// backup write leaves the previous `.bak` byte-identical.
    fn replace_backup_with_current_main(&mut self) -> Result<(), RepositoryError> {
        let main_bytes = self.io.read(&self.main())?;
        let token = Self::new_temp_token();
        let backup_temp = self.backup_temp(&token);
        self.io.write_flush_sync(&backup_temp, &main_bytes)?;
        self.io.rename(&backup_temp, &self.backup())?;
        Ok(())
    }

    /// Best-effort removal of `data.json.tmp.*` siblings left behind by a
    /// crashed or interrupted earlier save. Runs only while the write lock is
    /// held, so no other writer can be mid-write and every matching sibling is
    /// provably stale. Only the main-temp prefix is matched: corrupt-evidence
    /// files (`data.json.corrupt-*`), backup staging files
    /// (`data.json.bak.tmp.*`) and the lock file are never touched. Errors are
    /// ignored: cleanup is best-effort and never blocks a save.
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
    ///   the repository entering pending repair and any corrupt main preserved
    ///   on disk;
    /// - main corrupt + missing/invalid backup → [`RepositoryError::CorruptData`],
    ///   corrupt main still preserved on disk;
    /// - main or backup present but not readable (access denied, offline store,
    ///   ...) → [`RepositoryError::Io`]. An inaccessible file is never
    ///   mistaken for a missing one, and a main read failure does not trigger
    ///   backup recovery.
    pub fn load(&mut self) -> Result<LoadOutcome, RepositoryError> {
        let main = self.main();
        let main_bytes = match self.io.read(&main) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(RepositoryError::Io),
        };

        let main_decoded = main_bytes
            .as_deref()
            .and_then(|bytes| codec::decode(bytes).ok());

        match main_decoded {
            Some(document) => {
                let revision = document.data.revision;
                self.document = Some(document.clone());
                self.latest_loaded_revision = Some(revision);
                // A healthy main means there is nothing left to repair.
                self.pending_recovery = false;
                Ok(LoadOutcome::Found(document))
            }
            None => {
                // Main missing (NotFound) or corrupt: try the backup. The
                // corrupt/absent main is left untouched on disk as evidence.
                match self.try_recover_backup()? {
                    Some(document) => {
                        // Main is missing/corrupt but the backup decoded: enter
                        // pending repair so a later save cannot copy the corrupt
                        // main over the only valid copy. The backup's bytes stay
                        // the single source until an explicit repair.
                        self.pending_recovery = true;
                        let revision = document.data.revision;
                        self.document = Some(document.clone());
                        self.latest_loaded_revision = Some(revision);
                        Ok(LoadOutcome::Recovered(document))
                    }
                    None => {
                        if main_bytes.is_some() {
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

    /// Decode the backup file; `Ok(None)` only when the backup is absent
    /// (`NotFound`) or present but not decodable. Any other read error is an
    /// inaccessible-file condition surfaced as [`RepositoryError::Io`], never
    /// mistaken for "no backup".
    fn try_recover_backup(&mut self) -> Result<Option<StoredDocumentV1>, RepositoryError> {
        let backup = self.backup();
        let backup_bytes = match self.io.read(&backup) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(RepositoryError::Io),
        };
        Ok(codec::decode(&backup_bytes).ok())
    }

    /// Explicit recovery: resolve the pending-repair state by preserving the
    /// corrupt main as durable evidence and promoting the valid backup onto
    /// the main.
    ///
    /// Contract:
    /// 1. re-read the main; if it now decodes (external repair), clear the
    ///    pending state and refresh the observed revision — nothing else;
    /// 2. otherwise the main is corrupt or absent: the backup must decode, or
    ///    this returns [`RepositoryError::CorruptData`] and leaves both the
    ///    main and the backup byte-untouched;
    /// 3. if a corrupt main existed, its bytes are first persisted to a new
    ///    distinct evidence file (`data.json.corrupt-<uuid>`) — durable proof
    ///    that is never overwritten;
    /// 4. promote the valid backup onto the main (write to a fresh temp,
    ///    sync, atomic rename) and best-effort fsync;
    /// 5. clear pending repair, set the in-memory document and
    ///    `latest_loaded_revision` from the repaired main; ordinary saves are
    ///    allowed again.
    pub fn repair_from_backup(&mut self) -> Result<RepairOutcome, RepositoryError> {
        if !self.pending_recovery {
            return Ok(RepairOutcome::HadNoCorruptMain);
        }

        let main = self.main();
        let main_bytes = match self.io.read(&main) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(RepositoryError::Io),
        };

        // Re-read the main: if it now decodes, an external actor already
        // repaired it — just clear the pending state.
        if let Some(bytes) = main_bytes.as_deref()
            && let Ok(document) = codec::decode(bytes)
        {
            self.pending_recovery = false;
            self.document = Some(document.clone());
            self.latest_loaded_revision = Some(document.data.revision);
            return Ok(RepairOutcome::HadNoCorruptMain);
        }

        // Main is absent or still corrupt: the backup must be valid. If it is
        // also absent or invalid, refuse and leave both untouched.
        let backup = self.backup();
        let backup_bytes = match self.io.read(&backup) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(RepositoryError::CorruptData);
            }
            Err(_) => return Err(RepositoryError::Io),
        };
        let backup_document =
            codec::decode(&backup_bytes).map_err(|_| RepositoryError::CorruptData)?;

        // Preserve the corrupt main as durable evidence before promoting the
        // backup (only when the main file actually existed).
        let evidence = match main_bytes {
            Some(corrupt_main) => {
                let token = Self::new_temp_token();
                let evidence_path = self.corrupt_evidence(&token);
                self.io.write_flush_sync(&evidence_path, &corrupt_main)?;
                Some(evidence_path)
            }
            None => None,
        };

        // Promotion uses the same write-then-rename discipline as a save, so a
        // failure cannot leave a half-written main.
        self.promote_bytes_to_main(&backup_bytes)?;

        self.pending_recovery = false;
        self.document = Some(backup_document.clone());
        self.latest_loaded_revision = Some(backup_document.data.revision);
        Ok(RepairOutcome::Repaired { evidence })
    }

    /// Write `bytes` to a fresh main temp and atomically rename it onto the
    /// main path. Used by `repair_from_backup` to promote the valid backup.
    fn promote_bytes_to_main(&mut self, bytes: &[u8]) -> Result<(), RepositoryError> {
        let token = Self::new_temp_token();
        let temp = self.temp(&token);
        self.io.write_flush_sync(&temp, bytes)?;
        let main = self.main();
        self.io.rename(&temp, &main)?;
        let _ = self.io.sync_path(&main);
        Ok(())
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

    /// Generate a unique temp-file token for one operation. Uses a fresh UUID
    /// so concurrent operations never collide on a temp name; the token is
    /// used only within a single call and never persisted.
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
            pending_recovery: false,
        }
    }
}

/// Classification of the main file's on-disk state for the revision guard.
/// The three cases are deliberately distinct (F002, F004): a missing main is
/// the only state that means "no baseline"; an undecodable main is corruption,
/// never a fabricatable baseline; any non-`NotFound` read error is an
/// inaccessible-file condition surfaced as `RepositoryError::Io`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnDiskRevision {
    Missing,
    Corrupt,
    Ok(u64),
}
