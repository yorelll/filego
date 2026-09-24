//! Window placement boundary (M04.4).
//!
//! Only the monitor/DPI probes are FFI here; the geometry math is the pure
//! [`crate::platform::window_position`] module. `placement_rect` returns the
//! physical rect to hand to Slint's `Window::set_position` / `set_size`.

use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
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

/// Pure placement projection for per-monitor DPI (M04.4 / F003).
///
/// Scales `logical_width/height` by the TARGET monitor's `dpi_scale` (the
/// monitor under `cursor`) and applies the pure placement rules against that
/// monitor's `work_area`. This is the correct sizing for placement: the window
/// is about to be shown on the target monitor and renders at that monitor's
/// DPI, so centering/clamping must use `logical × target_scale`, never the
/// window's *current* `scale_factor()` (which may describe a different
/// monitor in mixed-DPI layouts).
///
/// Degenerate inputs are defended: a non-finite/non-positive scale falls back
/// to 1.0 and sizes are clamped to at least 1 physical pixel.
pub fn physical_rect_for_monitor(
    cursor: (i32, i32),
    work_area: MonitorRect,
    dpi_scale: f32,
    logical_width: f32,
    logical_height: f32,
) -> WindowRect {
    let scale = if dpi_scale > 0.0 && dpi_scale.is_finite() {
        dpi_scale
    } else {
        1.0
    };
    let width = (logical_width * scale).round().max(1.0) as i32;
    let height = (logical_height * scale).round().max(1.0) as i32;
    window_position::compute_position(cursor, &[work_area], width, height).unwrap_or(WindowRect {
        x: 0,
        y: 0,
        width,
        height,
    })
}

/// One-shot placement for the cursor monitor (M04.4 / F003): gather the cursor
/// position, the target monitor's work area and its DPI scale, and return the
/// full physical rect for a logical window size. Falls back to the primary work
/// area (0,0 point, scale 1.0) when the cursor is unobtainable or outside every
/// monitor.
pub fn placement_rect_for_cursor(logical_width: f32, logical_height: f32) -> WindowRect {
    let (cursor, monitors) = placement_inputs();
    let cursor = cursor.unwrap_or((0, 0));
    let work_area = monitors.first().copied().unwrap_or(MonitorRect {
        left: 0,
        top: 0,
        right: 1,
        bottom: 1,
    });
    let dpi_scale = dpi_scale_at(cursor.0, cursor.1);
    physical_rect_for_monitor(cursor, work_area, dpi_scale, logical_width, logical_height)
}

/// One-shot placement for the ACTIVE (foreground) window's monitor (M06.2
/// `MonitorStrategy::ActiveWindow`): locate the active window, use the center
/// of its screen rect as the anchor, and place on that monitor's work area.
/// Falls back to the cursor-monitor placement when there is no active window.
pub fn placement_rect_for_active_window(
    hwnd_bits: isize,
    logical_width: f32,
    logical_height: f32,
) -> WindowRect {
    let hwnd = HWND(hwnd_bits as *mut core::ffi::c_void);
    // Safety: `hwnd` is the active window queried via GetActiveWindow; read-only.
    let mut rect = RECT::default();
    let ok =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).is_ok() };
    if !ok || rect.right <= rect.left || rect.bottom <= rect.top {
        // No usable rect -> cursor fallback.
        return placement_rect_for_cursor(logical_width, logical_height);
    }
    let anchor_x = (rect.left + rect.right) / 2;
    let anchor_y = (rect.top + rect.bottom) / 2;
    let work_area = monitor_work_area_at(anchor_x, anchor_y).unwrap_or(MonitorRect {
        left: 0,
        top: 0,
        right: 1,
        bottom: 1,
    });
    let dpi_scale = dpi_scale_at(anchor_x, anchor_y);
    physical_rect_for_monitor(
        (anchor_x, anchor_y),
        work_area,
        dpi_scale,
        logical_width,
        logical_height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // M06.2: the active-window placement falls back to the cursor-monitor
    // path when the handle is unusable (headless/CI, window already gone) —
    // never panics, always returns a sane rect.
    #[test]
    fn active_window_placement_falls_back_for_invalid_hwnd() {
        let rect = placement_rect_for_active_window(-1, 600.0, 140.0);
        assert!(rect.width > 0 && rect.height > 0);
        // An invalid handle matches the fallback path (sane, finite rect).
        assert!(rect.x + rect.width >= rect.x);
    }

    #[test]
    fn dpi_scale_is_sane_without_a_monitor() {
        // Headless CI: whatever the environment reports must be positive/finite
        // (we clamp internally), never NaN/0.
        let scale = dpi_scale_at(-1_000_000, -1_000_000);
        assert!(scale > 0.0 && scale.is_finite());
    }

    // F003: placement must be computed with the TARGET monitor's DPI scale, not
    // the window's current global scale factor. A synthetic 200% secondary
    // monitor proves the physical size (and thus centering/clamping) follows
    // the target scale.
    #[test]
    fn physical_rect_for_monitor_uses_the_target_monitor_dpi() {
        let secondary = MonitorRect {
            left: 3840,
            top: 0,
            right: 7680,
            bottom: 2160,
        };
        // Cursor on the secondary monitor; its DPI is 200% (scale 2.0).
        let cursor = (5000, 800);
        let rect = physical_rect_for_monitor(cursor, secondary, 2.0, 600.0, 140.0);

        // 600x140 logical → 1200x280 physical at 200%.
        assert_eq!(rect.width, 1200);
        assert_eq!(rect.height, 280);
        // Centered horizontally on the secondary work area (3840..7680).
        assert_eq!(rect.x, 3840 + (3840 - 1200) / 2);
        // Top ~10% of the work-area height.
        assert_eq!(rect.y, 2160 / 10);
        // Fully inside the target monitor and never spanning.
        assert!(rect.x >= secondary.left && rect.x + rect.width <= secondary.right);
        assert!(rect.y >= secondary.top && rect.y + rect.height <= secondary.bottom);
    }

    // M07.4: the adapter's per-monitor DPI math (logical × target scale →
    // physical px, then clamp) verified for 125% / 150% / 200% on a synthetic
    // work area. Real mixed-DPI rendering stays a desktop-manual item.
    #[test]
    fn physical_rect_for_monitor_scales_at_125_150_200() {
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1440,
        };
        for (scale, expected_w, expected_h) in [(1.25, 750, 175), (1.5, 900, 210), (2.0, 1200, 280)]
        {
            let rect = physical_rect_for_monitor((1000, 700), work, scale, 600.0, 140.0);
            assert_eq!(rect.width, expected_w, "scale {scale}");
            assert_eq!(rect.height, expected_h, "scale {scale}");
            assert_eq!(rect.x, (2560 - expected_w) / 2, "scale {scale}");
            assert_eq!(rect.y, 1440 / 10, "scale {scale}");
            assert!(
                rect.x + rect.width <= work.right && rect.y + rect.height <= work.bottom,
                "scale {scale} stays inside work area"
            );
        }
    }

    #[test]
    fn physical_rect_for_monitor_clamps_with_the_target_scale() {
        // A window larger than the work area under a 150% scale must clamp with
        // the physical (scaled) size, staying fully inside the target monitor.
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let rect = physical_rect_for_monitor((960, 500), work, 1.5, 1920.0, 1040.0);
        // 1920x1040 logical * 1.5 → 2880x1560 physical → clamped to the work area.
        assert_eq!(rect.width, 1920);
        assert_eq!(rect.height, 1040);
        assert!(rect.x >= work.left && rect.x + rect.width <= work.right);
        assert!(rect.y >= work.top && rect.y + rect.height <= work.bottom);
    }

    #[test]
    fn physical_rect_for_monitor_defends_bad_scale_and_large_sizes() {
        // NaN / zero / negative scale falls back to 1.0 (never a -NaN size).
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        for bad_scale in [f32::NAN, 0.0, -2.0] {
            let rect = physical_rect_for_monitor((960, 500), work, bad_scale, 600.0, 140.0);
            assert_eq!(rect.width, 600);
            assert_eq!(rect.height, 140);
        }
        // A zero logical size is clamped to at least 1 physical px.
        let rect = physical_rect_for_monitor((960, 500), work, 2.0, 0.0, 0.0);
        assert_eq!(rect.width, 1);
        assert_eq!(rect.height, 1);
    }
}
