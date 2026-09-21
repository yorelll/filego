# Architecture Decision Records

ADRs capture decisions that constrain QuickFolder implementation. They are immutable records: if a decision changes, add a superseding ADR rather than rewriting history after release.

| ADR | Decision | Status |
|---|---|---|
| [001](001-slint-backend-renderer.md) | Slint backend and renderer | Accepted; CI and release-candidate workflow verified, desktop verification pending |
| [002](002-event-loop-and-tray.md) | One Slint-owned event loop | Accepted; CI and release-candidate workflow verified, desktop verification pending |
| [003](003-system-tray.md) | Built-in Slint SystemTrayIcon | Accepted; CI and release-candidate workflow verified, desktop verification pending |
| [004](004-window-lifecycle.md) | Hidden startup and close-to-tray lifecycle | Accepted; CI and release-candidate workflow verified, desktop verification pending |
| [005](005-windows-api-boundary.md) | Centralized Windows API boundary | Accepted |

New records should follow [`template.md`](template.md).
