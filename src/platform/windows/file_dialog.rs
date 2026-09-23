//! Native save/open file dialogs (M06.5 export/import).
//!
//! Uses the common item dialogs (`IFileSaveDialog` / `IFileOpenDialog`) so the
//! OS handles the overwrite prompt (`FOS_OVERWRITEPROMPT`) for exports — a
//! target file is NEVER overwritten without an explicit user confirmation — and
//! the native file picker for imports. The returned path is absolute and shown
//! to the user by the OS; it never enters logs.
//!
//! The pattern mirrors `folder_picker.rs` (M05): `SHCoCreateInstance` with the
//! stringized CLSID, `Show`, then `GetResult`/`GetResults` →
//! `GetDisplayName(SIGDN_FILESYSPATH)`.

use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, IFileOpenDialog, IFileSaveDialog,
    IShellItem, SHCoCreateInstance, SIGDN_FILESYSPATH,
};

/// Stringized `CLSID_FileOpenDialog`.
const FILE_OPEN_DIALOG_CLSID_STRING: &str = "{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}";
/// Stringized `CLSID_FileSaveDialog`.
const FILE_SAVE_DIALOG_CLSID_STRING: &str = "{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}";

/// Outcome of a native file dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDialogOutcome {
    /// The user confirmed; `path` is the absolute filesystem path of the file.
    Picked(String),
    /// The user cancelled.
    Cancelled,
    /// The dialog could not be shown (anonymous).
    Failed,
}

/// Show a native "save file" dialog with the OS's own overwrite prompt.
/// `default_name` is the suggested file name (e.g. `filego-export.json`).
pub fn pick_save_path(default_name: &str) -> FileDialogOutcome {
    let clsid_wide = wide_string(FILE_SAVE_DIALOG_CLSID_STRING);
    // Safety: `SHCoCreateInstance` on the UI thread with a NUL-terminated CLSID.
    let dialog: IFileSaveDialog = match unsafe {
        SHCoCreateInstance(
            windows::core::PCWSTR(clsid_wide.as_ptr()),
            None,
            None::<&windows::core::IUnknown>,
        )
    } {
        Ok(dialog) => dialog,
        Err(_) => return FileDialogOutcome::Failed,
    };

    // Safety: standard COM method calls on a live interface.
    unsafe {
        let options = dialog.GetOptions().unwrap_or_default();
        // FOS_OVERWRITEPROMPT is the OS-level "never overwrite silently" guard.
        let _ = dialog.SetOptions(options | FOS_OVERWRITEPROMPT);
        let name = wide_string(default_name);
        let _ = dialog.SetFileName(windows::core::PCWSTR(name.as_ptr()));
    }

    if unsafe { dialog.Show(None) }.is_err() {
        return FileDialogOutcome::Cancelled;
    }

    // Safety: after a successful Show, GetResult returns the chosen item.
    let item = match unsafe { dialog.GetResult() } {
        Ok(item) => item,
        Err(_) => return FileDialogOutcome::Cancelled,
    };
    path_of_item(&item)
}

/// Show a native "open file" dialog. `allowed_extension` (without the dot, e.g.
/// `json`) is a hint kept for a future `SetFileTypes` filter; the OS already
/// lets the user pick any file, so it is deliberately not mandatory. It is
/// referenced here to keep the API future-facing and to document the intent.
pub fn pick_open_path(allowed_extension: &str) -> FileDialogOutcome {
    let _ = allowed_extension;
    let clsid_wide = wide_string(FILE_OPEN_DIALOG_CLSID_STRING);
    // Safety: `SHCoCreateInstance` on the UI thread with a NUL-terminated CLSID.
    let dialog: IFileOpenDialog = match unsafe {
        SHCoCreateInstance(
            windows::core::PCWSTR(clsid_wide.as_ptr()),
            None,
            None::<&windows::core::IUnknown>,
        )
    } {
        Ok(dialog) => dialog,
        Err(_) => return FileDialogOutcome::Failed,
    };

    // Safety: standard COM method calls on a live interface.
    unsafe {
        let options = dialog.GetOptions().unwrap_or_default();
        let _ = dialog.SetOptions(options | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST);
    }

    if unsafe { dialog.Show(None) }.is_err() {
        return FileDialogOutcome::Cancelled;
    }

    // Safety: after a successful Show, GetResult returns the chosen item.
    let item = match unsafe { dialog.GetResult() } {
        Ok(item) => item,
        Err(_) => return FileDialogOutcome::Cancelled,
    };
    path_of_item(&item)
}

/// Resolve an `IShellItem` to an absolute filesystem path.
fn path_of_item(item: &IShellItem) -> FileDialogOutcome {
    // Safety: `GetDisplayName` returns a CoTaskMem-allocated PWSTR we free.
    let name = match unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) } {
        Ok(name) => name,
        Err(_) => return FileDialogOutcome::Cancelled,
    };
    let text = unsafe { name.to_string() }.unwrap_or_default();
    unsafe {
        windows::Win32::System::Com::CoTaskMemFree(Some(name.as_ptr().cast()));
    }
    if text.trim().is_empty() {
        FileDialogOutcome::Cancelled
    } else {
        FileDialogOutcome::Picked(text)
    }
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clsid_strings_are_the_known_common_dialog_guids() {
        // The stringized CLSIDs must match the well-known shell GUIDs; a typo
        // here silently breaks the dialog at runtime (SHCoCreateInstance fails).
        assert_eq!(
            FILE_OPEN_DIALOG_CLSID_STRING,
            "{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}"
        );
        assert_eq!(
            FILE_SAVE_DIALOG_CLSID_STRING,
            "{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}"
        );
    }

    #[test]
    fn wide_strings_terminate() {
        assert_eq!(wide_string("filego-export.json"), {
            let mut expect: Vec<u16> = "filego-export.json".encode_utf16().collect();
            expect.push(0);
            expect
        });
    }
}
