# FileGo M07 稳定性、异常、性能、无障碍与安全加固 — 独立 Code Review r02

## 元数据

- **版本**: 0.0.1（目标）
- **Topic**: `m07-hardening`
- **轮次**: r02（对 r01 修复 commit 的复审）
- **日期**: 2026-09-21（本次复审实际核查 2026-09-24 的 fix/response/CI 时间线）
- **被评审 commit（head SHA）**:
  - 代码修复 commit（fix head）: `e3e86a5`（`fix: refuse oversized imports before buffering and record exit-save rationale`）
  - 文档 commit（docs head）: `596472f`（response + known-issues 重写）
  - head（当前工作树）: `ec09108`（`docs: fill M07 response fix-head CI run IDs`，纯文档）
- **Base SHA（比较基准）**: `367bb8c`（r01 评审对象 head，`feat: harden stability, resources, security and RC readiness`）
- **比较范围**: `367bb8c..HEAD`
  - `git log --oneline 367bb8c..HEAD` = 4 个 commit：
    - `c462a0d` docs: record M07 hardening review r01（r01 文档入库）
    - `e3e86a5` fix（唯一代码提交：`src/main.rs` + `src/storage/import_export.rs`）
    - `596472f` docs（response + known-issues 重写 + typo 修正）
    - `ec09108` docs（补 fix-head CI run ID）
- **文件范围**:
  - `git diff --stat 367bb8c..HEAD` = **5 文件 +496/−12**：`src/main.rs`（+25）、`src/storage/import_export.rs`（+57）、`review/0-0-1/m07-known-issues.md`（+10/-5）、`review/0-0-1/m07-hardening-review-r01.md`（+206）、`review/0-0-1/m07-hardening-response-r01.md`（+210）
  - **代码改动仅 `e3e86a5` 一单提交、恰 2 个源文件**（`git show e3e86a5 --name-only` 实测）；`e3e86a5..HEAD` 仅 2 个 review 文档（response、known-issues），无任何其他代码改动。
  - **明确零改动**: `.github/workflows/*`、`Cargo.toml`、`Cargo.lock`、`deny.toml`、`about.toml`、数据安全核心（`src/storage/{repository,io,codec,schema,location}.rs`、`src/diagnostics.rs`、`src/app.rs`、`src/search/**`、`src/domain/**`、`src/platform/**`）在任何 fix/response commit 中均为零改动（`git diff e3e86a5^ e3e86a5 -- <上述路径>` 为空）。

## Reviewer 角色与独立性声明

- **Reviewer**: 独立 code-review agent（本评审）。
- **独立性声明**: 本 agent **未参与** M07（`367bb8c` 与 `e3e86a5`）的实现、修复、测试编写、提交与 CI 触发，也未参与 M00–M06 任何实现或 M07 r01 response 的撰写。本评审期间 **未编辑任何源代码、未提交、未推送、未运行任何 `cargo` 命令**（CLAUDE.md §3.1），未使用本地 Rust 工具链做任何构建/测试。除本 review 文档外未写入任何其他 Git 追踪文件。所有结论来自对实际代码、`git diff`/`git show`、GitHub Actions 日志（通过 `gh.exe` 独立拉取）与既有审阅证据链的独立查阅，不采信实现报告的任何断言。
- **工具**: GitHub CLI 使用绝对路径 `D:\Program Files\GitHub CLI\gh.exe`。

## CI 证据（独立复核）

本次复审通过 `gh.exe run view` 对全部相关 run **逐一独立核验**（不采信 response 文档自我声明的 run ID）：

| 项目 | Commit（head SHA） | Workflow / Run | 独立核验结果 | 结论 | 关键 job |
|---|---|---|---|---|---|
| Windows CI | `596472f24e83b77700a190b57830fc295409ab` | [Windows CI 35958301326](https://github.com/yorelll/filego/actions/runs/35958301326) | `conclusion=success`、`status=completed`、（本次为 docs commit 触发的 fix-head run） | **success** | fmt、clippy `-D warnings`、test、release build、EXE/版本、deny、about、portable artifact |
| Search benchmark | `596472f…` | [Search benchmark 35958301284](https://github.com/yorelll/filego/actions/runs/35958301284) | `conclusion=success`、`status=completed` | **success** | release 10k bench |
| Windows CI（M07 权威 run） | `367bb8c1…365d1` | [Windows CI 35950631364](https://github.com/yorelll/filego/actions/runs/35950631364) | `conclusion=success`、`headSha=367bb8c…` | **success** | r01 评审对象 head |
| Search benchmark（权威） | `367bb8c1…365d1` | [Search benchmark 35950631427](https://github.com/yorelll/filego/actions/runs/35950631427) | `conclusion=success`、`headSha=367bb8c…` | **success** | r01 评审对象 head |

- **fix-head 测试计数（run 35958301326 日志直接提取，本 reviewer 独立拉取）**:
  - `running 395 tests` → `test result: ok. 394 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`（lib，3.84s）
  - `running 4 tests` → `test result: ok. 4 passed; 0 failed`（bin）
  - ignored 的 1 项日志原文：`search::benchmark::tests::ten_k_release_benchmark_stays_under_lenient_bound ... ignored, release-mode wall-time benchmark; run via cargo test --release -- --ignored`。
  - **394+4 = 393（r01 基线）+ F-5 新增 1 项 + 4**，与 response 声称的「394+4」**完全一致**；与 r01 的 393+4 相比仅多出新 F-5 测试 1 项，无回归。
- **MSVC 权威性**: runner 为 `windows-latest`（`x86_64-pc-windows-msvc`）。fix 与 response 提交 `.github/**` 零改动（`git diff 367bb8c..HEAD -- .github/` = 空），workflow 无需重验；head `596472f`/`ec09108` 相对 `e3e86a5` 为纯文档，故 run 35958301326/35958301284 覆盖的就是 **fix 代码 head `e3e86a5`** 的完整质量门禁。

## 需求/验收标准映射（r01 关闭项对 r02 的复审）

> 状态约定：`CLOSED_R02`（r02 确认关闭）、`DISPOSED`（评审接受为已记录/非缺陷）、`PARTIAL`（主体关闭但含新观察，见 Findings）。

| r01 项 | 处置 | r02 复核证据 |
|---|---|---|
| M-1（Medium）CLOSED 表缺旁证出处 | `DISPOSED`（表已自含出处，**但见新 N-1 引证笔误**） | response 建立锚定表（来源 review + 代码/测试 + CI run），run ID 真实（gh 独立核验）。详见 Findings N-1 |
| M-2（Medium）known-issues 性能/内存数字无实测出处 | `CLOSED_R02` | `m07-known-issues.md` 已重写（见下），无 Git 可复算的伪称「已验证」数字 |
| M-3（Medium）缺 Git 追踪 response | `CLOSED_R02` | `m07-hardening-response-r01.md` 已入库（Git 追踪），含真实 run ID + 393+4/394+4 计数 + ignored 项标识，count 经 CI 日志独立复核一致 |
| M-4（Low）退出等待保存/保存不阻塞输入 | `CLOSED_R02`（架构论证记录于代码注释） | 见下节 |
| F-5（Low）导入读取边界 | `CLOSED_R02` | 见下节（代码 + 测试 + CI 全核验） |
| I-1..I-5（Info） | `RECORDED`（逐条核实仍在 response 中，无悬空） | 见 Findings |

## Findings（按严重级排序）

### Critical / High

**未发现。**

### Medium

**未发现新的 Medium 项。**（r01 的 M-1/M-2/M-3 三个 Medium 均在 r02 得到实质关闭，仅 M-1 的产物表含一处文档级引证笔误，降级为新 Low N-1，见下。）

### Low

- **N-1（Low，文档引证笔误，新增）— response `M-1` 锚定表中 M06-I1/I2/I3 三行的「上游来源 review 文档」误写为 `m06-settings-review-r01.md`，实际应为 **`m06-settings-review-r02.md`**。**
  - **文件**: `review/0-0-1/m07-hardening-response-r01.md` 第 55-57 行（M-1 锚定表三行）；对照 `review/0-0-1/m06-settings-review-r01.md` Info 节与 `review/0-0-1/m06-settings-review-r02.md` Info 节。
  - **问题**: 本 reviewer 对每个 ID 的「来源 review 文档」逐份 `grep` 核对。M04-N001（→ r02）、M05-L2/L3（→ r01）、M05-OBS-01/OBS-02（→ r02）、M01B-R02-N001（→ r02）的文档归属**全部正确**；**唯 M06-I1/I2/I3 归属错误**：
    - `m06-settings-review-r01.md` 的 I1 `=设置窗口无 on_close_requested`、I2 `=settings 持久化延迟`、I3 `=持久化失败时的进程内不一致边缘`（r01 映射表中 M-1 已正确引用「M06 r02 有 I1/I2/I3」）。
    - `m06-settings-review-r02.md` 的 I1 `=快照 stamp 秒级粒度（同秒覆盖导入碰撞）`、I2 `=快照在保存失败时多留一份`、I3 `=apply 按钮在 conflict 存在时不禁用`——**与 response 表格描述的议题（同秒 stamp 碰撞、保存失败冗余快照、冲突时 Apply 禁用）完全对应**。
    - 即：response 描述的内容属 r02，却把来源写成 r01。r02 review（line 68、131-133）正是这些 I1/I2/I3 的权威出处；且 r01 line 113 明确写「M06 r02 有 I1/I2/I3」，故这一笔误系 response 撰写时引入，非上游文档问题。
  - **影响**: 文档引证级。该三行的「代码/测试证据」与「CI run」两列**全部正确且真实存在**（`unique_stamp_disambiguates_same_second_backups`、`failed_import_snapshot_cleanup_removes_only_the_named_backup`、`remove_failed_import_snapshot`、slint `s-import-conflicts==0` 均经 `git grep` 核实在位）；内容实质不受影响，唯一缺陷是「来源文档轮次」标错（r01→r02）。M-1 的核心诉求（出处自含、无悬空）已满足，本项不构成数据/正确性/发布物风险。
  - **复现/证据**: `grep -n "^### I[123]" review/0-0-1/m06-settings-review-r01.md` vs `grep -n "I1\|I2\|I3" review/0-0-1/m06-settings-review-r02.md`（见上）。
  - **建议**: implementation agent 在 response 内修正三行来源为 `m06-settings-review-r02.md`（纯文档一字改动）。**不阻塞 M07 里程碑批准**；因 M-1 的议题恰是「出处准确性」，此笔误应在进入 release review 前随任何后续文档提交一并修正（例如与 `manual-acceptance.md` 同批），并（如 reviewer 要求）由一次快速 `git grep` 复核。
  - **处置建议**: `ACCEPTED`（修订为 r02）。

### Info

- **I-1..I-5（r01）— RECORDED 状态保持**: response 逐条记录；其中 I-3（「红action」拼音 typo）已在 `m07-known-issues.md` 修正为「panic hook 脱敏（redaction）」（`git diff 367bb8c..HEAD -- m07-known-issues.md` 实测命中）。I-4（`concurrent_burst_saves_never_torn_main` 用普通 `save`）的测试契约说明合理（单实例语义下直接覆盖正是为了压测锁串行化+原子替换，守 revision 的生产路径走 `save_at`/`save_if_current`）。I-5（benchmark.yml 不跑 deny/about）为既有设计，本轮 `.github` 零改动，无需重验。
- **N-1（新增 Low）已在上方详述。**

## 对 r01 各 finding 的逐项复审

### M-1（Medium）— CLOSED 表缺旁证出处 — **实质关闭（含新 Low N-1）**

- response 建立锚定表，每个 CLOSED 项四要素（上游 review 文档 + 代码/测试证据 + CI run）现已在 Git 追踪的 response 中**自含**；run ID（`35950631364`/`35950631427`）经 gh 独立核验为真（head 367bb8c，success）。
- 9 条 CLOSED 行的代码/测试证据：本 reviewer 逐一 `git grep` 核实在位（`tray_hotkey_hint`、`sync_hotkey_from_native`、slint `hotkey-unavailable-hint`、`unique_stamp`、`remove_failed_import_snapshot`、`failed_import_snapshot_cleanup_removes_only_the_named_backup`、slint `s-import-conflicts`、`undo_dismiss`/`NoticeEnterPath`、`RowAction::from_context_action`、`repair_from_backup` 持锁等）。**全部真实存在**。
- **唯一新观察**: M06-I1/I2/I3 的来源文档轮次标错（r01→应为 r02），见 **N-1**。
- **处置**: M-1 → `DISPOSED`（主体解决）；N-1 → `ACCEPTED`（修订轮次）。

### M-2（Medium）— known-issues 性能/内存数字无实测出处 — **CLOSED_R02**

`m07-known-issues.md` 表 2 三行实测重写（`git diff 367bb8c..HEAD` 逐行核验）：

- `M02-F003` 行：明确「性能数字为本地 debug 快速反馈（~77µs / ~772ms / release ~80ms），**非 MSVC 权威、非 Git 可复算产品断言；为（目标/待真实机器测量），权威数字由 M07.3/桌面性能验收实测后补入 `manual-acceptance.md`**」。
- `M02 review F005` 行：明确「**尚未实测量化，列为（目标/待真实机器测量）**」。
- `空闲 CPU/内存` 行：明确「**静态代码结构结论**（3 个 50ms drain timer 仅 `try_recv`），**不得声称「实测 CPU 0%」**；内存 `<50MB` 为**优化目标（待真实机器测量）**，权威记录计划补入 `manual-acceptance.md`」。

**无任何剩余 Git 可验证为假的数字断言**；所有数字均以「本地 debug 快速反馈 /（目标·待真实机器测量）」措辞出现，权威落盘点明确指向 manual-acceptance。

### M-3（Medium）— 缺 Git 追踪 response — **CLOSED_R02**

- `review/0-0-1/m07-hardening-response-r01.md` 已 Git 追踪（`git ls-files` 命中；head 工作树中该文件实存）。
- 权威 run ID：`35950631364`（Windows CI）+ `35950631427`（Search benchmark），均经 gh 核验为 head `367bb8c` 的 success run；fix-head run `35958301326`+`35958301284` 亦经核验为 success。
- 测试计数 `393+4`（r01）与 `394+4`（fix head）均与 CI 日志**逐字吻合**；ignored 的 1 项确认为 `ten_k_release_benchmark_stays_under_lenient_bound`（`#[ignore]`，仅 release 模式显式运行）。
- 引用悬空已消除（known-issues「说明」行改为指向 response 内的权威 run ID）。

### M-4（Low）— 退出等待保存/保存不阻塞输入 — **CLOSED_R02（架构论证已记录）**

- 论证记录于两处：(a) response M-4 节「架构论证」；(b) `src/main.rs` `tray.on_quit_requested` 注释块（`e3e86a5` 新增，`git show` 实测，含 M-4 索引与「0.0.1 无待保存队列 + 同步小写，不需要 timed primitive」结论）。
- 复核其论证前提：每次 settings 变更即时 `save_at`（编码先行 → 加锁 → 原子替换，失败回滚）；退出走与其它 mutation 相同的单线程串行命令路径；无阻塞调用在 UI 路径。结论成立。
- 真实桌面项 task/03 **L10**（退出期间有未保存改动正确保存或报告失败、不永久卡住）与 **L8**（输入不冻结）保留为 M08 手工项。本项**接受并关闭**。

### F-5（Low）— 导入读取边界 — **CLOSED_R02**

逐项核验 fix（`e3e86a5`）：

1. **`import_file_len_allowed(len: u64) -> bool`**（`src/storage/import_export.rs:204-212`）：`len <= MAX_IMPORT_BYTES as u64` 即放行。纯函数，正确。
2. **`main.rs` `import_data` 预检在 `fs::read` 之前**（`main.rs:1053-1065`，head 实测）：`if let Ok(metadata) = std::fs::metadata(&path) && !import_file_len_allowed(metadata.len())` → 直接 `set_notice(SNotice::ImportTooLarge)` + `DismissDataFlow` + `sync_settings_ui()` + return，**不读文件**。与 parse 超限分支的 UI 行为一致（`main.rs:1091-1095`）。
3. **post-read `parse_import` 字节检查仍为权威上限**（`import_export.rs:234` `if bytes.len() > MAX_IMPORT_BYTES` 保持）——覆盖 stat 与 read 之间文件变化的竞态；预检只**新增**拒绝面，未改动任何既有限制语义。
4. **测试 `import_file_len_pre_check_refuses_oversized_files_before_buffering`**（`import_export.rs:577-604`）：`MAX+1` 预检拒绝；`MAX`/`0` 预检放行；`MAX+1` 字节内容被 `parse_import` 实际拒绝为 `ImportTooLarge` 且预检预测一致。断言真实、非表面测试。
5. **CI 落证**: run `35958301326` lib 计数 394（= r01 的 393 + 新增 1）且 0 失败，新测试已在 MSVC 实跑通过。
6. 语义保持：无 mutation、无路径/query 泄漏、无新依赖（Cargo 零改动）、无 `#[allow]`。

### I-1..I-5（Info）— **RECORDED**（见 Findings 节）

## Cross-cutting 检查（集中于 `367bb8c..HEAD` 差异面）

- **正确性**: F-5 预检为「新增拒绝面」且 post-read 权威检查未动；`import_file_len_allowed` 纯函数边界（`<=` 含等于上限）与 `parse_import` 的 `>` 拒绝语义一致（等于上限放行、+1 拒绝，测试双向证明一致）；M-4 注释与代码行为一致。
- **错误处理**: 预检失败走既有 `ImportTooLarge` 通知 + `DismissDataFlow` 路径，与 parse 超限行为对齐，无新错误面；`metadata()` 失败（文件消失/权限）按 `Err(_)` 短路放行到 `fs::read` 既有错误路径，行为安全。
- **数据安全（绝对不变式）**: fix 不触碰 repository/backup/io 删除面；`remove_dir/remove_dir_all/delete_directory` 全 src 零命中守卫、canary 测试、repair 持锁、panic hook 脱敏、import 不 mutation——全部未被 `e3e86a5` 改动（`git diff e3e86a5^ e3e86a5` 仅 2 个文件，不含任何数据安全核心）。
- **隐私/安全**: 无路径/query 进日志；无新依赖；`Cargo.toml`/`Cargo.lock`/`deny.toml` 零改动；无网络/遥测。
- **Windows 行为**: 无新平台代码；MSVC run 全绿。
- **测试覆盖**: fix 新增 1 项测试落在 F-5 根因上；CI 394+4 全绿（fix head）与 393+4（r01 head）均经日志独立核实。
- **可维护性**: `import_file_len_allowed` 命名清晰、模块文档同步更新（F-5 索引）；response/known-issues 结构可读。
- **性能**: 预检为单次 `stat`，常数级，不进入搜索/保存热路径。
- **依赖与供应链**: 零变更；`.github` 零变更；benchmark 仅 release 10k 门禁（I-5 既有设计）。
- **文档诚实性**: known-issues 数字全部改写为「本地 debug 快速反馈 /（目标·待真实机器测量）」，权威落盘点==manual-acceptance；typo 已修；无任何「已自证发布」措辞。

## 未能自动验证的桌面项（必须进入 M08 桌面手工验收；同 r01，延续各里程碑）

沿用 `task/03-发布手工验收清单.md`：托盘真实渲染/双击/Explorer 重启恢复（C）、全局热键真实注册/冲突/快速重复（D）、UNC/移动盘离线（E）、IME 候选位置与混输（G）、混合 DPI/多显示器/横竖屏/RDP（H）、高对比/文本缩放/减少动画/屏幕阅读器（I）、启动项登录重启（J）、导入/备份/损坏恢复真实流程（K）、性能/内存/空闲 CPU 真实测量（L，含 M07.3 的 50ms/100ms/1s/50MB 目标——由 `manual-acceptance.md` 记录权威数字）、隐私观察（M）。其中 M07.3 的 50ms/100ms/1s/50MB 目标在真实桌面实测之前**不得**作为已达标事实（known-issues 已如 M-2 重写）。

## 结论

**Verdict: `APPROVED_FOR_MILESTONE`**（M07 RC-readiness hardening；非发布批准）

- r01 的 M-1/M-2/M-3（Medium）、M-4/F-5（Low）、I-1..I-5（Info）本次复审的处置：
  - M-1 → `DISPOSED`（出处自含达成，唯 M06-I1/I2/I3 来源轮次笔误，见新 N-1 Low）；
  - M-2/M-3 → `CLOSED_R02`（known-issues 数字重写、response 落盘 + run ID 及计数经 CI 日志独立复核一致）；
  - M-4/F-5 → `CLOSED_R02`（架构论证入代码注释；F-5 预检代码 + 测试 + CI 394+4 全绿）；
  - I-1..I-5 → `RECORDED`（含 typo 已修正）。
- 新增 **N-1（Low）**：response M-1 锚定表 M06-I1/I2/I3 来源文档误写 r01（应为 r02）。**不阻塞里程碑**；execution 在上方建议中已明确，需在进入 release review 前修正（纯文档改动），最好在任一后续文档提交中一并处理。
- **无 Critical/High/Medium 新项**；多项 adversarial 复核（F-5 预检与权威检查一致性、数据安全核心零改动、路径/query 不落日志、Cargo/.github 零改动）全部 PASS。
- **M07 是否已 RC-ready**:
  - 是。M07 可在完成 N-1 文档修正（或作为 release-review 前置条件）后被标记 **milestone-approved 且 RC-ready**：即 M08 可以进入候选冻结（candidate freeze）+ 用户手工验收 + 独立 release review。
  - **最终发布仍需要**：真实桌面手工验收并记录于 `review/0-0-1/manual-acceptance.md`（含 M07.3 权威性能/内存数字）、独立 release code-review 的 `APPROVED_FOR_RELEASE`、tag 指向已批准候选 commit、用户授权等（CLAUDE.md §6）。
  - **N-1 的处置时限**: 作为 r01 唯一遗留文档笔误，建议在 M08 任何文档提交中顺带修正并 `git grep` 复核；若 M08 冻结/发布评审启动时仍未修正，本 reviewer 将其视为 M-1 议题未完全闭合并保留阻断该环节的权力。此处置不影响 M07 里程碑批准。
