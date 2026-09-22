//! Filesystem access seam with a fault-injection variant for tests.
//!
//! `DocumentRepository` talks exclusively through `FileOps`, so every step of
//! the persistence write path (temp create/write, sync, backup replace, atomic
//! main rename, write-lock acquisition) can be faulted deterministically. This
//! keeps the persistence logic unit-testable and lets the fault matrix prove
//! that a failed step never deletes the original document or its backup.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The disk-access contract the repository relies on. [`FsFileOps`] is the
/// real implementation; `FaultyFileOps` (test-only, in the `fault` submodule)
/// injects errors without shipping its machinery in release builds.
///
/// Methods take `&mut self` so a fault-injecting test double can track its own
/// mutable step counter without interior mutability. The trait is `Send` so a
/// test double behind `Box<dyn FileOps>` can be moved across threads to
/// simulate two processes contending on one base directory.
pub trait FileOps: Send {
    /// Create (or truncate) the file, write `bytes`, flush, and sync to
    /// durable storage. On error no promise is made about the file's state,
    /// so callers treat a returned error as "temp file not ready".
    fn write_flush_sync(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Atomically replace `to` with `from` on the same volume (main replace /
    /// backup replace). On Windows the rename replaces the destination.
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    /// Read the file's bytes in full. `NotFound` errors are meaningful to
    /// callers (used for "no file yet"); every other error is surfaced, never
    /// swallowed, so an inaccessible file is never mistaken for a missing one.
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>>;
    /// Whether `path` exists. Errors (access denied, an offline store, a
    /// broken parent link for a reason other than absence) are returned, so an
    /// inaccessible file is never treated as if it did not exist.
    fn exists(&mut self, path: &Path) -> io::Result<bool>;
    /// Create `path` exclusively; fails with `ErrorKind::AlreadyExists` when a
    /// file already exists there (used for the per-directory write lock).
    fn create_new(&mut self, path: &Path) -> io::Result<()>;
    /// Delete the file at `path`, tolerating `NotFound` (best-effort cleanup).
    fn remove(&mut self, path: &Path) -> io::Result<()>;
    /// Best-effort fsync of `path`'s current bytes (used after the atomic
    /// replace, for directory consistency). The default implementation opens
    /// and syncs the file; a fault-injecting double may simply leave it
    /// behaving realistically. Callers treat an error as non-fatal.
    fn sync_path(&mut self, path: &Path) -> io::Result<()> {
        fs::File::open(path)?.sync_all()
    }
}

/// Real filesystem implementation used by `DocumentRepository::new`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FsFileOps;

impl FileOps for FsFileOps {
    fn write_flush_sync(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let mut file = fs::File::create(path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn exists(&mut self, path: &Path) -> io::Result<bool> {
        fs::metadata(path).map(|_| true).or_else(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
    }

    fn create_new(&mut self, path: &Path) -> io::Result<()> {
        fs::File::create_new(path).map(|_| ())
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// Cross-process writer exclusion for one base directory.
///
/// 0.0.1 runs a single process, but the repository honours a per-directory
/// write lock so two processes sharing a data directory can never interleave a
/// check-then-rename (F003). The lock is a `data.json.lock` sibling created
/// with exclusive (`File::create_new`) semantics, held for the whole
/// revision-check → backup → rename → cleanup window, and removed on drop.
///
/// Stale-lock recovery (a crashed writer left the file behind) is a
/// manual/next-slice concern for 0.0.1 and is deliberately NOT auto-expired:
/// an automatic expiry would silently break the exclusion the lock provides.
/// A leftover lock simply keeps later writers failing with
/// `ConcurrentModification` until the user removes it.
pub(crate) struct WriteLock {
    path: PathBuf,
}

impl WriteLock {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        // Best-effort: a failed removal leaves a stale lock that a later
        // writer resolves manually (see the stale-lock note above).
        let _ = fs::remove_file(&self.path);
    }
}

/// Fault-injection seam (test-only; excluded from release builds).
///
/// Every injectable sub-operation consumes exactly one step of a shared
/// counter. A [`FaultPoint`] whose `step` equals the counter value when the
/// matching operation runs makes that operation return an injected error
/// instead of touching the real filesystem. The repository's call order is
/// fixed, so a test predicts each step's identity by counting the sub-steps
/// `save` performs. The counter is purely observational; fault points never
/// alter repository behavior on steps they do not target.
#[cfg(test)]
pub mod fault {
    use std::{
        fs,
        io::{self, Write},
        path::Path,
    };

    use super::{FileOps, FsFileOps};
    use crate::storage::location::BACKUP_TEMP_FILE_PREFIX;

    /// The operation that is allowed to fail at an injected point.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FaultOp {
        /// Step 0 of a save: acquire the per-directory write lock.
        LockAcquire,
        /// Main-temp write then sync (steps 1,2 of a save).
        TempWrite,
        TempSync,
        /// Probe whether a main file exists (step 3 of a save).
        MainExists,
        /// Backup-temp write then sync (steps 4,5 of a save).
        BackupWrite,
        BackupSync,
        /// Rename of the backup temp onto the live backup (step 6 of a save).
        BackupReplace,
        /// Atomic rename of the main temp onto the main path (step 7 of a
        /// save; step 4 when no main exists and the backup steps are skipped).
        ReplaceRename,
        /// Best-effort removal of a stale main-temp sibling during cleanup.
        StaleTempRemove,
    }

    /// A single injected failure: fail `op` at `step`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FaultPoint {
        pub step: usize,
        pub op: FaultOp,
    }

    /// Test double that fails exactly the configured operations at the
    /// configured step indices and otherwise delegates to the real
    /// implementation. Is not usable on steps it does not target; besides test
    /// assertions there are no panic paths (misconfiguration just leaves the
    /// counter unconsumed).
    #[derive(Debug)]
    pub struct FaultyFileOps {
        step: usize,
        points: Vec<FaultPoint>,
    }

    impl FaultyFileOps {
        pub fn new() -> Self {
            Self {
                step: 0,
                points: Vec::new(),
            }
        }

        pub fn inject(mut self, point: FaultPoint) -> Self {
            self.points.push(point);
            self
        }
    }

    impl Default for FaultyFileOps {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FaultyFileOps {
        /// Consume one step. Returns true when the point for `op` sits on the
        /// counter value that has just been consumed, asking the caller to
        /// pretend the operation failed.
        fn tick(&mut self, op: FaultOp) -> bool {
            let current = self.step;
            self.step += 1;
            self.points
                .iter()
                .any(|point| point.step == current && point.op == op)
        }

        /// Whether `path` is a backup temp (`data.json.bak.tmp.*`); used to
        /// route the write/sync/rename faults to the backup or the main temp.
        fn is_backup_temp(path: &Path) -> bool {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(BACKUP_TEMP_FILE_PREFIX))
        }
    }

    impl FileOps for FaultyFileOps {
        fn write_flush_sync(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            // Two injectable sub-steps per call: write then sync. Both tick
            // unconditionally (no short-circuit) so the counter stays in step
            // with the documented save sequence.
            let is_backup = Self::is_backup_temp(path);
            let (write_op, sync_op) = if is_backup {
                (FaultOp::BackupWrite, FaultOp::BackupSync)
            } else {
                (FaultOp::TempWrite, FaultOp::TempSync)
            };
            let fail_write = self.tick(write_op);
            let fail_sync = self.tick(sync_op);
            if fail_sync {
                // Pretend the sync failed without touching the disk at all.
                let label = if is_backup {
                    "backup temp sync"
                } else {
                    "temp sync"
                };
                return Err(io::Error::other(format!("injected fault: {label}")));
            }
            if fail_write {
                // Simulate a partial write: a real file with truncated
                // content is left behind, then the write reports failure.
                if let Ok(mut file) = fs::File::create(path) {
                    let _ = file.write_all(&bytes[..bytes.len() / 2]);
                    let _ = file.flush();
                }
                let label = if is_backup {
                    "backup temp write"
                } else {
                    "temp write"
                };
                return Err(io::Error::other(format!("injected fault: {label}")));
            }
            FsFileOps.write_flush_sync(path, bytes)
        }

        fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
            let op = if Self::is_backup_temp(from) {
                FaultOp::BackupReplace
            } else {
                FaultOp::ReplaceRename
            };
            if self.tick(op) {
                return Err(io::Error::other("injected fault: rename"));
            }
            FsFileOps.rename(from, to)
        }

        fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
            // Read faulting is not part of the save fault matrix; load and
            // revision-guard I/O-classification tests drive those errors with
            // a dedicated double in `repository_tests`, not this counter.
            FsFileOps.read(path)
        }

        fn exists(&mut self, path: &Path) -> io::Result<bool> {
            if self.tick(FaultOp::MainExists) {
                return Ok(false);
            }
            FsFileOps.exists(path)
        }

        fn create_new(&mut self, path: &Path) -> io::Result<()> {
            if self.tick(FaultOp::LockAcquire) {
                return Err(io::Error::other("injected fault: lock acquire"));
            }
            FsFileOps.create_new(path)
        }

        fn remove(&mut self, path: &Path) -> io::Result<()> {
            if self.tick(FaultOp::StaleTempRemove) {
                return Err(io::Error::other("injected fault: remove"));
            }
            FsFileOps.remove(path)
        }
    }
}
