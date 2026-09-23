//! Window placement boundary (M04.4).
//!
//! Only the monitor/DPI probes are FFI here; the geometry math is the pure
//! [`crate::platform::window_position`] module. `placement_rect` returns the
//! physical rect to hand to Slint's `Window::set_position` / `set_size`.

use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{
        GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    },
    UI::{HiDpi::GetDpiForMonitor, WindowsAndMessaging::GetCursorPos},
};

use crate::platform::window_position::{self, MonitorRect, WindowRect};

/// Compute the physical window rect for the search window.
///
/// `window_width/height` are the PHYSICAL (DPI-scaled) window size the caller
/// computes from Slint's logical size × scale factor. Placement follows the
/// pure rules (cursor monitor, centered, top ≈ 10%, clamped inside its work
/// area).
pub fn placement_rect(window_physical_width: i32, window_physical_height: i32) -> WindowRect {
    let (cursor, monitors) = placement_inputs();
    let cursor = cursor.unwrap_or((0, 0));
    window_position::compute_position(
        cursor,
        &monitors,
        window_physical_width,
        window_physical_height,
    )
    .unwrap_or(WindowRect {
        x: 0,
        y: 0,
        width: window_physical_width.max(1),
        height: window_physical_height.max(1),
    })
}

/// Gather the placement inputs: cursor position + the work-area rectangle of
/// the monitor under the cursor (nearest if the cursor is outside every
/// monitor); the primary work area is used as a fallback.
pub fn placement_inputs() -> (Option<(i32, i32)>, Vec<MonitorRect>) {
    let cursor = cursor_position();
    let mut monitors = Vec::new();
    if let Some((x, y)) = cursor
        && let Some(work) = monitor_work_area_at(x, y)
    {
        monitors.push(work);
    }
    if monitors.is_empty()
        && let Some(work) = monitor_work_area_at(0, 0)
    {
        monitors.push(work);
    }
    (cursor, monitors)
}

/// Physical cursor position.
pub fn cursor_position() -> Option<(i32, i32)> {
    // Safety: point is a valid out-param; ignore failures (multi-monitor edge).
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

/// Work area of the monitor containing `(x, y)` (nearest if none contains it).
/// Returns the work-area rect from `rcWork`, clamped to the monitor rect when
/// the work area is degenerate.
pub fn monitor_work_area_at(x: i32, y: i32) -> Option<MonitorRect> {
    // Safety: standard two-call query pattern on a borrowed stack struct.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor == HMONITOR::default() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let mut work = MonitorRect {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
            if work.width() <= 0 || work.height() <= 0 {
                work = MonitorRect {
                    left: info.rcMonitor.left,
                    top: info.rcMonitor.top,
                    right: info.rcMonitor.right,
                    bottom: info.rcMonitor.bottom,
                };
            }
            Some(work)
        } else {
            None
        }
    }
}

/// The effective DPI scale of the monitor under `(x, y)` (from
/// `GetDpiForMonitor`), used to convert Slint logical sizes to physical px.
/// Falls back to 1.0 (96 DPI) when unavailable.
pub fn dpi_scale_at(x: i32, y: i32) -> f32 {
    // Safety: standard DPI query; out-params are valid.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor == HMONITOR::default() {
            return 1.0;
        }
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(
            monitor,
            windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
            &mut dpi_x,
            &mut dpi_y,
        );
        let scale = (dpi_x.max(1) as f32) / 96.0;
        if scale > 0.0 && scale.is_finite() {
            scale
        } else {
            1.0
        }
    }
}

/// Scale a Slint logical window size into physical pixels for the monitor under
/// the cursor.
pub fn physical_size_for_cursor(logical_width: f32, logical_height: f32) -> (i32, i32) {
    let (x, y) = cursor_position().unwrap_or((0, 0));
    let scale = dpi_scale_at(x, y);
    (
        (logical_width * scale).round() as i32,
        (logical_height * scale).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_scale_is_sane_without_a_monitor() {
        // Headless CI: whatever the environment reports must be positive/finite
        // (we clamp internally), never NaN/0.
        let scale = dpi_scale_at(-1_000_000, -1_000_000);
        assert!(scale > 0.0 && scale.is_finite());
    }
}
