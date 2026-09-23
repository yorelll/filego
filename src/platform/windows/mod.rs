//! Windows integration boundary (M04).
//!
//! This is the ONLY module that touches the `windows`-crate FFI. Everything
//! policy-like lives in the pure `platform` siblings (`hotkey`, `ipc`,
//! `window_position`, `shell_open`) which this module turns into actual Win32
//! calls.
//!
//! # Architecture: hotkey / IPC worker thread
//!
//! Slint owns the winit event loop on the main thread; we must never run a
//! Win32 message pump on it. Instead a dedicated worker thread creates a hidden
//! top-level popup window (`FileGoHotkeyWindow`, styles `WS_EX_TOOLWINDOW`
//! and `WS_EX_NOACTIVATE`, never shown), runs
//! `GetMessage`/`TranslateMessage`/`DispatchMessage` for it, receives
//! `WM_HOTKEY` (global hotkey presses) and `WM_COPYDATA` (second-instance
//! activation) in that window's WndProc, and forwards a resolved action to the
//! UI thread through a `std::sync::mpsc` channel; the main thread drains it via
//! a polling timer on the event loop.
//!
//! Alternatives evaluated and rejected: `SetWindowLongPtrW`-subclassing Slint's
//! winit HWND (owns Slint's private wnd-proc chain, fragile across Slint
//! upgrades) and a named pipe (more moving parts than a hidden window and a
//! mutex and `WM_COPYDATA`). The hidden-window worker keeps the Slint event
//! loop untouched and the FFI fully isolated. No Win32 pump ever runs on the
//! Slint thread.
//!
//! # Single-instance
//!
//! A session-scoped named mutex `Local\FileGo.<suffix>` (suffix from
//! [`user_session`]) is created by the first instance; the second instance
//! discovers the primary's hidden window (`single_instance::activator`) and
//! sends the fixed `Show` payload, then exits — never a second tray icon.

pub mod clipboard;
pub mod folder_picker;
pub mod hotkey_adapter;
pub mod single_instance;
pub mod tray_open;
pub mod user_session;
pub mod window_focus;
pub mod window_placement;

use crate::domain::settings::HotkeySetting;
use crate::platform::hotkey::{HotkeyErrorKind, HotkeyMachine, HotkeyState};
use std::sync::mpsc::{Receiver, Sender};

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        CS_HREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        MSG, RegisterClassW, TranslateMessage, WM_COPYDATA, WM_DESTROY, WM_HOTKEY, WNDCLASSW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    },
};
use windows::core::PCWSTR;

use crate::platform::ipc::IpcRequest;

/// Public events the native layer resolves for the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeEvent {
    /// A global hotkey was pressed → show/toggle the search window.
    HotkeyShow,
    /// A second instance asked to activate us.
    ActivateFromSecondInstance,
}

/// Shared hotkey state for the worker (the machine is mutated from the main
/// thread via settings changes and read from the worker for event matching).
type SharedHotkey =
    std::sync::Arc<std::sync::Mutex<HotkeyMachine<hotkey_adapter::Win32HotkeyRegistry>>>;

/// Worker-thread entry: create the hidden top-level window, then pump messages
/// until the queue is torn down (WM_QUIT).
///
/// The PUBLICATION ORDER is the F001 contract: the shared HWND slot is written
/// ONLY after `CreateWindowExW` succeeds, so a `register` issued once the slot
/// is non-null always targets a fully-existing window (never the `Unavailable`
/// "no window yet" path). The worker returns `()` once the loop ends (F005:
/// graceful thread end, no `std::process::abort()`).
fn worker_main(
    events: Sender<NativeEvent>,
    hotkey: SharedHotkey,
    hwnd_slot: hotkey_adapter::HwndSlot,
) {
    // The wide strings must outlive the RegisterClassW/CreateWindowExW calls,
    // so they live in a scope-local Vec (never a dangling temporary pointer).
    let class_wide = wide_string(HIDDEN_WINDOW_CLASS);
    let class_name = PCWSTR::from_raw(class_wide.as_ptr());
    let instance = winapi_exe_hinstance();

    let class = WNDCLASSW {
        style: CS_HREDRAW,
        lpfnWndProc: Some(hotkey_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: Default::default(),
        hCursor: Default::default(),
        hbrBackground: Default::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: class_name,
    };
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        // Cannot register; the hidden window cannot come up.
        return;
    }

    // F002: an invisible TOP-LEVEL tool window, NOT a message-only window
    // (`HWND_MESSAGE`). Message-only windows are not enumerable by
    // `FindWindowW`/`EnumWindows`, so the second instance's activation
    // discovery could never find the primary. `WS_EX_TOOLWINDOW` keeps it out
    // of the taskbar / alt-tab; `WS_EX_NOACTIVATE` stops a stray activation;
    // we never call ShowWindow so it is never visible. It is, however, still a
    // top-level window that `FindWindowW` enumerates — the F002 fix.
    //
    // `WS_POPUP` (a top-level owned window) + no parent + no owner keeps a
    // zero-size icon-less handle that receives `WM_HOTKEY` / `WM_COPYDATA`.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name,
            class_name,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None, // top-level: not HWND_MESSAGE, no owning window
            None,
            Some(instance),
            None,
        )
    };
    let hwnd = match hwnd {
        Ok(hwnd) => hwnd,
        Err(_) => return,
    };

    // Register the WndProc context BEFORE publishing the HWND slot: events +
    // hotkey machine, keyed by thread id in a thread-local (the WndProc
    // receives only the HWND + a per-instance user-data slot; we stash the
    // shared state on the thread that owns hwnd). The hotkey machine itself
    // carries the pause gate (Paused → no events), so the WndProc does not need
    // a separate flag. Ordering this before the publish guarantees that the
    // moment the main thread observes the HWND, the WndProc context is fully
    // live (no early WM_HOTKEY / WM_COPYDATA can be dropped against an
    // unregistered thread-local).
    set_thread_state(hwnd, events.clone(), hotkey);

    // Publish the raw pointer bits (HWND is not Send; the slot is shared with
    // the main thread through `Arc<Mutex<Option<isize>>>`) ONLY now that the
    // window fully exists and its WndProc context is registered — the F001
    // ordering contract: a `register` issued once the slot is non-null always
    // targets a live, dispatched window (never the "window not created yet"
    // `Unavailable` path).
    *hwnd_slot.lock().expect("hwnd slot lock") = Some(hwnd.0 as isize);

    let mut msg = MSG::default();
    loop {
        // Safety: msg is valid for GetMessageW; None hwnd = all queue messages.
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        // -1 means an error; 0 means WM_QUIT.
        if result.0 == 0 || result.0 == -1 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
    // F005: return instead of `std::process::abort()`. The process is still
    // expected to exit via the main event loop (the tray shell owns its
    // lifetime); a worker that reaches WM_QUIT simply ends its thread and lets
    // the handles / window be torn down through normal RAII + DestroyWindow.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

/// Hidden-window WndProc: resolves `WM_HOTKEY` and `WM_COPYDATA` into
/// [`NativeEvent`]s forwarded on the event channel.
unsafe extern "system" fn hotkey_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            // lparam packs LOWORD=vk, HIWORD=modifiers (Windows docs).
            let vk = (lparam.0 & 0xFFFF) as u8;
            let modifiers_raw = ((lparam.0 >> 16) & 0xFFFF) as u32;
            let modifiers = decode_hotkey_modifiers(modifiers_raw);
            if let Some(state) = matching_state(hwnd) {
                let action = state
                    .hotkey
                    .lock()
                    .expect("hotkey lock")
                    .on_event(vk, modifiers);
                if action == Some(crate::platform::hotkey::HotkeyAction::Show) {
                    let _ = state.events.send(NativeEvent::HotkeyShow);
                }
            }
            LRESULT(0)
        }
        WM_COPYDATA => {
            let decoded = {
                // Safety: lparam points at a COPYDATASTRUCT valid during dispatch;
                // we copy the fixed 64 bytes out before anything can invalidate it.
                let copy_data =
                    std::ptr::NonNull::new(lparam.0 as *mut std::ffi::c_void).map(|ptr| unsafe {
                        &*(ptr.as_ptr()
                            as *const windows::Win32::System::DataExchange::COPYDATASTRUCT)
                    });
                match copy_data {
                    Some(data) => {
                        let mut buf = [0u8; crate::platform::ipc::PROTOCOL_LEN];
                        let len = (data.cbData as usize).min(buf.len());
                        if !data.lpData.is_null() {
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    data.lpData as *const u8,
                                    buf.as_mut_ptr(),
                                    len,
                                );
                            }
                        }
                        crate::platform::ipc::decode(&buf[..len])
                    }
                    None => IpcRequest::Ignored,
                }
            };
            if decoded == IpcRequest::Show
                && let Some(state) = matching_state(hwnd)
            {
                let _ = state.events.send(NativeEvent::ActivateFromSecondInstance);
                LRESULT(1) // handled, informs the sender
            } else {
                LRESULT(1)
            }
        }
        WM_DESTROY => {
            clear_thread_state(hwnd);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Decode a Win32 modifier word into the domain `HotkeyModifiers`.
fn decode_hotkey_modifiers(raw: u32) -> crate::domain::settings::HotkeyModifiers {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    };
    let _ = MOD_NOREPEAT;
    crate::domain::settings::HotkeyModifiers {
        control: raw & MOD_CONTROL.0 != 0,
        alt: raw & MOD_ALT.0 != 0,
        shift: raw & MOD_SHIFT.0 != 0,
        win: raw & MOD_WIN.0 != 0,
    }
}

// ------------------------- thread-local WndProc state ------------------------
//
// The WndProc runs on the worker thread; it receives only the HWND. The worker
// creates exactly ONE window on its thread, so we stash the event sender +
// shared hotkey machine in a single-slot thread-local (no HWND key needed).
// `HWND` is raw `*mut c_void` (not Hash/Eq/Send), so a map keyed on it would
// not even compile.

use std::cell::RefCell;

struct WndProcState {
    events: Sender<NativeEvent>,
    hotkey: SharedHotkey,
    hwnd: HWND,
}

thread_local! {
    static WNDPROC_STATE: RefCell<Option<WndProcState>> = const { RefCell::new(None) };
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_thread_state(hwnd: HWND, events: Sender<NativeEvent>, hotkey: SharedHotkey) {
    WNDPROC_STATE.with(|slot| {
        *slot.borrow_mut() = Some(WndProcState {
            events,
            hotkey,
            hwnd,
        });
    });
}

fn clear_thread_state(_hwnd: HWND) {
    WNDPROC_STATE.with(|slot| *slot.borrow_mut() = None);
}

fn matching_state(hwnd: HWND) -> Option<WndProcState> {
    WNDPROC_STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|state| state.hwnd == hwnd)
            .map(|state| WndProcState {
                events: state.events.clone(),
                hotkey: state.hotkey.clone(),
                hwnd: state.hwnd,
            })
    })
}

/// The hidden-window class shared with the activator.
pub const HIDDEN_WINDOW_CLASS: &str = "FileGoHotkeyWindow";

/// Get the current executable's HINSTANCE for window-class registration.
fn winapi_exe_hinstance() -> windows::Win32::Foundation::HINSTANCE {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    // Safety: `None` requests the current process's exe module; the returned
    // HMODULE is a static handle that stays valid for the process lifetime.
    unsafe {
        GetModuleHandleW(None)
            .map(|h| windows::Win32::Foundation::HINSTANCE(h.0))
            .unwrap_or_default()
    }
}

/// Minimal public adapter the main thread uses to drive the native worker.
///
/// Concrete enough that `main.rs` wires it without exposing Win32 types.
pub struct NativePlatform {
    events: Sender<NativeEvent>,
    receiver: Receiver<NativeEvent>,
    hwnd: Option<HWND>,
    hotkey: SharedHotkey,
    /// Pause gate shared with tray callbacks (the worker WndProc reads it; the
    /// UI thread toggles it). Also consulted by the pure hotkey machine.
    paused: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl NativePlatform {
    /// Start the worker thread and register the hotkey (if `settings.hotkey`
    /// is `Some` and valid). Returns self or an anonymous error.
    ///
    /// F001 ordering: the hotkey is registered ONLY AFTER the worker has
    /// published its real HWND. The worker writes the shared slot only once
    /// `CreateWindowExW` succeeded, so a registration issued after
    /// `wait_for_hwnd` always targets a live window — the startup race that used
    /// to yield a silent, permanent `Disabled` (the old `HotkeyMachine::new`
    /// before the window existed) is gone.
    ///
    /// The machine starts in a *pending* (not-yet-registered) `Disabled` state
    /// and only calls `set(hotkey_setting)` once the HWND is populated — the
    /// review's suggested option (b). Registration failures are surfaced as
    /// `Disabled + last_error` (never silently swallowed) and readable through
    /// [`Self::hotkey_state`] / [`Self::hotkey_last_error`]. A transient
    /// `Unavailable` result is retried once after a short delay before being
    /// recorded.
    pub fn start(hotkey_setting: Option<HotkeySetting>) -> Result<NativePlatform, NativeError> {
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        let hwnd_slot = hotkey_adapter::shared_hwnd_slot();
        let registry = hotkey_adapter::Win32HotkeyRegistry::new(hwnd_slot.clone());
        // Pending machine: no registration yet. The worker inherits the shared
        // reference (stashed into the WndProc thread-local) before we ever call
        // `set`, so a hotkey press can never arrive before the machine exists.
        let machine = HotkeyMachine::new(registry, None);
        let hotkey: SharedHotkey = std::sync::Arc::new(std::sync::Mutex::new(machine));
        let paused = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let thread_hotkey = hotkey.clone();
        let thread_events = events_tx.clone();
        let thread_slot = hwnd_slot.clone();
        std::thread::Builder::new()
            .name("filego-native-worker".to_owned())
            .spawn(move || worker_main(thread_events, thread_hotkey, thread_slot))
            .map_err(|_| NativeError::WorkerStartFailed)?;

        // Wait for the hidden window to come up BEFORE registering: this is the
        // happens-before edge that makes `register` deterministic (F001).
        let hwnd = wait_for_hwnd(&hwnd_slot, std::time::Duration::from_millis(2_000));
        if let Some(combo) = hotkey_setting {
            // Register once the HWND exists. If the window still did not come up
            // (`hwnd` is None), `set` resolves `register` → `Unavailable` and
            // records `last_error`, which the tray can surface.
            let mut hotkey_guard = hotkey.lock().expect("hotkey lock");
            if hotkey_guard.set(combo).is_err() {
                // Retry once after a short delay for a transient platform
                // failure (F001 retry-once path). A second `set` from a
                // `Disabled` state with the same combo just re-registers.
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = hotkey_guard.set(combo);
            }
        }

        Ok(NativePlatform {
            events: events_tx,
            receiver: events_rx,
            hwnd,
            hotkey,
            paused,
        })
    }

    /// The hidden window (present once the worker thread has created it).
    pub const fn hidden_window(&self) -> Option<HWND> {
        self.hwnd
    }

    /// Blocking receiver of resolved native events (the main thread drains it
    /// on a timer); exposed for tests.
    pub fn try_recv_event(&self) -> Option<NativeEvent> {
        self.receiver.try_recv().ok()
    }

    /// Channel on which the main thread can submit frames from a timer.
    pub fn event_sender(&self) -> Sender<NativeEvent> {
        self.events.clone()
    }

    /// Current hotkey state (for tray text).
    pub fn hotkey_state(&self) -> HotkeyState {
        self.hotkey.lock().expect("hotkey lock").state()
    }

    /// The last registration failure, if any. Lets the tray surface a
    /// `Disabled` hotkey (F001: registration failures are never silent).
    pub fn hotkey_last_error(&self) -> Option<HotkeyErrorKind> {
        self.hotkey.lock().expect("hotkey lock").last_error()
    }

    /// Apply a hotkey change (validate + re-register; keep-old on conflict).
    pub fn set_hotkey(
        &self,
        combo: HotkeySetting,
    ) -> Result<(), crate::platform::hotkey::HotkeyErrorKind> {
        self.hotkey.lock().expect("hotkey lock").set(combo)
    }

    /// Clear the hotkey entirely.
    pub fn clear_hotkey(&self) -> Result<(), crate::platform::hotkey::HotkeyErrorKind> {
        self.hotkey.lock().expect("hotkey lock").clear()
    }

    /// Pause the hotkey.
    pub fn pause_hotkey(&self) {
        self.hotkey.lock().expect("hotkey lock").pause();
    }

    /// Resume the hotkey.
    pub fn resume_hotkey(&self) {
        self.hotkey.lock().expect("hotkey lock").resume();
    }

    /// A no-op platform used when the worker cannot start: the tray shell still
    /// runs; hotkeys and single-instance IPC are simply unavailable.
    pub fn disabled() -> NativePlatform {
        let (tx, rx) = std::sync::mpsc::channel();
        let hwnd_slot = hotkey_adapter::shared_hwnd_slot();
        let registry = hotkey_adapter::Win32HotkeyRegistry::new(hwnd_slot.clone());
        let machine = HotkeyMachine::new(registry, None);
        NativePlatform {
            events: tx,
            receiver: rx,
            hwnd: None,
            hotkey: std::sync::Arc::new(std::sync::Mutex::new(machine)),
            paused: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Shared pause flag for the tray toggle (thread-safe).
    pub fn paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Toggle the hotkey pause state on/off.
    pub fn toggle_pause(&self) -> bool {
        let next = !self.paused();
        self.paused
            .store(next, std::sync::atomic::Ordering::Relaxed);
        if next {
            self.pause_hotkey();
        } else {
            self.resume_hotkey();
        }
        next
    }
}

/// Poll the shared HWND slot until the worker creates the window.
fn wait_for_hwnd(slot: &hotkey_adapter::HwndSlot, budget: std::time::Duration) -> Option<HWND> {
    let deadline = std::time::Instant::now() + budget;
    loop {
        let bits = *slot.lock().expect("hwnd slot");
        if let Some(bits) = bits {
            return Some(HWND(bits as *mut core::ffi::c_void));
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Anonymous native-layer failure (never a raw code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeError {
    WorkerStartFailed,
    WindowCreateFailed,
    MutexFailed,
    InvalidMutexName,
}

impl NativeError {
    pub const fn as_detail(self) -> &'static str {
        match self {
            NativeError::WorkerStartFailed => "the background worker could not be started",
            NativeError::WindowCreateFailed => "the hidden window could not be created",
            NativeError::MutexFailed => "the single-instance check could not run",
            NativeError::InvalidMutexName => "the single-instance mutex name is invalid",
        }
    }
}

// Re-export pieces the caller needs.
pub use hotkey_adapter::Win32HotkeyRegistry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_string_terminates_correctly() {
        assert_eq!(wide_string("FileGo"), vec![70, 105, 108, 101, 71, 111, 0]);
        assert_eq!(wide_string(""), vec![0]);
    }

    #[test]
    fn modifier_decoding_is_bidirectional() {
        let mods = crate::domain::settings::HotkeyModifiers {
            control: true,
            alt: true,
            shift: false,
            win: false,
        };
        let raw = (hotkey_adapter::hotkey_modifiers(mods).0 & 0xFFFF) as u32;
        // Drop NOREPEAT bits (they live above 0x10000 for window messages).
        let compact = raw & 0xF;
        assert_eq!(decode_hotkey_modifiers(compact), mods);
    }

    #[test]
    fn native_errors_are_anonymous() {
        for error in [
            NativeError::WorkerStartFailed,
            NativeError::WindowCreateFailed,
            NativeError::MutexFailed,
            NativeError::InvalidMutexName,
        ] {
            assert!(!error.as_detail().is_empty());
            assert!(!error.as_detail().contains("0x"));
        }
    }

    // F002 wiring invariant: the worker creates the hidden window with the
    // class `HIDDEN_WINDOW_CLASS`, and the second instance's activator looks it
    // up by the SAME class name via `FindWindowW`. If the two constants ever
    // diverge, second-instance activation silently breaks. The hidden window is
    // now a real top-level popup (never message-only), so `FindWindowW` can
    // enumerate it — the F002 root-cause fix. This test pins the shared,
    // discoverable class at the wiring layer (the FFI itself is not headless-
    // testable).
    #[test]
    fn hidden_window_class_is_shared_with_the_activator() {
        let worker_class = super::HIDDEN_WINDOW_CLASS;
        let activator_class =
            crate::platform::windows::single_instance::activator::HIDDEN_WINDOW_CLASS;
        assert_eq!(worker_class, activator_class);
        assert_eq!(worker_class, "FileGoHotkeyWindow");
    }
}
