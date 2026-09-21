# Review: M00 Foundation (Round 03)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m00-foundation`
- **Round:** `03`
- **Date:** 2026-09-21
- **Reviewer agent:** Independent M00 code-review agent
- **Role declaration:** Code-review only; no repository files were modified.
- **Independence statement:** The reviewer did not implement the reviewed M00 code, workflow, fixes, responses, or review documents.
- **Original review:** `review/0-0-1/m00-foundation-review-r01.md`
- **Implementation response:** `review/0-0-1/m00-foundation-response-r01.md`
- **Prior re-review:** `review/0-0-1/m00-foundation-review-r02.md`
- **Original reviewed SHA:** `a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Re-review head SHA:** `40501b66a5bc6cf1a68e1718c81b1f2f83f13301`
- **Comparison range:** `b63c5dcf4023e57c36e1d64e47af4f7a371a000c..40501b66a5bc6cf1a68e1718c81b1f2f83f13301`
- **Scope re-reviewed:**
  - `README.md`
  - `docs/adr/001-slint-backend-renderer.md`
  - `docs/adr/002-event-loop-and-tray.md`
  - `docs/adr/003-system-tray.md`
  - `docs/adr/004-window-lifecycle.md`
  - `docs/adr/README.md`
  - `review/0-0-1/m00-foundation-review-r02.md`
  - current CI, release-candidate workflow, candidate artifact, and prior lifecycle-fix evidence
- **Requirements:** M00 checklist; `REQ-REL-001..006`; M00 lifecycle and platform-spike prerequisites.
- **Current ordinary CI:** [Windows CI run `35568497899`](https://github.com/yorelll/quickfolder/actions/runs/35568497899)
- **Current ordinary CI SHA:** `40501b66a5bc6cf1a68e1718c81b1f2f83f13301`
- **Current ordinary CI conclusion:** `success`

## Evidence Reviewed

| Evidence | Commit / candidate | Workflow / run | Conclusion |
|---|---|---|---|
| Lifecycle regression revalidation | `08bfb527ac20ea0e3d20d2b291f1543383b6b522` | [Windows CI 35562858225](https://github.com/yorelll/quickfolder/actions/runs/35562858225) | `success` |
| Response-document revalidation | `b63c5dcf4023e57c36e1d64e47af4f7a371a000c` | [Windows CI 35567214670](https://github.com/yorelll/quickfolder/actions/runs/35567214670) | `success` |
| Release-candidate workflow validation | `9081bf418e6a46b381ce9577d8940cc33fef7c4a` | [Build release candidate 35565687100](https://github.com/yorelll/quickfolder/actions/runs/35565687100) | `success` |
| r02 documentation correction | `40501b66a5bc6cf1a68e1718c81b1f2f83f13301` | [Windows CI 35568497899](https://github.com/yorelll/quickfolder/actions/runs/35568497899) | `success` |

The current ordinary CI run passed all required Windows x86-64 primary gates:

1. checked-in lockfile and immutable dependency verification;
2. `cargo fmt --all -- --check`;
3. Clippy with warnings denied;
4. all tests;
5. Release build;
6. executable/version-helper smoke check;
7. dependency, source, advisory, and license policy audit;
8. third-party license inventory generation and sanity checks;
9. portable ZIP/SHA-256 creation and artifact upload.

The release-candidate workflow independently exercised exact SHA dispatch and checkout, candidate version validation, locked quality gates, dependency/license audit, unsigned EXE/ZIP/SHA-256 packaging, and candidate artifact upload.

The independently inspected release-candidate artifact was:

```text
QuickFolder-0.0.1-unsigned-candidate-9081bf418e6a46b381ce9577d8940cc33fef7c4a
```

Verified candidate hashes:

- `QuickFolder-0.0.1-windows-x86_64.exe`
  `680429a0735a08d88eb7a97a5bd99720fee4ccd668ede1bd3708026c63f157e6`
- `QuickFolder-0.0.1-windows-x86_64.zip`
  `e7be8eb5843b2919cf600c0841a30934ecd8cd017800b8a9745a01bcb71605ee`

Both matched CI-generated `QuickFolder-0.0.1-SHA256SUMS.txt`. The standalone executable and ZIP-contained executable both have valid `MZ` PE headers. The ZIP contains:

- `QuickFolder.exe`
- `README.md`
- `LICENSE`
- `THIRD_PARTY_LICENSES.html`
- `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`

## Re-review Method

- Re-read r01, the committed r01 implementation response, and r02.
- Inspected the exact current tree and committed diff from `b63c5dc` to `40501b6`.
- Rechecked the corrected README and ADR verification-status claims.
- Independently inspected current ordinary CI evidence for exact head `40501b6`.
- Reconfirmed the earlier lifecycle regression and release-candidate workflow evidence.
- Did not run Rust locally, in accordance with `CLAUDE.md`.
- Did not perform interactive Windows desktop testing.

## Prior Finding Disposition

| Finding | Disposition | Evidence |
|---|---|---|
| `M00-R01-F001` | **CLOSED** | `src/app.rs` preserves `LifecycleState::Exiting` through the direct native-close path. Regression test `native_close_after_exit_preserves_terminal_state` verifies Exit → native close → Show is terminal and records only `quit`. Windows CI run `35562858225` passed. |
| `M00-R01-F002` | **CLOSED** | Release-candidate workflow run `35565687100` succeeded for exact candidate SHA `9081bf418e6a46b381ce9577d8940cc33fef7c4a`. Exact checkout, locked gates, audit, package, checksum, and artifact upload were independently exercised and inspected. |
| `M00-R01-F003` | **CLOSED** | Current `README.md:7`, ADR-001 through ADR-004 verification-status lines, and `docs/adr/README.md:7-10` correctly record both ordinary CI and release-candidate workflow success while retaining desktop behavior as unverified. Exact-head CI run `35568497899` passed. |
| `M00-R02-F001` | **CLOSED** | The stale release-workflow status was corrected in `40501b6`; each affected document identifies release-candidate workflow run `35565687100` as successful and leaves only real Windows desktop behavior unverified. |

## Requirement Assessment

| Requirement / M00 criterion | Status | Evidence / notes |
|---|---|---|
| Rust verification only through GitHub Actions | PASS | Exact-head Windows CI `35568497899` passed; no local Rust execution claimed. |
| Immutable dependency lock | PASS | `Cargo.lock` is committed; CI passed frozen/locked dependency validation. |
| Formatting, Clippy, tests, and Release build | PASS | Passed in current ordinary CI and prior release-candidate workflow. |
| Release EXE and version smoke | PASS | Passed in both CI workflows; independently verified PE headers. |
| Dependency/source/license/advisory audit | PASS | Windows x86-64 target-scoped `cargo-deny` and `cargo-about` gates passed. No advisory ignore was introduced. |
| Portable ZIP and SHA-256 | PASS | Ordinary and candidate package workflows succeeded; candidate checksum manifest and package contents were independently verified. |
| Lifecycle state machine | PASS | Terminal shutdown-state regression is fixed, covered by a focused test, and Windows CI validated. |
| Release workflow actual validation | PASS | Exact-SHA `workflow_dispatch` validation completed successfully without publishing a tag or GitHub Release. |
| Verification-status documentation | PASS | Current README and ADRs accurately distinguish CI/release-workflow evidence from desktop-only gaps. |
| Real Windows desktop behavior | UNVERIFIED | Expected manual validation scope remains explicitly recorded. |

## New Findings

No new concrete defects were found in the re-reviewed code, workflows, documentation correction, CI evidence, or release-candidate artifact evidence.

## Cross-Cutting Checks

### Correctness and Error Handling

- Terminal shutdown remains protected in both command-driven and native-close paths.
- Queued window actions cannot revive the lifecycle state once `Exiting` is set.
- Initial quit failure remains retryable.
- CI passes all lifecycle tests for the exact current head.

### Data Safety, Privacy, and Security

- No recursive deletion, real-folder deletion, path-opening shell command, disk scan, telemetry, network client, or user-path/query logging was introduced.
- M00 still contains no user-data persistence or folder operation implementation; later milestones remain responsible for safe JSON persistence and folder semantics.
- No project-owned unsafe code exists.

### Supply Chain and Licensing

- `Cargo.lock` remains immutable and target-scoped audit policy remains explicit for Windows x86-64.
- CI passed `cargo-deny` advisory, source, bans, and licensing checks.
- CI passed `cargo-about --locked --fail --all-features --target x86_64-pc-windows-msvc`.
- The candidate artifact includes generated third-party license inventory and the vendored Slint royalty-free license text.
- README badge and in-app `AboutSlint` remain available for the selected Slint license basis.

### Windows Behavior and Artifact Controls

- CI establishes Windows x86-64 compilation, linking, resource embedding, package construction, candidate checksum generation, and read-only release-workflow behavior.
- The release workflow has no publishing, tag creation, signing, or GitHub Release capability.
- The current M00 approval does **not** grant release approval.

## Remaining Manual / Platform Validation Gaps

These items are intentionally not treated as M00 CI defects and remain required before an application release:

- silent tray-first startup and absence of unintended taskbar entry;
- tray click/menu interaction and clean process termination;
- Explorer restart and `TaskbarCreated` tray recovery;
- window focus after opening;
- Chinese IME composition and candidate interaction;
- light/dark taskbar icon clarity;
- 100%, 125%, 150%, and 200% DPI behavior;
- multi-monitor, disconnect/reconnect, and taskbar-edge behavior;
- idle CPU, disk activity, memory, startup latency, and perceived responsiveness.

## Verdict

`APPROVED_FOR_MILESTONE`

### Rationale

All r01 and r02 findings are independently verified as closed on the reviewed history. The current exact head has a successful ordinary Windows CI run, while the release-candidate workflow has independently proven exact-SHA candidate packaging, checksums, dependency policy enforcement, and unsigned artifact upload. Documentation accurately records that evidence and preserves the real desktop validation boundary.

This approval is limited to the **M00 milestone**. It does not authorize a `0.0.1` tag, GitHub Release, or `APPROVED_FOR_RELEASE`; those remain subject to later implementation, manual acceptance, release-candidate review, and the project’s release gates.
