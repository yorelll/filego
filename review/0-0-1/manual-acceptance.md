# FileGo 0.0.1 — Manual Acceptance Record

> Template for the project owner’s real Windows desktop acceptance.
>
> This file intentionally contains no inferred test result. Only the project owner may provide the facts to record. Result values are `PASS`, `FAIL`, `BLOCKED`, or `NOT_TESTED`; any required item that is not `PASS` blocks release unless an independent release reviewer explicitly records an allowed disposition under the project rules.

## Candidate identity

| Field | Value |
|---|---|
| RC commit SHA | `a16d28b015ac5816677f3bba961f8e89962fc119` |
| GitHub Actions workflow/run ID/URL | `Build release candidate` (release.yml), run `35981840362`, https://github.com/yorelll/filego/actions/runs/35981840362 |
| EXE artifact name | `FileGo-0.0.1-windows-x86_64.exe` (inside `FileGo-0.0.1-unsigned-candidate-a16d28b015ac5816677f3bba961f8e89962fc119`) |
| ZIP artifact name | `FileGo-0.0.1-windows-x86_64.zip` |
| SHA-256 manifest name | `FileGo-0.0.1-SHA256SUMS.txt` |
| Locally computed EXE SHA-256 | `e0b50242ebe5c7eb9af7638d5ac9605f1cc95852a6a750446b231e462fba6eca` |
| Locally computed ZIP SHA-256 | `7c36af3fe845eba7f263a2eb5437028904a2564049d7c92e3e63912fd6bd2b28` |
| Hash comparison result | Match: locally recomputed EXE/ZIP SHA-256 equal the values in the workflow-generated `SHA256SUMS` manifest; ZIP `FileGo.exe` is byte-identical to the standalone EXE (same SHA-256) |
| Test date | 2026-09-24 (candidate verification date) |
| Tester | Project owner |

> Candidate-identity values above are the implementation agent's downloaded-artifact verification (hashes, PE debug-directory/path scan, `--version`). They are not desktop acceptance results; every checklist row and real-machine measurement below remains unfilled until the project owner performs them.

## Test environment

| Windows version/build | Architecture | Displays/resolution/DPI | Primary IME/version | Network/removable-media environment | Notes |
|---|---|---|---|---|---|
| | x86-64 | | | | |

## Checklist results

Use the complete procedure and expectations in [`task/03-发布手工验收清单.md`](../../task/03-发布手工验收清单.md). Add one row for every exercised checklist item; retain `NOT_TESTED` rather than inferring a pass.

| Checklist ID | Result (`PASS` / `FAIL` / `BLOCKED` / `NOT_TESTED`) | Evidence / observations |
|---|---|---|
| B1 | | |
| B2 | | |
| B3 | | |
| B4 | | |
| B5 | | |
| B6 | | |
| B7 | | |
| C1 | | |
| C2 | | |
| C3 | | |
| C4 | | |
| C5 | | |
| C6 | | |
| C7 | | |
| C8 | | |
| C9 | | |
| C10 | | |
| D1 | | |
| D2 | | |
| D3 | | |
| D4 | | |
| D5 | | |
| D6 | | |
| D7 | | |
| D8 | | |
| D9 | | |
| D10 | | |
| E1–E16 | | |
| F1–F20 | | |
| G1–G8 | | |
| H1–H10 | | |
| I1–I12 | | |
| J1–J6 | | |
| K1–K11 | | |
| L1–L10 | | |
| M1–M5 | | |

## Required real-machine measurements

| Measurement | Release-EXE procedure / environment | Result | Evidence / notes |
|---|---|---|---|
| 10,000-record search (target <50 ms) | Three runs per representative query; record median and machine specification | | |
| Hotkey to visible (target ≤100 ms) | Five presses; record median and worst value | | |
| Cold start (target ≤1 s) | Five starts; record median and whether startup was silent | | |
| Idle CPU | Ten minutes; record CPU percentage and periodic spikes | | |
| Idle disk activity | Observe data directory and relevant activity | | |
| Stable working set (target ≤50 MB) | Record MB and rendering/system baseline context | | |

## Counts and blockers

| Result | Count | Notes |
|---|---|---|
| PASS | | |
| FAIL | | |
| BLOCKED | | |
| NOT_TESTED | | |

- Blocking issues:
- Non-blocking observations:
- Required retest after any fix:

## Project-owner conclusion

`ACCEPTED` / `REJECTED` / `INCOMPLETE`

- Date:
- Project owner:

> This record is not release approval. The next required gate is an independent release code review that evaluates the actual candidate SHA, artifact evidence, and these owner-supplied results and explicitly reaches `APPROVED_FOR_RELEASE`.
