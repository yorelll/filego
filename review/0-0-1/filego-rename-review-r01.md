# Review: FileGo Rename and Dual-Toolchain Transition (Round 01)

## Metadata

- **Version:** `0.0.1`
- **Topic:** `filego-rename`
- **Round:** `01`
- **Date:** 2026-09-21
- **Reviewer role:** Independent code-review agent; no files modified.
- **Independence statement:** The reviewer did not implement the FileGo rename, toolchain policy, workflow changes, or validation runs.
- **Base SHA:** `138761d4c4e9bc81821d12e73b1a34f889263fa6`
- **Head SHA:** `e5cd1968072e9e59f35493141b39040c6c64e67c`
- **Comparison range:** `138761d..e5cd196`
- **Repository:** `https://github.com/yorelll/filego`
- **Scope:** Product/crate/binary/asset naming; local GNU validation policy; remote MSVC CI and release-workflow target/path enforcement; artifact naming; current documentation; historical audit evidence preservation.

## Evidence Reviewed

| Evidence | Commit / candidate | Workflow / run | Conclusion |
|---|---|---|---|
| FileGo MSVC CI | `c42f5728ef6b8a78e458bea14d5e65a406c4b042` | [Windows CI 35587340136](https://github.com/yorelll/filego/actions/runs/35587340136) | `success` |
| FileGo documentation CI | `e5cd1968072e9e59f35493141b39040c6c64e67c` | [Windows CI 35592204821](https://github.com/yorelll/filego/actions/runs/35592204821) | `success` |
| FileGo release-candidate workflow | `3c42413cad0c1f3eb34ede4de0622246ef8cd903` | [Build release candidate 35590444471](https://github.com/yorelll/filego/actions/runs/35590444471) | `success` |
| Local GNU validation | FileGo rename worktree | Rust `1.92.0-x86_64-pc-windows-gnu` | `success` |

Local GNU validation was performed with Rustup auto-install disabled and passed:

- `cargo fmt --all -- --check`;
- Clippy with `-D warnings`;
- 11 unit tests plus zero-test binary/doc-test targets;
- GNU Release build.

The FileGo release-candidate workflow independently validated exact SHA dispatch/check-out, version validation, locked quality gates, MSVC build, dependency/license audits, unsigned EXE/ZIP/SHA-256 packaging, and candidate artifact upload.

The FileGo candidate artifact was independently checked:

- Artifact: `FileGo-0.0.1-unsigned-candidate-3c42413cad0c1f3eb34ede4de0622246ef8cd903`
- ZIP contents include `FileGo.exe`, `README.md`, `LICENSE`, `THIRD_PARTY_LICENSES.html`, and `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`.
- The packaged EXE has a valid `MZ` PE header.
- Artifact checksum validation was reported successful.

## Requirement Assessment

| Requirement | Status | Evidence |
|---|---|---|
| Current product naming is FileGo | PASS | `Cargo.toml`, Rust imports, UI, docs, templates, artifact names, license header, assets, and repository URL use FileGo/filego. |
| Rust package and binaries renamed consistently | PASS | `Cargo.toml:name = "filego"`, `Cargo.lock` root package, `filego::` imports, `filego-version` binary, CI helper paths, and version output all align. |
| Asset paths and Windows resource embedding renamed | PASS | `assets/icons/filego*.{png,ico}`, `build.rs`, Slint `@image-url()` references, and icon-processing script are consistent. |
| Local GNU verification is permitted but bounded | PASS | `CLAUDE.md:27-58` requires `RUSTUP_AUTO_INSTALL=0`, an exact GNU toolchain preflight, locked commands, and distinguishes local feedback from release evidence. |
| Remote CI and release use MSVC x86-64 | PASS | CI/release Clippy, tests, and Release builds explicitly use `--target x86_64-pc-windows-msvc`; verification/packaging paths use `target\\x86_64-pc-windows-msvc\\release\\...`. |
| MSVC artifact and release-candidate names are FileGo | PASS | CI/release workflows emit FileGo EXE/ZIP/SHA/artifact names; FileGo ordinary and candidate workflows passed. |
| Historical audit evidence retained accurately | PASS | Historical M00 r01/r02/r03/response records retain original QuickFolder artifact names, URLs, and commit evidence; current review index explicitly explains the rename. |
| Current app data path naming | PASS | `.filego-data` and planned `%LOCALAPPDATA%\\FileGo` references are used in current project-owned configuration/plans. |
| Real desktop validation | UNVERIFIED | Tray behavior, Explorer recovery, IME, DPI, focus, taskbar behavior, multi-monitor, and idle-resource observations remain manual validation work. |

## Findings

### `FILEGO-R01-F001` — Low — Active local workspace and retained external brief keep the former name

- **Location:** `D:\work\tools\quickfolder\` and `D:\work\tools\quickfolder项目描述.md`
- **Category:** Naming migration / local environment
- **Problem:** The Git-tracked project content, remote repository, artifact names, crate/binaries, and active paths are renamed to FileGo. However, the active local repository root and the retained external product brief still include `quickfolder`.
- **Impact:** This does not affect Cargo package identity, application data location, MSVC CI, release artifacts, GitHub URLs, or release behavior. It remains a local filesystem naming inconsistency relative to a literal reading of the requested complete rename.
- **Evidence:** Current project root remains `D:\work\tools\quickfolder`; `.gitignore` retains the old brief filename as a compatibility ignore rule. Historical audit records intentionally retain old names and are not part of this finding.
- **User direction:** The project owner explicitly directed that the active local root and retained external brief **not** be renamed because doing so does not affect product behavior and risks active session records. This direction is valid risk control for the current working session.
- **Required change:** The implementation response must explicitly record this accepted non-change, its rationale, and the fact that the old local filesystem names are not product/release identifiers. A future repository relocation, if ever desired, must be a separately authorized and session-safe filesystem operation; it is not required for the current product or release state.
- **Release blocking:** No.

## Cross-Cutting Checks

### Rename Integrity

- Current project-owned code/configuration references consistently use `FileGo` for display/artifact naming and `filego` for Rust/package/path identifiers.
- The repository URL points to `yorelll/filego`.
- Historical review/response evidence retains former names exactly where necessary to preserve artifact, URL, and commit evidence.

### GNU/MSVC Policy

- The local GNU policy is safely constrained: it disables implicit Rustup installation and prevents local success from being represented as MSVC or release evidence.
- The remote MSVC policy is explicit in both ordinary CI and candidate workflow commands and output paths.
- `cargo fmt` intentionally has no target because it does not produce target-specific output.
- `cargo metadata --frozen` intentionally has no build target because it is used only for lock/metadata validation.

### Data Safety, Privacy, and Security

- The rename did not introduce deletion APIs, shell command construction, telemetry, disk scanning, user-path/query logging, or unsafe Rust.
- Dependency, source, advisory, and license gates passed after the rename for the MSVC target.

### Remaining Manual Validation

Before any application release, manual Windows checks still include tray-first startup, taskbar presence, tray interactions, Explorer restart, focus behavior, Chinese IME, high DPI, multi-monitor behavior, theme/icon readability, idle resource use, and responsiveness.

## Verdict

`CHANGES_REQUESTED`

### Rationale

The Git-tracked FileGo rename, dual-toolchain policy, remote MSVC CI target enforcement, artifact naming, local GNU validation, MSVC CI, and FileGo release-candidate workflow are verified and internally consistent.

The sole remaining issue is the former local repository directory and retained external brief filename. The project owner has explicitly directed that they not be renamed during this session because of session-record risk and no product-behavior impact. The implementation response must record that deliberate accepted exception and rationale. This review does not grant release approval.
