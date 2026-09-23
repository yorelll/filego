//! Native multi-folder picker (M05).
//!
//! Real Windows folder selection via the common item dialog:
//! `SHCoCreateInstance(CLSID_FileOpenDialog)` → `IFileOpenDialog`
//! (`FOS_PICKFOLDERS | FOS_ALLOWMULTISELECT` = a one-or-more folder picker) →
//! `Show` → `GetResults()` → `IShellItemArray` → per item
//! `IShellItem.GetDisplayName(SIGDN_FILESYSPATH)` which yields an absolute
//! filesystem path. No new `Cargo.toml` feature is needed: `SHCoCreateInstance`
//! lives in the already-enabled `Win32_UI_Shell` (shell32); the
//! `CLSID_FileOpenDialog` GUID is passed in string form through
//! `SHCoCreateInstance`'s `pszclsid` and is a private constant (windows 0.62
//! does not generate a named constant for it).
//!
//! This is the **only** place in the crate where a native dialog runs. The
//! picker is modal, user-initiated and never a scan.
//!
//! # Drag-drop (M05.2)
//!
//! Slint 1.18 does not forward the OS `DroppedFile` event to the Slint layer
//! (its winit backend drops those events), so OS drag-drop onto the window is a
//! documented seam: the pure presenter accepts an explicit path batch
//! (`crate::presentation::management::preview_batch`) that the UI can populate
//! from a future native drop hook or from pasted lines. The primary flows —
//! manual path, clipboard paste, and the native multi-folder picker — are fully
//! functional.

use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_PICKFOLDERS, IFileOpenDialog, IShellItemArray, SHCoCreateInstance,
    SIGDN_FILESYSPATH,
};

/// Stringized `CLSID_FileOpenDialog` (dc1c5a9c-e88a-4dde-a5a1-60f82a20aef7) for
/// `SHCoCreateInstance`'s `pszclsid` parameter.
const FILE_OPEN_DIALOG_CLSID_STRING: &str = "{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}";

/// Outcome of the native picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickOutcome {
    /// The user confirmed; `paths` are absolute, non-empty picks in display
    /// order (already resolved by `SIGDN_FILESYSPATH`).
    Picked(Vec<String>),
    /// The user cancelled (or the dialog was dismissed). No paths.
    Cancelled,
    /// The dialog could not be shown. Anonymous (no path carried).
    Failed,
}

/// Show the native multi-folder dialog.
pub fn pick_folder_dialog() -> PickOutcome {
    let clsid_wide: Vec<u16> = FILE_OPEN_DIALOG_CLSID_STRING
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // Safety: `SHCoCreateInstance` is called on the UI thread with a
    // NUL-terminated CLSID string and no outer unknown; the returned COM
    // object is owned by us and released on drop.
    let dialog: IFileOpenDialog = match unsafe {
        SHCoCreateInstance(
            windows::core::PCWSTR(clsid_wide.as_ptr()),
            None,
            None::<&windows::core::IUnknown>,
        )
    } {
        Ok(dialog) => dialog,
        Err(_) => return PickOutcome::Failed,
    };

    // Safety: `SetOptions`/`GetOptions` are ordinary COM method calls on a
    // live interface.
    unsafe {
        let options = dialog.GetOptions().unwrap_or_default();
        let _ = dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_ALLOWMULTISELECT);
    }

    // Safety: `Show` opens the modal dialog; the returned HRESULT honours the
    // user's OK/Cancel (a cancellation is indistinguishable from a stray error
    // through this API, so we map either to `Cancelled`).
    if unsafe { dialog.Show(None) }.is_err() {
        return PickOutcome::Cancelled;
    }

    // Safety: after a successful `Show`, `GetResults` returns the multi-select
    // array owned by the dialog; we enumerate and release each item through the
    // RAII `IShellItem`.
    let results = match unsafe { dialog.GetResults() } {
        Ok(results) => results,
        Err(_) => return PickOutcome::Cancelled,
    };
    collect_paths(&results)
}

fn collect_paths(results: &IShellItemArray) -> PickOutcome {
    // Safety: `GetCount`/`GetItemAt` are standard COM calls on a live array.
    let count = match unsafe { results.GetCount() } {
        Ok(count) => count,
        Err(_) => return PickOutcome::Cancelled,
    };
    let mut paths = Vec::new();
    for index in 0..count {
        let item = match unsafe { results.GetItemAt(index) } {
            Ok(item) => item,
            Err(_) => continue,
        };
        // `SIGDN_FILESYSPATH` yields an absolute filesystem path; a virtual
        // shell folder without a filesystem path errors here and is skipped.
        // Safety: the returned `PWSTR` is CoTaskMem-allocated by the shell and
        // must be freed by the caller with `CoTaskMemFree`.
        let name = match unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) } {
            Ok(name) => name,
            Err(_) => continue,
        };
        let text = unsafe { name.to_string() }.unwrap_or_default();
        unsafe {
            windows::Win32::System::Com::CoTaskMemFree(Some(name.as_ptr().cast()));
        }
        if !text.trim().is_empty() {
            paths.push(text);
        }
    }
    if paths.is_empty() {
        PickOutcome::Cancelled
    } else {
        PickOutcome::Picked(paths)
    }
}
