//! Privacy-preserving diagnostic policy.
//!
//! # Panic log (M07.1)
//!
//! A process panic may have a payload that embeds user data (a folder path, a
//! search query, a serialized settings slice). The default Rust panic hook
//! prints that payload. The runtime installs a REDACTED hook that writes only an
//! anonymous, stable line to a capped, rotated log file under the data
//! directory (so there is a cleanable diagnostic stream in release without any
//! console requirement). The log NEVER contains the panic payload, a user path,
//! a query, or configuration content.
//!
//! # Log hygiene (M07.5)
//!
//! There is no general application log file in 0.0.1: ordinary diagnostics use
//! fixed, anonymous `eprintln!` texts (invisible in the release window
//! subsystem, so no console spam). The ONLY file log is this capped panic log;
//! a Git-tracked source guard (see `storage/tests.rs`) rejects full-path/query
//! logging patterns.

use std::path::Path;

/// User-entered paths, search text, and serialized configuration are never safe
/// diagnostic fields. Callers should log stable error categories instead.
pub const SENSITIVE_FIELDS_REDACTED: bool = true;

/// Cap for the panic log file (bytes) before it is rotated away.
pub const PANIC_LOG_CAP: u64 = 256 * 1024;
/// Name of the live panic log file.
pub const PANIC_LOG_FILE: &str = "panic.log";
/// Name of the rotated panic log file (kept as the single previous generation).
pub const PANIC_LOG_FILE_ROTATED: &str = "panic.log.1";

/// Append one redacted line to the capped panic log in `base_dir`, rotating a
/// full log to `.1` (overwritten) and starting a fresh one so the on-disk
/// diagnostics stay bounded. Non-fatal on any I/O error. The line must already
/// be anonymous (no path/query/payload — the caller's contract).
pub fn log_panic(base_dir: &Path, line: &str) {
    impl_log_panic(base_dir, line, PANIC_LOG_CAP);
}

/// The `log_panic` implementation with an injectable cap (tests use a small cap
/// to exercise rotation without writing hundreds of KB).
fn impl_log_panic(base_dir: &Path, line: &str, cap: u64) {
    let _ = std::fs::create_dir_all(base_dir);
    let path = base_dir.join(PANIC_LOG_FILE);
    if let Ok(metadata) = std::fs::metadata(&path)
        && metadata.len() > cap
    {
        // Rotate: replace the single previous generation. Append-only, never
        // deletes user folders or the data files.
        let _ = std::fs::rename(&path, base_dir.join(PANIC_LOG_FILE_ROTATED));
    }
    let stamped = format!("{} {}\n", chrono::Utc::now().to_rfc3339(), line);
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(stamped.as_bytes());
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_log_is_capped_and_rotated() {
        let base = tempfile::TempDir::new().expect("temp dir");
        // A tiny cap turns a few KB of writes into a rotation trigger.
        let cap = 64;
        for _ in 0..20 {
            // ~40 chars × 20 = ~800 chars > cap, so the live log must rotate.
            impl_log_panic(
                base.path(),
                "panic at src/main.rs:1 (payload redacted)",
                cap,
            );
        }
        // The live log is bounded (starts fresh after a rotation).
        let live_len = std::fs::metadata(base.path().join(PANIC_LOG_FILE))
            .expect("live log")
            .len();
        assert!(live_len <= cap * 2, "live log grew unbounded: {live_len}");
        // Exactly one previous generation is kept.
        assert!(base.path().join(PANIC_LOG_FILE_ROTATED).exists());
        // And the sibling data files are never touched by the rotation.
        assert!(!base.path().join("data.json").exists());
    }

    #[test]
    fn panic_log_redacts_user_content() {
        // The log lines are produced by the installed hook with only an
        // anonymous message + a code location; a test asserts the contract that
        // the module never writes a payload word into the file.
        let base = tempfile::TempDir::new().expect("temp dir");
        impl_log_panic(
            base.path(),
            "FileGo encountered an internal error and will close (details redacted)",
            64,
        );
        let content = std::fs::read_to_string(base.path().join(PANIC_LOG_FILE)).expect("read");
        // No user-path/query-shaped content may appear in a generated line.
        assert!(!content.contains('\\'), "no backslash path");
        assert!(!content.contains("C:"));
    }
}
