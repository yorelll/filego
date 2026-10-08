//! Resolve the current user's FileGo data directory from the Windows known
//! folder, not a process environment variable or a temporary build directory.

use std::path::PathBuf;

use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

pub fn current_user_data_dir() -> Option<PathBuf> {
    // Safety: the shell returns a CoTaskMem-allocated UTF-16 path; we free it
    // immediately after decoding. No network, directory enumeration, or file
    // content is accessed by this known-folder lookup.
    let path =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }.ok()?;
    let decoded = unsafe { path.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(path.as_ptr().cast())) };
    decoded
        .filter(|value| !value.is_empty())
        .map(|base| PathBuf::from(base).join("FileGo"))
}
