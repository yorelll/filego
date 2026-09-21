# ADR-005: Centralized Windows API boundary

- Status: Accepted
- Date: 2026-09-20
- Owners: FileGo maintainers
- Requirements: REQ-REL-002, REQ-LIFE-014, REQ-FOLDER-024

## Context

Later milestones require Win32 global hotkeys, a current-user single-instance channel, startup registration, Shell folder opening, monitor work-area positioning, native activation, and possibly taskbar style changes. Scattered native calls make resource lifetime, error checking, testing, and security review unreliable.

M00 can avoid project-owned native calls because Slint provides window and tray services. This is an opportunity to fix the boundary before platform features arrive.

## Decision

- All project-owned Windows APIs live under `src/platform/windows/` and are compiled with `cfg(target_os = "windows")`.
- Application/domain/storage modules cannot depend on Win32 types or handles.
- Platform effects are exposed through narrow traits and typed commands; tests use fakes.
- Prefer one direct `windows` crate dependency with the minimum feature set when M04 first needs APIs. Do not mix `winapi`, `windows-sys`, and `windows` in project code without a new ADR.
- Unsafe blocks must be as small as possible and document handle ownership, thread affinity, pointer validity, and callback lifetime.
- Every fallible Win32 operation must inspect its documented return convention and convert errors to typed internal categories. User UI receives localizable categories, not raw codes or paths.
- Never open user paths by constructing `cmd.exe` or PowerShell command strings. Use Shell APIs directly.
- No platform adapter may expose recursive file/directory deletion; FileGo only removes records.

## Alternatives considered

- Let each feature choose its own convenience crate: rejected because it creates overlapping bindings and unclear ownership.
- Put native calls directly in Slint callbacks: rejected because it blocks testing and leaks OS concerns into presentation code.
- Add the `windows` crate before it is needed: rejected in M00 to minimize unverified dependencies; add exact features with M04 code.

## Consequences

### Positive

- Native code has one auditable surface.
- Core behavior can be tested on hosted CI without interactive Win32 calls.
- Shell injection and real-directory deletion risks become structurally easier to review.

### Negative and risks

- Some convenient crates may be rejected or wrapped, requiring more adapter code.
- Slint itself has transitive Windows bindings; “one binding” applies to project-owned direct API code, not framework internals.

## Verification

- Code review enforces module boundaries and unsafe invariants.
- CI uses clippy with warnings denied and Windows release linking.
- M04 adds adapter failure tests and desktop tests for every native feature.
- Current status: boundary exists; no project-owned unsafe/Win32 API has been added.
