# ADR-001: Slint backend and renderer

- Status: Accepted; Windows CI verified, desktop verification pending
- Date: 2026-09-20
- Owners: FileGo maintainers
- Requirements: REQ-REL-001, REQ-REL-002, REQ-UI-006, REQ-WINDOW-014

## Context

FileGo targets Windows 10 22H2 and Windows 11, needs Unicode/IME, high-DPI, accessibility, a native tray icon, and low idle resource use. It must not embed a browser engine. The local MinGW64 GNU toolchain provides fast development feedback; Windows MSVC CI remains the compatibility and release-validation authority.

Slint 1.18.0 requires Rust 1.92 and contains the `SystemTrayIcon` implementation introduced in 1.17. The application must choose compile-time features deliberately rather than accepting every default renderer and backend.

## Decision

- Pin `slint` and `slint-build` to exactly `1.18.0` and pin Rust to `1.92.0`.
- Disable Slint default features.
- Enable `std`, `compat-1-2`, `backend-winit`, `renderer-software`, `software-renderer-systemfonts`, `accessibility`, and `system-tray`.
- Use the Winit backend with Slint's software renderer and its Windows system-font/Parley path for M00. This favors compatibility, installed CJK font fallback, and a smaller, more deterministic renderer dependency set over GPU acceleration.
- Use Slint's normal desktop resource embedding (`EmbedFiles`, selected by the default `slint-build` configuration). PNG assets remain self-contained in the portable EXE while Windows system fonts are discovered at runtime. Do not use the MCU-oriented `EmbedForSoftwareRenderer` mode unless a later measurement justifies pre-rendered resources.
- Keep renderer selection isolated to Cargo features so a later measured ADR may add Skia if real hardware tests show software rendering cannot meet the UI latency/resource targets.
- Use Slint under `LicenseRef-Slint-Royalty-free-2.0` and retain the accessible `AboutSlint` attribution in the production About page.

## Alternatives considered

- Slint default features: rejected because they enable a broader backend/renderer surface than this Windows application requires.
- Skia plus software fallback: deferred because Skia increases binary size and dependency surface before measurements show it is needed.
- FemtoVG: deferred for the same reason and because FileGo's UI has modest rendering needs.
- Browser UI frameworks: rejected by the product's native/lightweight constraint.

## Consequences

### Positive

- One native UI framework supplies Windows windowing, accessibility integration, renderer, and tray support.
- Software rendering avoids depending on a particular GPU driver path for the first architecture spike.
- Exact versions and a checked-in lockfile after bootstrap make CI repeatable.

### Negative and risks

- Software rendering may consume more CPU for animation or large surfaces; M07 must measure this.
- The system-font feature increases dependencies and uses installed Windows fonts; Chinese glyph fallback, shaping, and IME rendering still require real Windows tests.
- Icon assets are preprocessed at explicit sizes; their small-size clarity and Windows DPI scaling must still be checked on real systems.
- Slint's desktop royalty-free license requires attribution and preserved notices.
- Rust 1.92 is newer than earlier Slint releases' MSRV but matches Slint 1.18's actual workspace requirement.

## Verification

- Automated: local GNU checks provide fast feedback; Windows MSVC CI must compile the UI and embedded desktop resources, run clippy/tests, and build Release with only selected features.
- Desktop: inspect text/IME, 100–200% DPI, high contrast, and idle CPU on Windows 10/11.
- Current status: FileGo rename, GNU/MSVC policy, and explicit MSVC target paths passed Windows CI run `35587340136` (head `c42f5728`); FileGo release-candidate workflow run `35590444471` at candidate `3c42413c` also passed. Real Windows desktop behavior remains unverified. Local GNU validation is permitted only under the repository policy and cannot replace MSVC evidence.

## References

- https://github.com/slint-ui/slint/releases/tag/v1.18.0
- https://docs.rs/slint/1.18.0/slint/docs/cargo_features/index.html
- https://docs.slint.dev/1.18.0/docs/slint/guide/backends-and-renderers/backend_winit/
- https://github.com/slint-ui/slint/blob/v1.18.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md
