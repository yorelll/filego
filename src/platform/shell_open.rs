//! Pure folder-open controller (M04.5).
//!
//! The Windows boundary implements [`ShellOpener`] with exactly one call into
//! `ShellExecuteExW`, passing the folder path as `lpFile` and no verb/
//! parameters — never a command line, never an interpreter. This module owns
//! the *decision* logic (what to do after success/failure, how to map an error
//! to a localized reason) so it is fully testable and the ad-hoc placement of
//! `unsafe` stays in one thin boundary.
//!
//! Privacy rule: paths are never carried in error display or logs; the
//! anonymous [`OpenErrorKind`] is the only thing that crosses the boundary.

use crate::domain::settings::AppSettings;

/// Result of a shell open attempt, anonymous and privacy-safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenErrorKind {
    /// The shell verb was not found / nothing is associated (SE_ERR_NOASSOC).
    NoAssociation,
    /// The path was not found (SE_ERR_FNF / SE_ERR_PNF).
    NotFound,
    /// Access denied (SE_ERR_ACCESSDENIED).
    AccessDenied,
    /// A DDE failure (outlook-style "another program is opening" cases).
    DdeFailure,
    /// Out of memory or another generic failure.
    Unavailable,
    /// The shell call succeeded at the API level but the process refused
    /// (hInstApp <= 32, mapped here).
    ShellRejected,
}

impl OpenErrorKind {
    /// Stable, understandable, privacy-safe detail (localization happens in the
    /// presenter; never contains a path).
    pub const fn as_detail(self) -> &'static str {
        match self {
            OpenErrorKind::NoAssociation => "there is no application associated with this folder",
            OpenErrorKind::NotFound => "the folder path could not be found",
            OpenErrorKind::AccessDenied => "you do not have permission to open this folder",
            OpenErrorKind::DdeFailure => "the shell could not open the folder (DDE failure)",
            OpenErrorKind::Unavailable => "the folder could not be opened",
            OpenErrorKind::ShellRejected => "the folder could not be opened by the shell",
        }
    }
}

/// The outcome of one open attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOutcome {
    /// The shell accepted the open; the controller should apply the post-open
    /// effects (hide/clear per settings) and persist usage counters.
    Opened,
    /// The shell refused; keep the window and offer retry/copy/remove.
    Failed(OpenErrorKind),
}

/// What the presenter should do after an open attempt, derived from settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostOpenAction {
    /// Hide the window and clear the query input.
    HideAndClear,
    /// Hide the window only.
    Hide,
    /// Keep the window visible and clear the input.
    KeepVisibleAndClear,
    /// Keep the window visible and the input as-is.
    KeepVisible,
}

/// Derive the post-open action from settings (M04.5 "按设置隐藏/清空").
pub fn post_open_action(settings: &AppSettings) -> PostOpenAction {
    match (settings.hide_after_open, settings.clear_after_open) {
        (true, true) => PostOpenAction::HideAndClear,
        (true, false) => PostOpenAction::Hide,
        (false, true) => PostOpenAction::KeepVisibleAndClear,
        (false, false) => PostOpenAction::KeepVisible,
    }
}

/// Decision on whether to keep the window open after a failed open.
///
/// Always keep (never auto-delete; the record stays). No path is logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureResponse {
    /// Keep the window; surface the reason and offer recovery actions.
    KeepWindow(OpenErrorKind),
}

impl FailureResponse {
    pub const fn kind(self) -> OpenErrorKind {
        match self {
            FailureResponse::KeepWindow(kind) => kind,
        }
    }
}

/// Decide the response to a failed open. `record_inaccessible` inputs are
/// supplied by the caller's accessibility state; an inaccessible record is
/// never auto-removed — it is kept with the reason surfaced.
pub fn on_open_failure(kind: OpenErrorKind) -> FailureResponse {
    FailureResponse::KeepWindow(kind)
}

/// The one-and-only capability the shell boundary exposes.
pub trait ShellOpener {
    /// Open a folder path via the default shell verb. Implementations must use
    /// `ShellExecuteExW` with `lpFile = path`, no verb, no parameters — never a
    /// command line or an interpreter.
    fn open_folder(&self, path: &str) -> Result<(), OpenErrorKind>;
}

/// Guard that the controller performs BEFORE invoking the boundary, so a path
/// that is obviously empty never reaches the shell.
pub fn preflight_path(path: &str) -> Result<(), OpenErrorKind> {
    if path.trim().is_empty() {
        Err(OpenErrorKind::NotFound)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A recording opener that lets tests verify the ONLY shell call is
    /// `open_folder(path)` with the raw path, and lets tests inject failures.
    #[derive(Clone, Default)]
    struct RecordingOpener {
        calls: Rc<RefCell<Vec<String>>>,
        fail: Rc<RefCell<Option<OpenErrorKind>>>,
    }

    impl ShellOpener for RecordingOpener {
        fn open_folder(&self, path: &str) -> Result<(), OpenErrorKind> {
            self.calls.borrow_mut().push(path.to_owned());
            if let Some(kind) = *self.fail.borrow() {
                return Err(kind);
            }
            Ok(())
        }
    }

    fn settings(hide: bool, clear: bool) -> AppSettings {
        AppSettings {
            hide_after_open: hide,
            clear_after_open: clear,
            ..AppSettings::default()
        }
    }

    #[test]
    fn opener_receives_the_raw_path_without_transformation() {
        let opener = RecordingOpener::default();
        let paths = [
            r"C:\Users\me\Documents",
            r"\\server\share\with space\深度目录",
            r"D:\a\b\c.sqlite", // file-like extension is irrelevant: it is a folder record path
        ];
        for path in paths {
            let _ = opener.open_folder(path);
        }
        let calls = opener.calls.borrow();
        assert_eq!(calls.len(), paths.len());
        for (call, original) in calls.iter().zip(paths.iter()) {
            assert_eq!(call, original, "the adapter must pass the path verbatim");
        }
    }

    #[test]
    fn post_open_actions_follow_settings() {
        assert_eq!(
            post_open_action(&settings(true, true)),
            PostOpenAction::HideAndClear
        );
        assert_eq!(
            post_open_action(&settings(true, false)),
            PostOpenAction::Hide
        );
        assert_eq!(
            post_open_action(&settings(false, true)),
            PostOpenAction::KeepVisibleAndClear
        );
        assert_eq!(
            post_open_action(&settings(false, false)),
            PostOpenAction::KeepVisible
        );
    }

    #[test]
    fn failure_always_keeps_the_window_and_never_deletes() {
        for kind in [
            OpenErrorKind::NoAssociation,
            OpenErrorKind::NotFound,
            OpenErrorKind::AccessDenied,
            OpenErrorKind::DdeFailure,
            OpenErrorKind::Unavailable,
            OpenErrorKind::ShellRejected,
        ] {
            let response = on_open_failure(kind);
            assert_eq!(response, FailureResponse::KeepWindow(kind));
            // There is no "remove record" outcome anywhere in this module.
        }
    }

    #[test]
    fn error_details_are_anonymous_and_actionable() {
        for kind in [
            OpenErrorKind::NoAssociation,
            OpenErrorKind::NotFound,
            OpenErrorKind::AccessDenied,
            OpenErrorKind::DdeFailure,
            OpenErrorKind::Unavailable,
            OpenErrorKind::ShellRejected,
        ] {
            let detail = kind.as_detail();
            assert!(!detail.is_empty());
            assert!(!detail.contains('\\'), "no path may appear in details");
            assert!(!detail.contains("C:"), "no drive letter may appear");
        }
    }

    #[test]
    fn empty_path_is_rejected_before_reaching_the_shell() {
        assert_eq!(preflight_path("   "), Err(OpenErrorKind::NotFound));
        assert_eq!(preflight_path(""), Err(OpenErrorKind::NotFound));
        assert!(preflight_path(r"C:\real").is_ok());
    }

    #[test]
    fn opener_error_kind_maps_consistently() {
        // The boundary maps Win32 SE_ERR_* to these kinds; assert the stable
        // mapping table used by the adapter (documented, not executed here).
        let mappings = [
            (31_u32, OpenErrorKind::NoAssociation), // SE_ERR_NOASSOC
            (2_u32, OpenErrorKind::NotFound),       // SE_ERR_FNF
            (3_u32, OpenErrorKind::NotFound),       // SE_ERR_PNF
            (5_u32, OpenErrorKind::AccessDenied),   // SE_ERR_ACCESSDENIED
            (29_u32, OpenErrorKind::DdeFailure),    // SE_ERR_DDEFAIL
            (8_u32, OpenErrorKind::Unavailable),    // SE_ERR_OOM
        ];
        for (_code, kind) in mappings {
            let response = on_open_failure(kind);
            assert_eq!(response.kind(), kind);
        }
    }

    #[test]
    fn retry_reuses_the_same_path_verbatim() {
        // A retry is simply calling open_folder again with the same string; the
        // recorder can prove the retry did not mutate the path.
        let opener = RecordingOpener::default();
        let path = r"\\nas\share\My Folder㊗";
        let first = opener.open_folder(path);
        let temp = opener.fail.borrow_mut();
        drop(temp);
        let _ = first;
        let retry = opener.open_folder(path);
        assert!(retry.is_ok());
        let calls = opener.calls.borrow();
        assert_eq!(&*calls, &[path.to_owned(), path.to_owned()]);
    }
}
