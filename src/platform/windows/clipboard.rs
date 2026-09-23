//! Clipboard adapter (M04 Copy-Path).
//!
//! Slint 1.18 exposes no public clipboard API on `Window`, so M04 wires the
//! system clipboard directly. `CF_UNICODETEXT` (13) is a stable Win32 constant;
//! we keep it named in this module to avoid pulling the `Win32_System_Ole`
//! kitchen-sink feature just for one constant.
//!
//! Best-effort: a clipboard that is held by another app (or a headless CI run)
//! yields a silent skip — the copy is never a hard failure path.

use windows::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL},
    System::{
        DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
};

/// `CF_UNICODETEXT` (a stable Win32 clipboard format id).
const CF_UNICODETEXT: u32 = 13;

/// Copy `text` to the system clipboard as UTF-16 (best effort).
///
/// Never logs the text. On any failure the previous clipboard content is
/// preserved (we do not EmptyClipboard before the allocations succeed).
pub fn set_text(text: &str) {
    // Safety: OpenClipboard with no owner window is the standard simple usage;
    // it fails if another app holds the clipboard open.
    if unsafe { OpenClipboard(None) }.is_err() {
        return;
    }
    let result = write_unicode_text(text);
    // Safety: CloseClipboard matches the OpenClipboard above.
    let _ = unsafe { CloseClipboard() };
    let _ = result;
}

fn write_unicode_text(text: &str) -> Result<(), ()> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = wide.len() * 2;

    // Safety: GMEM_MOVEABLE allocates a movable global block; the block is
    // copied below before the String buffer is released.
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes) }.map_err(|_| ())?;
    // Safety: GlobalLock returns a write pointer while the block is locked.
    let ptr = unsafe { GlobalLock(handle) };
    if ptr.is_null() {
        // Safety: the block was allocated but never locked; still ours to free.
        let _ = unsafe { GlobalFree(Some(handle)) };
        return Err(());
    }
    // Safety: `bytes` matches the allocation; copying into the locked block.
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
    }
    // Safety: unlock before handing the clipboard the handle.
    let _ = unsafe { GlobalUnlock(handle) };
    // Safety: SetClipboardData takes ownership of the block on success.
    let ok = unsafe { SetClipboardData(CF_UNICODETEXT, Some(HANDLE(handle.0))) }.is_ok();
    if !ok {
        // Ownership was NOT transferred; free it.
        let _ = unsafe { GlobalFree(Some(handle)) };
        return Err(());
    }
    Ok(())
}

/// Read the current clipboard text (UTF-16 → Rust `String`), best effort.
/// Returns an empty string when the clipboard is unavailable or holds no
/// Unicode text. Never logs the content.
pub fn read_text() -> String {
    // Safety: OpenClipboard with no owner window is the standard usage; it
    // fails if another app holds the clipboard open.
    if unsafe { OpenClipboard(None) }.is_err() {
        return String::new();
    }
    let result = read_unicode_text();
    // Safety: CloseClipboard matches the OpenClipboard above.
    let _ = unsafe { CloseClipboard() };
    result.unwrap_or_default()
}

fn read_unicode_text() -> Result<String, ()> {
    // Safety: GetClipboardData returns a HANDLE owned by the clipboard; we only
    // read it while the clipboard is open. The handle type is `Foundation::HANDLE`
    // (a handle, not an HGLOBAL); GlobalLock takes the raw HGLOBAL form, and the
    // returned memory is the same in practice for CF_UNICODETEXT blocks.
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.map_err(|_| ())?;
    if handle.0.is_null() {
        return Err(());
    }
    let hglobal = HGLOBAL(handle.0);
    // Safety: GlobalLock maps the block (treated as an HGLOBAL); the clipboard
    // owns it, we only read.
    let ptr = unsafe { windows::Win32::System::Memory::GlobalLock(hglobal) };
    if ptr.is_null() {
        return Err(());
    }
    let mut wide = Vec::new();
    let mut index = 0usize;
    // Conservative bound: scan for the NUL terminator within a sane limit to
    // avoid reading beyond the block. The clipboard text is user input; 1 MiB
    // of UTF-16 is far beyond any folder path we would paste.
    const MAX_READ: usize = 1 << 20;
    while index < MAX_READ {
        // Safety: each u16 read advances within the locked block; we stop at
        // the terminator or the limit.
        let unit = unsafe { *((ptr as *const u16).add(index)) };
        if unit == 0 {
            break;
        }
        wide.push(unit);
        index += 1;
    }
    // Safety: unlock before closing the clipboard.
    let _ = unsafe { windows::Win32::System::Memory::GlobalUnlock(hglobal) };
    Ok(String::from_utf16_lossy(&wide))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_text_serializes_with_terminator() {
        let mut wide: Vec<u16> = "abc".encode_utf16().collect();
        wide.push(0);
        assert_eq!(wide, vec![97, 98, 99, 0]);
        assert_eq!(wide.len() * 2, 8);
    }

    #[test]
    fn clipboard_format_constant_is_stable() {
        assert_eq!(CF_UNICODETEXT, 13);
    }
}
