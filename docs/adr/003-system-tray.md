# ADR-003: Built-in Slint SystemTrayIcon

- Status: Accepted; Windows CI verified, desktop verification pending
- Date: 2026-09-20
- Owners: FileGo maintainers
- Requirements: REQ-LIFE-001, REQ-LIFE-003, REQ-LIFE-006, REQ-LIFE-009

## Context

M00 needs a minimal Windows tray vertical slice: a visible icon, left-click toggle, Open menu command, and Exit. The final application must also survive Explorer restarts. A custom tray implementation would require unsafe icon/menu/window ownership and duplicate APIs Slint already implements.

Inspection of Slint 1.18.0's Windows source shows direct `Shell_NotifyIconW` use, a hidden message-only window, return-value/error handling for creation, and a registered `TaskbarCreated` message that re-adds the icon after Explorer restarts.

## Decision

- Implement `AppTray` as Slint's built-in `SystemTrayIcon`.
- Use the project-owner-provided tray artwork processed into a transparent 32×32 RGBA PNG. Preserve the unchanged source and reproducible processing details under `assets/source/`, `tools/process_icons.py`, and `docs/assets.md`. The owner confirmed public-project and release distribution authorization on 2026-09-21.
- Expose three callbacks: `toggle-window`, `open-window`, and `quit-requested`.
- Map left click to toggle. The context menu contains “打开 FileGo”, a separator, and “退出”.
- Rely on Slint 1.18's same-thread Windows backend and `TaskbarCreated` recovery instead of adding another tray dependency.
- Keep the tray component strongly alive for the complete event-loop lifetime.

## Alternatives considered

- `tray-icon`: mature and portable, but rejected because it introduces another event integration and overlapping Windows bindings.
- Direct FileGo `Shell_NotifyIconW`: rejected because it repeats substantial unsafe resource management and recovery code.
- No M00 tray: rejected because it would leave the highest architecture risk unresolved.

## Consequences

### Positive

- No project-owned unsafe code or second Win32 binding dependency in M00.
- Slint handles icon conversion, hidden message window, menu dispatch, cleanup, and Explorer recovery.
- The tray itself keeps the event loop alive while the main window is hidden.

### Negative and risks

- Tray behavior is coupled to Slint 1.18 and must be re-evaluated on dependency upgrades.
- The API is new; Windows 10/11, light/dark taskbar readability, single-vs-double-click, and Explorer restart need desktop tests.
- M00 has only the minimal menu. Full menus and state indicators belong to M04/M06.

## Verification

- CI: Slint compiler and Windows linker must accept `SystemTrayIcon`, the generated RGBA PNG icon, menu, callbacks, and selected features.
- Desktop: tests C1–C3, C8–C9 from the manual acceptance checklist.
- Current status: FileGo rename, GNU/MSVC policy, and explicit MSVC target paths passed Windows CI run `35587340136` (head `c42f5728`). The renamed release-candidate workflow package paths still require a FileGo candidate run. Real Windows desktop behavior remains unverified.

## References

- https://docs.slint.dev/1.18.0/docs/slint/reference/std-widgets/views/systemtrayicon/
- https://github.com/slint-ui/slint/blob/v1.18.0/examples/system-tray/system-tray.slint
- https://github.com/slint-ui/slint/blob/v1.18.0/internal/core/items/system_tray/windows.rs
