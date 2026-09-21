# ADR-004: Hidden startup and close-to-tray lifecycle

- Status: Accepted; Windows CI verified, desktop verification pending
- Date: 2026-09-20
- Owners: QuickFolder maintainers
- Requirements: REQ-LIFE-001, REQ-LIFE-002, REQ-LIFE-003, REQ-WINDOW-006

## Context

QuickFolder is tray-first. It must start silently, reuse one search window, hide on native close, and terminate only through the tray Exit command. Window visibility state must be testable without a live desktop.

Slint exposes `Window::on_close_requested`, `CloseRequestResponse::HideWindow`, `show`, `hide`, and `is_visible`. It does not expose a stable declarative skip-taskbar property or a portable native activation API.

## Decision

- Construct but do not show `AppWindow` at startup.
- Register `on_close_requested` and return `HideWindow` after updating the lifecycle state; do not call quit.
- Retain the component and reuse it across every show/hide cycle.
- Model `Hidden`, `Visible`, and `Exiting` in a pure `LifecycleController`; perform effects through a `WindowPort` trait.
- An explicit Open command always shows; a tray left-click toggles; only Exit invokes the event-loop quit path.
- Use a frameless, always-on-top M00 shell. Do not use private Winit APIs to skip taskbar or force focus in M00.
- M04 must add a narrowly scoped native Windows adapter for taskbar visibility/activation/monitor placement if desktop tests show the stable Slint surface cannot satisfy them.

## Alternatives considered

- Calling `app.run()`: rejected because it shows the window immediately and couples event-loop entry to that window.
- Destroy/recreate on every close: rejected because it loses state and adds latency.
- Private `i-slint-backend-winit` access now: deferred because it adds unstable backend coupling and timing hazards before the basic build is verified.
- `std::process::exit` from tray: rejected for normal shutdown because it bypasses cleanup.

## Consequences

### Positive

- Lifecycle semantics are independently unit-testable.
- Native close and explicit exit are unambiguous.
- The architecture is ready to insert future save/cleanup before Exit.

### Negative and risks

- “No taskbar entry” and reliable foreground focus are not fully proven by M00. Hidden windows normally have no taskbar entry, but showing a frameless Window may still create one.
- `always-on-top` is a temporary shell choice and needs UX review in M03/M04.
- Focus and current-monitor positioning remain M04 work.

## Verification

- Automated: state transition tests with a fake port, including error rollback and close-to-hidden.
- CI: build generated callbacks and close handler.
- Desktop: hidden startup, no taskbar entry, focus after Open, repeated close/reopen, and graceful Exit.
- Current status: Windows CI run `35562858225` (head `08bfb527`) passed format, Clippy, tests, and Release build with the lifecycle tests. Real Windows desktop behavior and the release-candidate workflow remain unverified.

## References

- https://docs.rs/slint/1.18.0/slint/struct.Window.html
- https://docs.rs/slint/1.18.0/slint/enum.CloseRequestResponse.html
- https://github.com/slint-ui/slint/issues/7161
