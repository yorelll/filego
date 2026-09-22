//! Filesystem access seam with a fault-injection variant for tests.
//!
//! `DocumentRepository` talks exclusively through `FileOps`, so every step of
//! the persistence write path (temp create/write, sync, backup copy, atomic
//! rename) can be faulted deterministically. This keeps the persistence logic
//! unit-testable and lets the fault matrix prove that a failed step never
//! deletes the original document or its backup.

use std::{fs, io, path::Path};

/// The disk-access contract the repository relies on. [`FsFileOps`] is the
/// real implementation; `FaultyFileOps` (test-only, in the `fault` submodule)
/// injects errors without shipping its machinery in release builds.
///
/// Methods take `&mut self` so a fault-injecting test double can track its own
/// mutable step counter without interior mutability.
pub trait FileOps {
    /// Create (or truncate) the file, write `bytes`, flush, and sync to
    /// durable storage. On error no promise is made about the file's state,
    /// so callers treat a returned error as "temp file not ready".
    fn write_flush_sync(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Copy existing bytes of `from` into `to` (backup creation).
    fn copy(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    /// Atomically replace `to` with `from` on the same volume (main replace).
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>>;
    fn exists(&mut self, path: &Path) -> bool;
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

    fn copy(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::copy(from, to).map(|_| ())
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn exists(&mut self, path: &Path) -> bool {
        path.exists()
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
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

    /// The operation that is allowed to fail at an injected point.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FaultOp {
        /// Step 0 of a save: open/create the temp file and write its bytes.
        TempWrite,
        /// Step 1 of a save: flush and sync the temp file.
        TempSync,
        /// Step 2 of a save: probe whether a main file exists (before backup).
        MainExists,
        /// Step 3 of a save: copy the current main file to the backup path.
        BackupCopy,
        /// Step 4 of a save: atomic rename of the temp file onto the main path.
        ReplaceRename,
        /// Later steps of a save: best-effort removal of a stale temp sibling.
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
    }

    impl FileOps for FaultyFileOps {
        fn write_flush_sync(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            // Two injectable sub-steps: temp write then temp sync. Both tick
            // unconditionally (no short-circuit) so the counter stays in step
            // with the documented save sequence.
            let fail_write = self.tick(FaultOp::TempWrite);
            let fail_sync = self.tick(FaultOp::TempSync);
            if fail_sync {
                // Pretend the sync failed without touching the disk at all.
                return Err(io::Error::other("injected fault: temp sync"));
            }
            if fail_write {
                // Simulate a partial write: a real temp file with truncated
                // content is left behind, then the write reports failure.
                if let Ok(mut file) = fs::File::create(path) {
                    let _ = file.write_all(&bytes[..bytes.len() / 2]);
                    let _ = file.flush();
                }
                return Err(io::Error::other("injected fault: temp write"));
            }
            FsFileOps.write_flush_sync(path, bytes)
        }

        fn copy(&mut self, from: &Path, to: &Path) -> io::Result<()> {
            if self.tick(FaultOp::BackupCopy) {
                return Err(io::Error::other("injected fault: copy"));
            }
            FsFileOps.copy(from, to)
        }

        fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
            if self.tick(FaultOp::ReplaceRename) {
                return Err(io::Error::other("injected fault: rename"));
            }
            FsFileOps.rename(from, to)
        }

        fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
            // Read faulting is not part of the save fault matrix; load fault
            // tests drive corruption with real file content instead.
            FsFileOps.read(path)
        }

        fn exists(&mut self, path: &Path) -> bool {
            if self.tick(FaultOp::MainExists) {
                return false;
            }
            FsFileOps.exists(path)
        }

        fn remove(&mut self, path: &Path) -> io::Result<()> {
            if self.tick(FaultOp::StaleTempRemove) {
                return Err(io::Error::other("injected fault: remove"));
            }
            FsFileOps.remove(path)
        }
    }
}
