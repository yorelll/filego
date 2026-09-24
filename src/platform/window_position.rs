//! Pure monitor-aware window positioning (M04.4).
//!
//! All geometry math is deterministic over synthetic monitor rectangles, so the
//! adapter's `GetMonitorInfoW` / `MonitorFromPoint` / DPI lookup is the only
//! FFI. The adapter asks "which monitor should the search window appear on
//! right now" and "what is its work area / scale factor", then calls
//! [`compute_position`] and hands the result to Slint's
//! `Window::set_position` / `set_size`.
//!
//! Placement rules (matches the M04.4 acceptance list):
//! - default monitor = the one containing the current cursor point; fall back
//!   to the primary (index 0) work area when the point is on no monitor;
//! - horizontal: centered on the work area;
//! - vertical: top ≈ 10% of the work area height (not covering the taskbar);
//! - clamped so the whole window stays inside that monitor's work area (never
//!   spans two monitors, never slides under a taskbar);
//! - sizes are treated as the physical, DPI-scaled window size the caller just
//!   computed from Slint's logical size × scale factor.

/// A monitor work area in physical pixels (from `MONITORINFO.rcWork`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl MonitorRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// A window rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

fn clamp_window(
    work: MonitorRect,
    width: i32,
    height: i32,
    prefer_x: i32,
    prefer_y: i32,
) -> WindowRect {
    // Clamp size first so the window never exceeds the work area.
    let width = width.min(work.width()).max(1);
    let height = height.min(work.height()).max(1);
    let x = prefer_x.clamp(work.left, work.right - width);
    let y = prefer_y.clamp(work.top, work.bottom - height);
    WindowRect {
        x,
        y,
        width,
        height,
    }
}

/// Compute the search-window rectangle for `cursor` over `monitors`.
///
/// `cursors: (x, y)` are physical pixels (the adapter passes `GetCursorPos`).
/// `window_width/height` are the physical DPI-scaled window size. `monitors[0]`
/// must be the primary monitor's work area (the adapter guarantees ordering).
pub fn compute_position(
    cursor: (i32, i32),
    monitors: &[MonitorRect],
    window_width: i32,
    window_height: i32,
) -> Option<WindowRect> {
    let primary = monitors.first()?;
    let work = monitors
        .iter()
        .find(|monitor| {
            (monitor.left..monitor.right).contains(&cursor.0)
                && (monitor.top..monitor.bottom).contains(&cursor.1)
        })
        .copied()
        .unwrap_or(*primary);

    let prefer_x = work.left + (work.width() - window_width) / 2;
    let prefer_y = work.top + work.height() / 10; // top ≈ 10%
    Some(clamp_window(
        work,
        window_width,
        window_height,
        prefer_x,
        prefer_y,
    ))
}

/// Identity of the monitor to use, decoded by the adapter.
///
/// RESERVED for a future "always place on a fixed monitor" setting. The current
/// M04.4 placement uses the cursor monitor exclusively (`compute_position` is
/// cursor-driven), so `TargetMonitor` is intentionally not wired into any call
/// path yet; the variants are kept so the geometry code documents the two
/// policies and a settings-driven adapter can select between them without
/// inventing a new shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMonitor {
    /// The monitor containing the current cursor.
    Cursor,
    /// The primary monitor.
    Primary,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: MonitorRect = MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };
    // A secondary monitor to the right of the primary.
    const SECONDARY: MonitorRect = MonitorRect {
        left: 1920,
        top: 0,
        right: 3840,
        bottom: 1040,
    };

    #[test]
    fn centers_on_the_cursor_monitor() {
        let rect = compute_position((960, 500), &[PRIMARY, SECONDARY], 600, 140).unwrap();
        // Centered horizontally on the primary.
        assert_eq!(rect.x, (1920 - 600) / 2);
        // top 10% of work height.
        assert_eq!(rect.y, 1040 / 10);
        assert_eq!(rect.width, 600);
        assert_eq!(rect.height, 140);
    }

    #[test]
    fn cursor_on_secondary_monitor_places_there() {
        let rect = compute_position((2500, 500), &[PRIMARY, SECONDARY], 600, 140).unwrap();
        assert_eq!(rect.x, 1920 + (1920 - 600) / 2);
        assert_eq!(rect.y, 1040 / 10);
    }

    #[test]
    fn cursor_between_monitors_falls_back_to_primary() {
        let rect = compute_position((-10, -10), &[PRIMARY, SECONDARY], 600, 140).unwrap();
        assert_eq!(rect.x, (1920 - 600) / 2);
    }

    #[test]
    fn window_larger_than_work_area_is_clamped_inside() {
        let rect = compute_position((960, 500), &[PRIMARY], 4000, 3000).unwrap();
        // Clamped to exactly the work area bounds.
        assert_eq!(rect.width, 1920);
        assert_eq!(rect.height, 1040);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        // Still contained.
        assert!(rect.x >= PRIMARY.left && rect.x + rect.width <= PRIMARY.right);
        assert!(rect.y >= PRIMARY.top && rect.y + rect.height <= PRIMARY.bottom);
    }

    #[test]
    fn never_spans_two_monitors() {
        // A window whose centered x would overrun the right edge of the primary
        // is clamped back so it stays fully inside the primary.
        let rect = compute_position((959, 500), &[PRIMARY, SECONDARY], 1920, 140).unwrap();
        assert!(
            rect.x + rect.width <= PRIMARY.right,
            "must fit inside the primary"
        );
        assert!(rect.x >= PRIMARY.left);
    }

    #[test]
    fn respects_a_taskbar_at_the_bottom() {
        // Work area already excludes the taskbar on Windows; the y clamp keeps
        // the window above it.
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 960,
        };
        let rect = compute_position((960, 500), &[work], 600, 500).unwrap();
        assert!(rect.y + rect.height <= 960);
        // Preferred y = 10% = 96, but height 500 fits, so stays at 96.
        assert_eq!(rect.y, 96);
    }

    #[test]
    fn zero_monitor_list_yields_none() {
        assert_eq!(compute_position((0, 0), &[], 600, 140), None);
    }

    #[test]
    fn primary_work_area_may_start_at_a_negative_origin() {
        // A monitor layout where the primary is not at (0,0).
        let shifted = MonitorRect {
            left: -1920,
            top: -100,
            right: 0,
            bottom: 900,
        };
        let rect = compute_position((-960, 400), &[shifted], 600, 140).unwrap();
        assert!(rect.x >= -1920 && rect.x + 600 <= 0);
        assert!(rect.y >= -100 && rect.y + 140 <= 900);
    }

    // M07.4: verify the pure geometry for the common Windows scaling points.
    // `compute_position` takes the PHYSICAL (DPI-scaled) window size, so each
    // scale is checked as "logical × scale" physical dimensions stay centered /
    // top-10% / fully inside the work area. These are pure-function tests of the
    // 125% / 150% / 200% math; the real mixed-DPI desktop rendering remains a
    // manual acceptance item.
    #[test]
    fn geometry_holds_at_125_percent_scaling() {
        // 2560x1440 work area at 125% (scale 1.25): a 600x140 logical window is
        // 750x175 physical.
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1440,
        };
        let rect = compute_position(
            (1000, 700),
            &[work],
            (600.0 * 1.25) as i32,
            (140.0 * 1.25) as i32,
        )
        .unwrap();
        assert_eq!(rect.width, 750);
        assert_eq!(rect.height, 175);
        assert_eq!(rect.x, (2560 - 750) / 2);
        assert_eq!(rect.y, 1440 / 10);
        assert!(rect.x + rect.width <= 2560);
        assert!(rect.y + rect.height <= 1440);
    }

    #[test]
    fn geometry_holds_at_150_percent_scaling() {
        // 1920x1080 work area at 150% (scale 1.5): 600x140 logical → 900x210
        // physical.
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let rect = compute_position(
            (960, 500),
            &[work],
            (600.0 * 1.5) as i32,
            (140.0 * 1.5) as i32,
        )
        .unwrap();
        assert_eq!(rect.width, 900);
        assert_eq!(rect.height, 210);
        assert_eq!(rect.x, (1920 - 900) / 2);
        assert_eq!(rect.y, 1040 / 10);
        assert!(rect.x + rect.width <= 1920);
        assert!(rect.y + rect.height <= 1040);
    }

    #[test]
    fn geometry_holds_at_100_and_200_percent_scaling() {
        // The two endpoints: 100% and 200% on a 4K work area. Each stays
        // centered / top-10% / fully inside the target monitor.
        let work = MonitorRect {
            left: 0,
            top: 0,
            right: 3840,
            bottom: 2160,
        };
        let r100 = compute_position(
            (500, 500),
            &[work],
            (600.0 * 1.0) as i32,
            (140.0 * 1.0) as i32,
        )
        .unwrap();
        assert_eq!((r100.width, r100.height), (600, 140));
        let r200 = compute_position(
            (500, 500),
            &[work],
            (600.0 * 2.0) as i32,
            (140.0 * 2.0) as i32,
        )
        .unwrap();
        assert_eq!((r200.width, r200.height), (1200, 280));
        for rect in [r100, r200] {
            assert!(rect.x >= 0 && rect.x + rect.width <= 3840);
            assert!(rect.y >= 0 && rect.y + rect.height <= 2160);
            assert_eq!(rect.x, (3840 - rect.width) / 2);
            assert_eq!(rect.y, 2160 / 10);
        }
    }
}
