//! Windows integration boundary.
//!
//! Slint's Windows backend owns the native event pump and built-in system tray
//! implementation in M00. Hotkeys, single-instance IPC, startup registration,
//! shell opening, and monitor placement are added behind this module in M04.

pub const TARGET_DESCRIPTION: &str = "Windows 10 22H2 and Windows 11 (x86-64)";
