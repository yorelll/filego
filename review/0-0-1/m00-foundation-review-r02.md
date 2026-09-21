# Review: M00 Foundation (Round 02)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m00-foundation`
- **Round:** `02`
- **Date:** 2026-09-21
- **Reviewer agent:** Independent M00 code-review agent
- **Role declaration:** Code-review only; no repository file was modified.
- **Independence statement:** The reviewer did not implement the reviewed M00 code, workflows, fixes, or response.
- **Original review:** `review/0-0-1/m00-foundation-review-r01.md`
- **Implementation response:** `review/0-0-1/m00-foundation-response-r01.md`
- **Original reviewed SHA:** `a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Re-review head SHA:** `b63c5dcf4023e57c36e1d64e47af4f7a371a000c`
- **Comparison range:** `a71e2e1..b63c5dc`
- **Scope re-reviewed:**
  - `src/app.rs`
  - `README.md`
  - `docs/adr/001-slint-backend-renderer.md`
  - `docs/adr/002-event-loop-and-tray.md`
  - `docs/adr/003-system-tray.md`
  - `docs/adr/004-window-lifecycle.md`
  - `docs/adr/README.md`
  - `review/0-0-1/m00-foundation-response-r01.md`
  - related CI, release-workflow, candidate-artifact, dependency, and license evidence
- **Requirements:** M00 checklist; `REQ-REL-001..006`; M00 lifecycle and platform-spike prerequisites.
- **Current ordinary CI:** [Windows CI run `35567214670`](https://github.com/yorelll/quickfolder/actions/runs/35567214670)
- **Current ordinary CI SHA:** `b63c5dcf4023e57c36e1d64e47af4f7a371a000c`
- **Current ordinary CI conclusion:** `success`

## CI and Artifact Evidence Reviewed

| Evidence | Commit / candidate | Workflow / run | Conclusion |
|---|---|---|---|
| Lifecycle regression revalidation | `08bfb527ac20ea0e3d20d2b291f1543383b6b522` | [Windows CI 35562858225](https://github.com/yorelll/quickfolder/actions/runs/35562858225) | `success` |
| Response-document revalidation | `b63c5dcf4023e57c36e1d64e47af4f7a371a000c` | [Windows CI 35567214670](https://github.com/yorelll/quickfolder/actions/runs/35567214670) | `success` |
| Release-candidate workflow validation | `9081bf418e6a46b381ce9577d8940cc33fef7c4a` | [Build release candidate 35565687100](https://github.com/yorelll/quickfolder/actions/runs/35565687100) | `success` |

The current ordinary CI run passed all primary quality and packaging steps:

1. lockfile bootstrap gate;
2. immutable dependency validation;
3. Rust formatting;
4. Clippy with warnings denied;
5. all tests;
6. Windows x86-64 Release build;
7. version helper smoke;
8. dependency/source/license/advisory audit;
9. third-party license generation and sanity validation;
10. portable ZIP/SHA-256 package build and artifact upload.

The release-candidate workflow independently passed dispatch-input validation, exact candidate checkout, locked quality gates, dependency and license audits, unsigned EXE/ZIP/SHA-256 packaging, and candidate artifact upload.

The release-candidate artifact was independently downloaded and checked:

- Artifact: `QuickFolder-0.0.1-unsigned-candidate-9081bf418e6a46b381ce9577d8940cc33fef7c4a`
- Standalone EXE SHA-256: `680429a0735a08d88eb7a97a5bd99720fee4ccd668ede1bd3708026c63f157e6`
- ZIP SHA-256: `e7be8eb5843b2919cf600c0841a30934ecd8cd017800b8a9745a01bcb71605ee`
- Both values matched CI-generated `QuickFolder-0.0.1-SHA256SUMS.txt`.
- Standalone EXE and ZIP-contained EXE have `MZ` PE headers.
- ZIP contains `QuickFolder.exe`, `README.md`, `LICENSE`, `THIRD_PARTY_LICENSES.html`, and `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`.

## Re-review Method

- Re-read the r01 review, committed r01 implementation response, project constraints, M00 task plan, acceptance matrix, ADRs, source, Slint UI, dependency configuration, CI and release workflows.
- Inspected the source and document diff from `a71e2e1` through the exact re-review head `b63c5dc`.
- Independently queried the lifecycle revalidation, response-head ordinary CI, and release-candidate CI runs.
- Independently checked the candidate checksum manifest, package contents, PE headers, and third-party license report.
- Did not run Rust locally, consistent with `CLAUDE.md`.
- Did not perform interactive Windows desktop validation.

## Prior Finding Disposition

| Finding | Disposition | Evidence |
|---|---|---|
| `M00-R01-F001` | **CLOSED** | `src/app.rs:47-57` preserves `LifecycleState::Exiting` when the direct native-close path executes. `src/app.rs:263-283` adds `native_close_after_exit_preserves_terminal_state`, proving Exit → native close → Show remains terminal and records only `quit`. Exact lifecycle revalidation CI run `35562858225` passed. |
| `M00-R01-F002` | **CLOSED** | Release-candidate workflow `35565687100` successfully executed against exact SHA `9081bf418e6a46b381ce9577d8940cc33fef7c4a`; exact checkout, locked quality gates, dependency audits, package creation, checksum generation, and artifact upload were all exercised. Candidate artifacts were independently checked. |
| `M00-R01-F003` | **OPEN** | README and ADRs no longer claim that ordinary Windows CI was never run, but still state that the release-candidate workflow is unverified. The exact release-candidate workflow succeeded as run `35565687100` for head `9081bf…`. The status documentation remains factually stale. |

## Requirement Assessment

| Requirement / M00 criterion | Status | Evidence / notes |
|---|---|---|
| Rust verification only through GitHub Actions | PASS | All claimed Rust evidence is from exact GitHub Actions runs. |
| Checked-in immutable dependencies | PASS | `Cargo.lock` is committed; ordinary and release-candidate workflows both passed frozen/locked dependency checks. |
| Formatting, Clippy, tests, and Release build | PASS | Passed in ordinary Windows CI run `35567214670` and release-candidate run `35565687100`. |
| Release EXE existence and version smoke | PASS | Passed in both CI workflows; independently verified PE headers. |
| Dependency/license/source audit | PASS | Both workflows passed target-scoped Windows x86-64 `cargo-deny` and `cargo-about` gates. |
| Portable ZIP and SHA-256 | PASS | Candidate workflow generated standalone EXE, ZIP, and checksum manifest; independent checksum and package-content checks passed. |
| Terminal shutdown lifecycle | PASS | r01 finding fixed with direct regression coverage and exact CI revalidation. |
| Release workflow actual validation | PASS | Authorized exact-SHA workflow dispatch passed without creating a tag, GitHub Release, or signed binary. |
| Verification-status documentation | PARTIAL | Ordinary-CI status is corrected; release-workflow status remains falsely listed as unverified. |
| Real Windows desktop behavior | UNVERIFIED | Still requires actual desktop validation; this does not prevent an M00 milestone review once documentation is corrected. |

## Findings

### `M00-R02-F001` — Low — Verification-status documentation still says a verified release workflow is unverified

- **Location:**
  - `README.md:7`
  - `docs/adr/001-slint-backend-renderer.md:51`
  - `docs/adr/002-event-loop-and-tray.md:46`
  - `docs/adr/003-system-tray.md:47`
  - `docs/adr/004-window-lifecycle.md:50`
- **Category:** Documentation / audit accuracy
- **Problem:** The response corrected the prior claim that no ordinary CI had run, but the tracked status text still says the release-candidate workflow is unverified.
- **Trigger / evidence:** Release-candidate workflow run `35565687100` succeeded for exact SHA `9081bf418e6a46b381ce9577d8940cc33fef7c4a`; all primary release-candidate job steps succeeded.
- **Impact:** The tracked architecture and public-status documentation remains factually inaccurate. It obscures evidence that exact-SHA checkout, candidate packaging, checksum creation, and unsigned artifact upload have been validated.
- **Required change:** Update each status statement to distinguish ordinary Windows CI verified; release-candidate workflow verified by run `35565687100` at head `9081bf…`; real Windows desktop behavior remains unverified.
- **Verification required:** Commit corrected documentation and audit records, then obtain ordinary Windows CI success for that documentation head. The release-candidate workflow need not be re-dispatched solely for this documentation-only correction unless the workflow itself changes.
- **Release blocking:** No by itself, but must be corrected before milestone evidence is considered accurate.

## Cross-cutting Re-review Checks

### Correctness and Error Handling

- `accept_window_close()` now preserves `Exiting`.
- The regression test covers `ExitFromTray → native close callback state transition → Show`.
- `handle()` retains terminal-state suppression for queued commands.
- Initial quit failure remains retryable.

### Data Safety and Privacy

- No folder deletion, recursive directory operation, path opening, shell invocation, or unsafe filesystem operation was introduced.
- No telemetry, application network client, path scan, file-content read, or user-path/query logging was added.
- Generic runtime diagnostic strings remain path-safe.

### Security, Dependencies, and Licensing

- No advisory ignore was introduced.
- Windows x86-64 target filtering remains explicit in `deny.toml`.
- Both ordinary and release-candidate workflows passed dependency, source, advisory, and license gates.
- Candidate artifact includes the vendored Slint license and generated third-party license inventory.
- No project-owned unsafe code exists.

### Windows, UI, and Accessibility

- CI verifies compilation/linking of Slint tray declarations, icons, Winit backend, and Windows resources.
- CI cannot verify actual tray behavior, native callback scheduling, taskbar behavior, foreground focus, Explorer restart recovery, DPI, taskbar-theme contrast, or IME interaction.
- These remain explicitly documented manual desktop checks.

### CI, Workflow, and Artifact Controls

- The release workflow demonstrated exact SHA checkout and verification, read-only token permissions, locked gates, unsigned packaging, SHA-256 generation, and artifact upload.
- It deliberately has no publishing, tag creation, GitHub Release, or signing path.
- Final release gating remains separate: user manual acceptance and `APPROVED_FOR_RELEASE` are still required.

## Residual Manual Validation Items

Before any application release, the project owner must test and record:

- silent tray-first startup and no unintended taskbar entry;
- tray click/menu behavior and clean termination;
- Explorer restart tray recovery;
- focus behavior after opening;
- Chinese IME composition and candidate selection;
- light/dark taskbar icon legibility;
- 100%, 125%, 150%, and 200% DPI behavior;
- multi-monitor and taskbar-edge behavior;
- idle CPU, disk activity, memory, startup latency, and responsiveness.

## Verdict

`CHANGES_REQUESTED`

### Rationale

The two blocking r01 findings are independently verified as closed: terminal shutdown state is preserved and regression-tested; the exact-SHA release-candidate workflow executed successfully and its unsigned candidate artifact was independently checked.

The remaining issue is low severity but affects audit accuracy: tracked documentation still incorrectly describes the release-candidate workflow as unverified despite successful exact-head run `35565687100`. Correct that status, commit the response/re-review audit records, and obtain ordinary CI success for the documentation head. No release approval is granted by this review.
