# ADR-002: One Slint-owned event loop

- Status: Accepted; CI and desktop verification pending
- Date: 2026-09-20
- Owners: QuickFolder maintainers
- Requirements: REQ-LIFE-001, REQ-LIFE-002, REQ-LIFE-003, REQ-LIFE-014

## Context

Windows tray callbacks need a native message pump. Slint's Winit backend also owns a native event loop. Running an external tray crate with a second main-thread loop creates unclear ownership, deadlock, callback delivery, and shutdown risks.

Slint 1.18 provides `SystemTrayIcon`. Its Windows backend creates a message-only window, and that window receives events through the same Winit/Slint event-loop thread. A visible tray icon keeps `slint::run_event_loop()` alive even when the main window is hidden.

## Decision

- Create the `AppWindow` and `AppTray` components on the main thread.
- Run only `slint::run_event_loop()`; do not create a Tao/Winit loop or a polling thread.
- Route tray callbacks into a platform-independent `LifecycleController` on the same thread.
- Keep only Slint weak handles in adapters/callbacks to avoid ownership cycles.
- Use `slint::quit_event_loop()` exclusively for the explicit tray Exit action.
- Future worker threads must return UI messages with Slint event-loop dispatch APIs; they must never manipulate Slint components or native handles directly.

## Alternatives considered

- External `tray-icon`: rejected for M00 because its Windows setup also needs a same-thread event loop, duplicating machinery now provided by Slint.
- Bespoke Win32 message loop plus Slint loop: rejected because two blocking loops cannot both own the main thread safely.
- Timer polling of channels: rejected because it causes avoidable idle wakeups and violates the near-zero idle CPU goal.

## Consequences

### Positive

- Tray, window, and menu callbacks have one ordering/thread model.
- Hidden startup is supported without a dummy visible window.
- Shutdown does not require coordinating multiple event-loop owners.

### Negative and risks

- This relies on Slint's relatively new system-tray implementation and must be proven on both Windows versions.
- Long-running work on callbacks would block all UI/tray processing; later I/O must use workers.

## Verification

- Automated: lifecycle state/port tests; Windows build; version smoke.
- Desktop: hidden startup, repeated show/hide, menu actions, clean Exit, and idle CPU.
- Current status: source implementation complete; no GitHub Actions run or desktop test yet.

## References

- https://docs.rs/slint/1.18.0/slint/fn.run_event_loop.html
- https://github.com/slint-ui/slint/tree/v1.18.0/examples/system-tray
- https://github.com/slint-ui/slint/blob/v1.18.0/internal/core/items/system_tray/windows.rs
