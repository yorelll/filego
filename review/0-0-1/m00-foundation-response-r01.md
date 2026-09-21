# Response: M00 Foundation (Review Round 01)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m00-foundation`
- **Implementation agent:** M00 implementation agent
- **Review document:** [`m00-foundation-review-r01.md`](m00-foundation-review-r01.md)
- **Original reviewed SHA:** `a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Fix candidate SHA:** `9081bf418e6a46b381ce9577d8940cc33fef7c4a`
- **Response date:** 2026-09-21

## Summary

| Finding | Assessment | Status |
|---|---|---|
| `M00-R01-F001` | `ACCEPTED` | Fixed and revalidated by ordinary Windows CI |
| `M00-R01-F002` | `ACCEPTED` | Fixed by an authorized exact-SHA release-candidate workflow run |
| `M00-R01-F003` | `ACCEPTED` | Fixed and revalidated by ordinary Windows CI |

No findings were rejected or deferred.

## Finding Responses

### `M00-R01-F001` — `ACCEPTED`

- **Assessment:** Correct. The original native close callback called `accept_window_close()` directly, which could change `Exiting` to `Hidden` after tray Exit had scheduled the event-loop shutdown.
- **Technical rationale:** The controller’s `handle()` method treated `Exiting` as terminal, but the direct close callback path bypassed that guard. A queued close callback could therefore clear the terminal state and allow a later queued Show command to perform a window operation.
- **Change made:**
  - `src/app.rs` now keeps `LifecycleState::Exiting` unchanged in `accept_window_close()`.
  - Added `native_close_after_exit_preserves_terminal_state`, which executes `ExitFromTray`, invokes the native-close state path, then sends `Show`; it asserts the terminal state remains `Exiting` and the fake platform records only `quit`.
  - Existing `exit_failure_retains_retryable_state` still proves that a failed initial `quit()` does not enter the terminal state and can be retried.
- **Files/lines:** `src/app.rs` (`accept_window_close` and lifecycle-controller unit tests).
- **Tests added/changed:** `native_close_after_exit_preserves_terminal_state`.
- **Risk after response:** Hosted CI confirms the state transition and regression test. Real Windows callback ordering and tray behavior remain desktop manual-validation items.
- **CI evidence:**
  - Commit containing the code fix: `157a3fff85888ca8191f1bafa2e972b858c370b2`.
  - Revalidation commit: `08bfb527ac20ea0e3d20d2b291f1543383b6b522` (format-only correction to the new test).
  - Windows CI: [35562858225](https://github.com/yorelll/quickfolder/actions/runs/35562858225), conclusion `success`.
  - Verified jobs: locked dependencies, fmt, Clippy with warnings denied, tests, Release build, version smoke, target-scoped dependency audit, license inventory, portable package, artifact upload.

### `M00-R01-F002` — `ACCEPTED`

- **Assessment:** Correct. The ordinary CI workflow could not validate the release candidate workflow’s `workflow_dispatch` inputs, exact-SHA checkout, candidate naming, and standalone EXE/ZIP/SHA-256 packaging.
- **Technical rationale:** This workflow is security- and release-relevant. It required one real run against the candidate SHA rather than inference from the ordinary CI workflow.
- **Change made:** No source change was required. With explicit project-owner authorization, the `Build release candidate` workflow was dispatched with version `0.0.1` and the exact candidate SHA.
- **Files/lines:** `.github/workflows/release.yml` executed without modification after ordinary CI validation.
- **Tests added/changed:** Workflow execution exercised input validation, exact candidate checkout, immutable dependency validation, fmt, Clippy, tests, Release build, version helper, dependency policy audit, license generation, unsigned candidate packaging, SHA-256 generation, and artifact upload.
- **Risk after response:** The workflow has been verified for this candidate. It deliberately has no write permission, tag creation, GitHub Release publication, or signing step. Future workflow changes require a fresh run.
- **CI evidence:**
  - Candidate SHA: `9081bf418e6a46b381ce9577d8940cc33fef7c4a`.
  - Release-candidate workflow: [35565687100](https://github.com/yorelll/quickfolder/actions/runs/35565687100), conclusion `success`.
  - Job: `Validate and package exact candidate`.
  - Artifact: `QuickFolder-0.0.1-unsigned-candidate-9081bf418e6a46b381ce9577d8940cc33fef7c4a`.
  - Candidate artifacts and locally rechecked SHA-256 values:
    - `QuickFolder-0.0.1-windows-x86_64.exe`: `680429a0735a08d88eb7a97a5bd99720fee4ccd668ede1bd3708026c63f157e6`
    - `QuickFolder-0.0.1-windows-x86_64.zip`: `e7be8eb5843b2919cf600c0841a30934ecd8cd017800b8a9745a01bcb71605ee`
  - The downloaded checksums matched CI’s `QuickFolder-0.0.1-SHA256SUMS.txt`. The standalone EXE and the ZIP-contained EXE both have `MZ` PE headers. The ZIP contains `QuickFolder.exe`, `README.md`, `LICENSE`, `THIRD_PARTY_LICENSES.html`, and `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`.

### `M00-R01-F003` — `ACCEPTED`

- **Assessment:** Correct. The tracked README and ADR verification-status statements predated successful Windows CI and therefore inaccurately claimed CI verification had not occurred.
- **Technical rationale:** Verification records must distinguish exact successful CI evidence from remaining desktop and release-candidate workflow gaps. The lifecycle fix also required a new exact-head CI run before documentation could accurately record its verification.
- **Change made:**
  - Updated `README.md` and ADR-001 through ADR-004 to reference the exact successful lifecycle revalidation run and preserve the unverified desktop/release-workflow limitations.
  - Updated `docs/adr/README.md` summary statuses to reflect Windows CI verification while retaining desktop verification pending.
- **Files/lines:** `README.md`, `docs/adr/001-slint-backend-renderer.md`, `docs/adr/002-event-loop-and-tray.md`, `docs/adr/003-system-tray.md`, `docs/adr/004-window-lifecycle.md`, `docs/adr/README.md`.
- **Tests added/changed:** Documentation-only change; ordinary Windows CI was rerun on the resulting exact head.
- **Risk after response:** Documentation now accurately distinguishes CI evidence from remaining manual desktop verification. Release-candidate workflow status is addressed separately by F002.
- **CI evidence:**
  - Documentation candidate SHA: `9081bf418e6a46b381ce9577d8940cc33fef7c4a`.
  - Windows CI: [35565282086](https://github.com/yorelll/quickfolder/actions/runs/35565282086), conclusion `success`.
  - CI artifact: `QuickFolder-0.0.1-windows-x86_64-9081bf418e6a46b381ce9577d8940cc33fef7c4a`.

## Additional Changes

None beyond the findings above. The response does not create a tag, GitHub Release, signed binary, or public publication.

## GitHub Actions Evidence

| Evidence | Commit / candidate | Workflow / run | Conclusion |
|---|---|---|---|
| Lifecycle regression fix | `08bfb527ac20ea0e3d20d2b291f1543383b6b522` | Windows CI [35562858225](https://github.com/yorelll/quickfolder/actions/runs/35562858225) | `success` |
| Documentation revalidation | `9081bf418e6a46b381ce9577d8940cc33fef7c4a` | Windows CI [35565282086](https://github.com/yorelll/quickfolder/actions/runs/35565282086) | `success` |
| Authorized release-candidate package | `9081bf418e6a46b381ce9577d8940cc33fef7c4a` | Build release candidate [35565687100](https://github.com/yorelll/quickfolder/actions/runs/35565687100) | `success` |

All listed workflows used the Windows x86-64 target and ran the locked format, Clippy, test, Release-build, packaging, dependency-audit, and licensing gates appropriate to their workflow.

## Remaining Manual Items

The following are deliberately not marked as completed by hosted CI and remain for real Windows desktop validation before an application release:

- Silent tray-first startup and taskbar behavior.
- Tray click/menu behavior, clean termination, and Explorer restart recovery.
- Input focus, Chinese IME composition, and keyboard interaction.
- Light/dark taskbar icon readability and 32px tray-icon clarity.
- DPI, multi-monitor, disconnect/reconnect, and taskbar-edge behavior.
- Idle CPU/disk/memory, startup latency, and user-perceived responsiveness.

## Re-review Request

Please independently inspect the updated code, complete diff, new lifecycle regression test, ordinary CI evidence, release-candidate workflow evidence, exact candidate artifact, and this response. Determine the disposition of every `M00-R01` finding and issue either `CHANGES_REQUESTED` or `APPROVED_FOR_MILESTONE`.
