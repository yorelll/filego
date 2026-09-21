# Review: M00 Foundation (Round 01)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m00-foundation`
- **Date:** 2026-09-21
- **Reviewer agent:** Independent M00 code-review agent
- **Role declaration:** Code-review only; no files were modified.
- **Independence statement:** The reviewer did not participate in implementing the reviewed M00 changes.
- **Base SHA:** Empty repository / no parent commit
- **Head SHA:** `a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Scope:** Entire initial M00 tree, including Rust/Slint shell, Windows CI and release workflows, lockfile, dependency policy, ADRs, assets, review templates, and packaging.
- **Requirements:** M00 checklist; `REQ-REL-001..006`; M00 lifecycle and platform-spike prerequisites.
- **CI workflow:** Windows CI
- **CI run:** [35561214563](https://github.com/yorelll/quickfolder/actions/runs/35561214563)
- **CI conclusion:** `success`
- **CI head SHA:** `a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Artifact:** `QuickFolder-0.0.1-windows-x86_64-a71e2e1d8ee18f6e14b4110127d61d75ad2a3ca7`
- **Artifact evidence reviewed:**
  - Downloaded ZIP SHA-256: `2f8bd5c755344b0228b612f43cad442e0bd0979e9e0294f30af56d68e5b52c92`
  - Hash matched the CI-produced `.sha256` file.
  - ZIP contained `QuickFolder.exe`, `README.md`, `LICENSE`, `THIRD_PARTY_LICENSES.html`, and `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`.
  - `QuickFolder.exe` has an `MZ` PE header.

## Review Method

- Inspected the complete root-to-head repository tree and commit history.
- Read project rules, M00 task plan, requirement/test matrix, README, Cargo manifest and lockfile, Rust sources, Slint source, build script, ADRs, dependency policies, review templates, CI workflow, and release workflow.
- Reviewed successful Windows CI evidence for the exact head commit.
- Reviewed the downloaded CI artifact and recorded artifact hash/package-content evidence.
- Did not run Rust locally, consistent with `CLAUDE.md`.
- Did not perform interactive Windows desktop validation.

## Requirement Assessment

| Requirement / M00 criterion | Status | Evidence |
|---|---|---|
| Rust verification only through GitHub Actions | PASS | `CLAUDE.md:27-87`; Windows CI run `35561214563` ran all Rust gates. |
| Checked-in immutable dependencies | PASS | `Cargo.lock` committed; CI passed `cargo metadata --frozen`; independent lockfile audit approved the 563-package lockfile. |
| Formatting, Clippy, tests, and Release build | PASS | CI steps 9–12 in run `35561214563` succeeded. |
| Release EXE existence and version smoke | PASS | CI step 13 succeeded; artifact PE header independently checked. |
| Dependency/license/source audit | PASS | CI steps 14–17 succeeded with target-scoped Windows x86-64 graph and generated license inventory. |
| Portable ZIP and SHA-256 | PASS | CI steps 18–19 succeeded; downloaded ZIP hash and required contents independently checked. |
| Minimal tray-first lifecycle architecture | PARTIAL | Compiles and lifecycle unit tests pass, but terminal shutdown can be bypassed by the native close callback; see finding `M00-R01-F001`. |
| Release workflow actual validation | UNVERIFIED / BLOCKING | `.github/workflows/release.yml` has not been dispatched against a candidate commit; see finding `M00-R01-F002`. |
| Truthful public/audit documentation | PARTIAL | README and several ADR verification-status statements still say CI was not run; see finding `M00-R01-F003`. |
| Real desktop behavior | UNVERIFIED | Tray, native close behavior, taskbar/focus, DPI, IME, Explorer restart, and icon appearance require a Windows desktop test. |

## Findings

### `M00-R01-F001` — High — Native close callback can clear the terminal shutdown state

- **Location:** `src/app.rs:47-52`, `src/app.rs:59-64`, `src/main.rs:52-57`
- **Category:** Correctness / Windows lifecycle
- **Problem:** `LifecycleController::handle()` correctly treats `Exiting` as terminal, but the native close callback bypasses `handle()` and calls `accept_window_close()`. That method unconditionally assigns `Hidden`, including after tray Exit has already changed the state to `Exiting`.
- **Trigger / reproduction:**
  1. The tray Exit callback calls `handle(ExitFromTray, ...)`, invokes `quit_event_loop()`, and stores `LifecycleState::Exiting`.
  2. Before the event loop fully stops, a queued native close-request callback is dispatched.
  3. `app.window().on_close_requested()` calls `accept_window_close()`, which sets the state to `Hidden`.
  4. A subsequent queued tray Open/Show callback is no longer blocked by the terminal-state guard and can call `window.show()`.
- **Impact:** The state machine no longer guarantees that shutdown remains terminal. It can permit a late window operation during shutdown and undermines the explicit lifecycle design and terminal-state test intent.
- **Evidence:** `handle()` has an early `Exiting` guard at `src/app.rs:62-64`, while `accept_window_close()` bypasses that guard and unconditionally assigns `Hidden` at `src/app.rs:50`. The native close callback invokes it directly at `src/main.rs:54-56`.
- **Required change:** Preserve `Exiting` in `accept_window_close()`—for example, return `Exiting` without assignment when the current state is terminal. Add a unit test that performs `ExitFromTray`, calls `accept_window_close()`, then confirms that a later `Show` remains a no-op and the only platform action remains `quit`.
- **Release blocking:** Yes.

### `M00-R01-F002` — Medium — Release workflow has never been executed

- **Location:** `.github/workflows/release.yml:1-171`
- **Category:** CI / release workflow verification
- **Problem:** The candidate-packaging workflow is substantial and security-relevant, but no actual run evidence exists for it.
- **Trigger / reproduction:** Dispatch `Build release candidate` with a valid reviewed candidate SHA and version. Until run, failures in exact-SHA checkout, workflow input handling, cargo-about/cargo-deny execution, target-specific report generation, EXE/ZIP/SHA packaging, or artifact upload remain undiscovered.
- **Impact:** This conflicts with the project rule that workflow changes must be verified by an actual workflow run. A successful ordinary CI run does not exercise `workflow_dispatch`, `candidate_sha` validation, exact checkout, candidate artifact naming, or candidate SHA checksum-file construction.
- **Evidence:** The reviewed successful run is Windows CI `35561214563`; it exercised `.github/workflows/ci.yml`, not `.github/workflows/release.yml`. The release workflow has no recorded run ID or artifact evidence.
- **Required change:** After a candidate SHA is approved for milestone verification and with appropriate authorization, dispatch the release-candidate build workflow using an exact SHA and `0.0.1`; record the resulting run, artifact names, and hashes. Do not publish a GitHub Release.
- **Release blocking:** Yes for release workflow approval; milestone approval should remain conditional until the project’s workflow-validation rule is satisfied.

### `M00-R01-F003` — Low — Public and ADR verification status is stale after successful CI

- **Location:** `README.md:7`, `docs/adr/001-slint-backend-renderer.md:3,49-51`, `docs/adr/002-event-loop-and-tray.md:3,44-46`, `docs/adr/003-system-tray.md:43-47`, `docs/adr/004-window-lifecycle.md:45-50`, `docs/adr/README.md:7-10`
- **Category:** Documentation / audit accuracy
- **Problem:** Multiple tracked documents still state that M00 has not run GitHub Actions or that compilation/tests are pending, despite CI run `35561214563` passing formatting, Clippy, tests, Release build, packaging, dependency audit, and artifact upload for the exact reviewed head commit.
- **Impact:** Public status and design evidence are inaccurate. This can mislead future implementers and reviewers about what has been verified versus what remains desktop-only or release-workflow-only.
- **Evidence:** `README.md:7` says M00 has not undergone first GitHub Actions build. ADR-001 says “Current status: not run”; ADR-002 says no GitHub Actions run; ADR-003 says not verified by GitHub Actions; ADR-004 says tests not executed. These conflict with run `35561214563`.
- **Required change:** Update current-status lines to distinguish:
  - verified by run `35561214563` at SHA `a71e2e1…`;
  - still unverified real desktop behavior;
  - still unverified release-candidate workflow.
  Preserve ADR decision history; only correct the verification-status facts.
- **Release blocking:** No by itself, but should be fixed before milestone approval documentation is finalized.

## Cross-Cutting Checks

### Correctness and Error Handling

- The pure lifecycle controller is a strong M00 boundary: `WindowPort` isolates `show`, `hide`, and `quit` effects from state transitions.
- Unit tests cover initial hidden state, toggle behavior, close-to-hide, explicit exit, side-effect failure rollback, terminal state under commands routed through `handle()`, and exit retry after failure.
- The review found the native close callback bypass described in `M00-R01-F001`; the existing terminal-state test does not cover that direct callback path.
- `EventLoopError` is deliberately converted to `PlatformError` at `src/main.rs:35-41`; Windows CI built and tested this exact code.

### Data Safety and “Never Delete Real Folders”

- M00 contains no folder record model, path operation, recursive deletion API, shell invocation, or filesystem deletion operation.
- `src/platform/windows/mod.rs` and ADR-005 explicitly prohibit exposing recursive directory deletion.
- No issue found for M00 scope.

### Privacy, Logging, and Network Behavior

- No application telemetry, network client, account, upload, path search, or disk-scanning code is present.
- Runtime error logs are generic strings only at `src/main.rs:113-117` and `src/main.rs:128-130`.
- The current error conversion is not printed, avoiding accidental path leakage through future platform errors.
- Direct dependency lockfile review found no explicit application network/telemetry dependency; CI cargo-deny passed the Windows release graph.

### Security and Supply Chain

- `Cargo.lock` is checked in and `--locked` / `--frozen` gates are used.
- Workflow actions are pinned to full commit SHAs.
- `cargo-deny` passes with target-scoped Windows x86-64 audit, no advisory ignore list, denied wildcard dependencies, denied unknown registry/Git sources, and allowed crates.io registry.
- `cargo-about` receives `--locked --fail --all-features --target x86_64-pc-windows-msvc`.
- Slint royalty-free license text is vendored and copied into artifacts. The project includes both README badge attribution and the `AboutSlint` widget.
- No project-owned unsafe code exists.
- No issue found in the reviewed M00 supply-chain controls.

### Windows Behavior

- Successful CI proves the selected Slint APIs, system tray declarative component, icon assets, Winit backend, Windows resources, and Win32 linking compile for Windows x86-64.
- Actual runtime tray behavior is not established by hosted CI.
- The M00 design correctly avoids a second event loop and relies on Slint’s tray integration.
- `M00-R01-F001` must be corrected before lifecycle behavior can be approved.

### Tests and CI Quality

- CI exact-head evidence is strong for the ordinary Windows workflow:
  - `a71e2e1…`
  - Windows CI `35561214563`
  - all 19 primary steps either succeeded or were intentionally skipped when lockfile already existed.
- Artifact hash/package inspection provides independent post-CI evidence.
- Unit test coverage is appropriate for M00’s lifecycle state machine but needs the native-close-after-exit regression test.
- Release workflow remains unexecuted; see `M00-R01-F002`.

### Performance

- M00 has no disk scan, polling loop, background worker, or periodic timer.
- Idle CPU, memory, and actual software-renderer behavior have not been measured and remain future validation work.

### Accessibility, Internationalization, and Assets

- The initial UI uses Chinese M00 shell text and Slint standard controls, but complete zh-CN/en-US parity is future M03/M06 work.
- `AboutSlint` attribution is included.
- Owner-provided icon assets have provenance, source hashes, reproducible processing documentation, and multi-size ICO packaging.
- Tray icon legibility at 32px, different taskbar themes, and DPI scaling remain manual checks.

### Maintainability

- Module boundaries for domain, storage, presentation, Windows platform adapters, diagnostics, and lifecycle controller are clear and appropriately minimal.
- ADRs identify deferred M04/M07 responsibilities instead of prematurely embedding Win32 APIs in presentation callbacks.
- Review and response templates comply with the documented evidence workflow.

## Residual Risks and Required Manual Checks

The following are not findings against M00 CI compilation, but remain unverified on a real Windows desktop:

- Silent tray-first startup and absence of unintended taskbar entry.
- Native close-to-hide after M00-R01-F001 is fixed.
- Tray left-click toggle, context-menu Open/Exit behavior, and clean process termination.
- Explorer restart and `TaskbarCreated` tray-icon recovery.
- Foreground focus when opening the window.
- Light/dark taskbar readability and 32px tray icon clarity.
- Chinese IME composition and candidate interaction.
- 100%, 125%, 150%, and 200% DPI behavior.
- Multi-monitor placement, disconnect/reconnect behavior, and taskbar edge behavior.
- Idle CPU, idle disk activity, startup latency, and memory observations.
- Release-candidate workflow dispatch and artifact verification for an exact candidate SHA.

## Verdict

`CHANGES_REQUESTED`

### Rationale

The ordinary Windows CI, dependency audit, Release compilation, version smoke, and portable artifact packaging are all successful for the exact reviewed commit. The M00 architecture, separation of concerns, privacy posture, license handling, and artifact evidence are generally sound.

However, `M00-R01-F001` is a concrete lifecycle correctness defect: the native close callback can clear the terminal `Exiting` state and bypass the state-machine guard. `M00-R01-F002` also leaves the release-candidate workflow unverified despite the project’s explicit rule requiring actual workflow execution after workflow changes. These must be addressed and independently re-reviewed before `APPROVED_FOR_MILESTONE`.</result>Ихадоу】【：】【“】【assistant to=functions.PowerShell კომენტary  手机上天天中彩票json<br/>{