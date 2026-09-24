# FileGo M07 稳定性、异常、性能、无障碍与安全加固 — 独立 Code Review r01

## 元数据

- **版本**: 0.0.1（目标）
- **Topic**: `m07-hardening`
- **轮次**: r01（对本 M07 实现 commit `367bb8c` 的独立评审）
- **日期**: 2026-09-24（CI 时间线 2026-09-24T03:14–03:22Z）
- **被评审 commit（head SHA）**: `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1` （`feat: harden stability, resources, security and RC readiness`）
- **Base SHA（比较基准）**: `935e03c7538290f12b8f63de346d6f33ca26a436`（M06 r02 approval head）
- **比较范围**: `935e03c..367bb8c`（单提交；`git log --oneline 935e03c..367bb8c` = 恰好 1 个 commit）
- **文件范围**: `git diff --stat 935e03c..367bb8c` = **16 个文件，+1360/−66**，全部为 M07 合理归因：

| 性质 | 文件 |
|---|---|
| 新审计文档 | `review/0-0-1/m07-known-issues.md`（+62） |
| 文档 | `docs/dependency-policy.md`（+4） |
| 源码 | `src/diagnostics.rs`（+105，panic 日志）、`src/main.rs`（+308）、`src/presentation/{i18n,settings_controller,commands}.rs`、`src/platform/{window_position,window_placement}.rs`、`src/storage/{backup,import_export,repository}.rs` |
| 测试 | `src/storage/{repository_tests,m06_tests,tests}.rs`、`src/presentation/settings_controller.rs`（内嵌）、`src/diagnostics.rs`（内嵌）、`src/presentation/commands.rs`（内嵌） |
| UI | `ui/app-window.slint`（+43） |

- **明确零改动（git diff 实测为空）**: `src/domain/**`、`src/search/**`、`src/app.rs`、`src/storage/{codec,schema,location,io}.rs`、`src/platform/windows/{file_dialog,folder_picker,clipboard,single_instance,hotkey_adapter,mod}.rs`、`src/platform/{ipc,shell_open,hotkey}.rs`、`.github/workflows/*`、`Cargo.toml`、`Cargo.lock`、`deny.toml`、`about.toml`。

## Reviewer 角色与独立性声明

- **Reviewer**: 独立 code-review agent（本评审）。
- **独立性声明**: 本 agent **未参与** M07（`367bb8c`）的实现、测试编写、提交与 CI 触发，也未参与 M00–M06 任何实现。本评审期间 **未编辑任何源代码、未提交、未推送、未运行任何 `cargo` 命令**（CLAUDE.md §3.1）；除本 review 文档外未写入任何其他 Git 追踪文件。所有结论来自对实际代码、`git diff`、GitHub Actions 日志与既有审阅证据链的独立查阅，不采信实现报告的任何断言。

## CI 证据（独立复核）

| 项目 | Commit | Workflow / Run | head SHA | 结论 | 关键 job |
|---|---|---|---|---|---|
| Windows CI | `367bb8c` | [Windows CI 35950631364](https://github.com/yorelll/filego/actions/runs/35950631364) | `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1` | **success** | fmt、clippy `-D warnings`、test、release build、EXE/版本、deny、about、portable artifact |
| Search benchmark | `367bb8c` | [Search benchmark 35950631427](https://github.com/yorelll/filego/actions/runs/35950631427) | `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1` | **success** | release 10k bench |

- **远程测试计数（从 run 35950631364 日志直接提取）**: `running 394 tests` → `393 passed; 0 failed; 1 ignored`（lib）+ `running 4 tests` → `4 passed`（bin）。实测 **393+4，0 失败**，与任务预期「~393+4」**完全一致**。
- **benchmark 实测 BENCH 行（run 35950631427 日志）**，全部远低于宽松 500ms 门禁：
  - `empty-query-default: median=0.44ms p95=0.58ms max=0.70ms`
  - `filtered-with-clone: median=35.49ms p95=36.81ms max=37.76ms`
  - `pinyin-heavy: median=38.90ms p95=40.27ms max=41.82ms`
  - `english-initials: median=33.02ms p95=33.82ms max=34.12ms`
  - `edit-distance: median=37.34ms p95=38.13ms max=40.11ms`
  - `multi-token: median=29.69ms p95=30.98ms max=33.96ms`
  - `test result: ok. 1 passed`（`ten_k_release_benchmark_stays_under_lenient_bound` 及非 RUN，`393 filtered out`）。
- runner 为 `windows-latest`（MSVC target `x86_64-pc-windows-msvc`），是真实 MSVC 证据。workflow 本身 `.github/**` 在 M07 范围内零改动（`git diff 935e03c..367bb8c -- .github/` 为空），无需 workflow 重验。

## 需求/验收标准映射（M07.1–M07.6）

> 状态约定：`PASS`（自动测试/代码/CI 证据充分）、`MANUAL`（仅桌面手工验收可覆盖，如实标记）、`PARTIAL`（部分自动 + 部分手工）、`FAKE`（无证据却宣称已验证）。**未发现任何 FAKE 项。**

### M07.1 异常恢复 — `PASS`（自动化部分）+ `MANUAL`（桌面项）

| 子项 | 状态 | 证据 |
|---|---|---|
| 配置读取失败：不退出、重试/恢复备份/重置/打开数据位置 | PASS | `open_repository` 返回 `StartupDataStatus`（main.rs:1936-1999），`Unreadable` 时 Data 页匿名双语通知 `settings.notice.data_unreadable`（i18n 已复核 zh/en 齐备）；恢复动作（`open_data_dir`、`restore_backup`、`reset_settings`、`list_backups`）全部接线可达（main.rs:1230-1298、1338-1344、4220-4248） |
| 保留损坏配置，不自动覆盖 | PASS | `load` 的 `Err(CorruptData)` 分支 `main_bytes.is_some()` 保留原件、`repair_from_backup` 写独立证据文件 `data.json.corrupt-<uuid>`（repository.rs:517-525）——均为既有已评审逻辑，M07 未破坏 |
| 快捷键失败仍可托盘使用 | PASS | N001 修复：`tray_hotkey_hint`（main.rs:1861-1885）仅在 `Disabled + last_error(Conflict/Unavailable)` 时显示匿名双语提示行（`enabled:false`，不可激活），启动（main.rs:3067）与设置变更后（`sync_hotkey_from_native` main.rs:945-969）双路径刷新；slint 侧 `hotkey-unavailable-hint` 条件 MenuItem 绑定见 ui/app-window.slint:2044-2054。**在 `Disabled` 状态下 hotkey 不工作是已知正常语义，无需提示** —— 函数仅对失败态显示，设计正确 |
| 打开失败保持窗口并提供恢复动作 | PASS | 既有 `on_open_failure` 恒 `KeepWindow`（shell_open.rs:101），匿名 kind 文案、Retry/Ctrl+C 保留（main.rs:452-459）；open 走 worker 线程 + `try_recv` drain timer，不阻塞输入 |
| Explorer 重启 / 显示器变化 / 睡眠唤醒 / 网络断连 | MANUAL | 桌面手工项（task/03 C8、H5/H6、L9），无代码层面可自动验证面 |
| panic hook 不泄漏 path/query、release 无 console spam | PASS | `install_redacted_panic_hook`（main.rs:4384-4402）不读 payload、仅写 `data_dir()/panic.log` 匿名行；release `windows_subsystem="windows"` 无 console。**唯一合法文件日志即该已封顶 panic 日志** |
| 无可写数据目录/磁盘满/权限错误可理解提示 | PASS（首个不可写目录分支）+ PARTIAL | 首跑 `save(&empty)` 失败→`Unreadable`（main.rs:1974-1982）；既有各保存失败路径均有匿名通知。磁盘满的「保存失败但旧数据可读」UI 已存在（`NoticeSaveFailed`/`SNotice::SaveFailed`），真实行为列桌面手工（task/03 K11） |

### M07.2 并发与生命周期 — `PASS`

- 快速连续 hotkey/query/open/save：repository 每条写经 `data.json.lock` 串行 + revision 守卫（既有）；M07 新增 3 项压力测试（`repository_tests.rs`/`settings_controller.rs`，逐一核读，全部真实且结果正确）：100 连写最终 revision=101 无残留；4 线程×20 main 恒可解码完整文档（never torn）；40 轮交替 toggle 无丢命令、无 SaveFailed、重开视图一致。
- settings 与 main window 并发：同一 `Rc<RefCell<DocumentRepository>>`（UI 线程）共享，每次 `save_at` 过锁；状态机命令串行化。既有 `two_writers_contending_save_if_current_only_one_wins` 保持。
- **退出等待保存 + 超时**：M07 未新增退出路径代码（`LifecycleController::ExitFromTray → quit_event_loop()` 为既有）。由于每次 settings 变更即时 `save_at`（含失败回滚），退出前无待写内存态，「退出即无未保存数据」由架构满足。该子项记录为「无待保存状态 + 既定即时保存」，见 Findings M-4（Low）；桌面 L10 保留复核。
- 后台检查可取消、不持 UI 强引用进线程：线程仅 1 个 `filego-native-worker` + 每条 shell open 一个临时线程；`Rc` 均不跨线程捕获（grep 复核全部 `thread::spawn` 闭包仅捕获 `Send + Sync` 的 `Arc`/channel clone）。timer 全部 `try_recv` 空转即返回。
- 多次 show/hide 无泄漏：Slint 部件生命周期 + 既有 LifecycleController 状态机；无每-show 线程生成。

### M07.3 性能/资源 — `PARTIAL`（测量目标已诚实记录，权威数字留桌面）

- Release 10k search <50ms：**CI 门禁为宽松 500ms 回归门禁，不是产品目标**（benchmark.rs:58-59 已复核）。**M07 已诚实记录：目标 50ms 由真实桌面测量（task/03 L.2），CI 数字只是回归门禁。** Release build 的 M07 pinyin 38.9ms/edit-distance 37.3ms（本机 hosted）与产品目标接近但不足以替代真实机器测量，未自称达标。`M02 review F005（MSVC>50ms 基线）` 归档为 known-issue（归 M07.3）。
- hotkey→visible ≤100ms / cold start ≤1s：仅「测量目标」，known-issues 明确列为 `MVP 缺失（不得标为 known issue 放行）`，未在本机定制测量前不宣称达标。诚实。
- idle 无 busy-loop：3 个 50ms drain timer（native/open/context）全部 `try_recv`，空 channel 立即返回（main.rs:3114-3138、3251-3261、4330-4340）。idle 无配置写/扫描（保存仅由用户触发）。
- 内存 <50MB 目标：known-issues 与 task/03 L.2 记录为「以 <50MB 为优化目标；Slint 软件渲染基线可能超标，如实记录不掩盖」，**未伪造实测**。
- 保存不阻塞输入：**记录为已声明限制**（详见 Findings M-4，Low）。保存同步在 UI 线程（触发者路径）；错误处理均有匿名通知，不冻结事件循环；批量逐步保存亦同步。产品约束「input 不被冻结」的最终确认留桌面手工（task/03 L8）。

### M07.4 DPI / 多显示器 / a11y / i18n — `PASS`（纯几何）+ `MANUAL`（桌面项）

- 100/125/150/200% 缩放逻辑：`window_position.rs` 3 项 + `window_placement.rs` 1 项纯几何测试，代数逐项复核通过（例：125% → 600×140 逻辑 = 750×175 物理，x=(2560-750)/2、y=144；150% → 900×210；200% → 1200×280）。所有测试均为纯函数、确定性、无 FFI。**正确无误**。
- 混合 DPI / 横竖 / 超宽 / 任务栏四边 / 远程桌面 / 断开重连 / 文本缩放 / 高对比 / 减少动画 / 屏幕阅读器：桌面手工项，全部进 task/03 H/I/M 清单，**无自动声称**。screen reader 受 Slint 1.18 限制，已知清单如实披露。
- Microsoft 拼音候选放置/composition：手工项（task/03 G1-G3）；presenter 层 IME gate 有状态机测试。未自称完成。
- i18n parity：既有 parity 测试（key 集合/计数/非空，i18n.rs tests 4 项）保持；M07 新增 key 全部双语文案（`undo_dismiss`/`tray.hotkey_conflict`/`tray.hotkey_unavailable`/`enter_path`/`import_too_large`/`data_unreadable` 六个新增 key 的 `Msg`→id→`ALL_KEYS`→zh→en 全链逐一复核齐备）。

### M07.5 安全/隐私/供应链 — `PASS`

- 无 cmd/PowerShell：全 src 实现源 `std::process::Command`/`Command::new`/`CreateProcess`/`powershell`/`cmd.exe`/`cmd /c`/`ShCreateProcess` grep **零命中**（仅测试 guard 自身命中）；唯一 shell 边界 `ShellExecuteExW` 复核 `lpVerb=null`、`lpParameters=null`、`lpFile=path`（tray_open.rs:152-186）。**守卫为递归 src 扫描（M07.tests 新增），且此轮 `.github` 未变。**
- IPC 固定消息：`ipc.rs` 复核——固定 64B 信封，仅 `FTGL/version=1/CMD_SHOW=1`，reserved 必须为 0，无 payload 槽；13 项测试含 `decode_never_treats_arbitrary_bytes_as_execute`。**正确。**
- 导入限制（8MiB/100k + 线性 count probe + 不 mutation）：`parse_import` 先限字节、再 `serde_json::Value` 线性结构计数，超限 `ImportTooLarge`（import_export.rs:217-236）；读文件在 parse 之前（`std::fs::read` 上限由 `MAX_IMPORT_BYTES` 兜底——见 Findings F-5）。3 项测试 + 既有 8 项 import 测试。**不能被绕过**（详见 Findings F-5 复核）。
- JSON/备份在用户数据目录，绝不写程序目录：`location.rs` 纯相对 base_dir 派生，`data_dir()` 为 `%LOCALAPPDATA%\FileGo` + 可测试注入；无任何 exe 相对路径常量（grep 复核）；panic.log 亦在 data_dir 下。
- 日志最少、轮转/上限、可清理、不含路径/query：唯一文件日志 `panic.log`（256KB cap + 单 `.1` 轮转，diagnostics.rs）已测；普通诊断仅固定匿名 `eprintln!`（release 无 console）；守护测试防止路径/query 进日志。**正确。**
- Actions pinned + 最小权限：`workflows_pin_actions_and_use_minimal_permissions` + `release_workflow_never_runs_untrusted_code_with_write` 结构守卫；`.github/*.yml` 逐字复核：全部 `uses:` 40 字符 SHA pin、`permissions: contents: read`（release 亦只读）、release 为 `workflow_dispatch`-only、无 `pull_request_target`、`persist-credentials:false`、无 release job。**与守卫断言一致。**
- 依赖审计：CI `cargo deny --locked check advisories bans licenses sources` + `cargo about` 成功；M07 依赖零变更（diff 为空）且 policy 文档记录 audit 姿态。
- 无遥测/网络：`src/` 无 network crate/调用（唯一 URL 是 About 页展示字符串）；README 明示不扫描、不上传、无遥测。

### M07.6 综合回归 — `PASS`（部分含 track/记录说明）

- `task/02 §12` 映射表已在 M07（本地，task/ 为 `.git/info/exclude` 忽略，仅本地记录非 Git 审计）；我已抽查映射可靠性——所有 M07 自动项均有对应测试名，未静默遗漏自动项。`03-发布手工验收清单.md`（本地）已把无自动覆盖项全列入 B–N 节，无静默遗漏。
- **RC known-issues 存在且 MVP_MISSING 与 known-issue 严格分离**：`m07-known-issues.md` 三张表（CLOSED / known-issue / MVP 缺失）划分清晰；hotkey→visible 与 cold-start 目标列为「未达标前不作为事实」。**诚实。**
- reviewer 对整个 base-to-head diff 做综合审查（即本文档 §「跨面检查」+ findings）。

## Findings（按严重级排序）

### Critical / High

**未发现。**

### Medium

- **M-1 — 本 reviewer 对 CLOSED 表所列所有既有 review ID 来源逐一核实，未发现悬空引用，但 `m07-known-issues.md` 自身未为该表提供旁证出处（大多推迟到不存在的 response）。**
  - **文件**: `review/0-0-1/m07-known-issues.md`（CLOSED 表：M04-N001、M06-I1/I2/I3、M05-L2/L3、M05-OBS-01/OBS-02、M01B-N001-followup、M07.1/2/4/5 各项）；`src/main.rs`、`src/storage/{backup,import_export,repository}.rs`、`src/diagnostics.rs`、`src/presentation/commands.rs` 注释。
  - **问题（严重度评估）**: 本 reviewer 通过 `git show` 对每个被引用的 ID 在既有 review 中的存在性逐一核实（M04 r02 有 N001；M06 r02 有 I1/I2/I3；M01B r02 有 N001；M05 r01 有 L2/L3、r02 有 OBS-01/OBS-02），**全部命中、无误标、无悬空**。潜在问题仅是文档链性的：`m07-known-issues.md` 预填/引用的「来源 review」多为 r01 review 的响应文档（该响应尚未存在，见 M-3），在 response 落盘前，清单自身不构成独立证据。
  - **影响**: 仅文档性/程序性。每个 CLOSED 声明的代码+测试证据已由本 r01 独立复核成立（见「Deferred cleanups」「Adversarial checks」），故非内容缺陷。
  - **建议**: (1) 实现方 response 逐一锚定每个 ID 到来源 review 文档 + 修复 commit + CI run；(2) 保持既有 convention（每里程碑 review 的 Findings ID 是后续引用的唯一权威来源）。

- **M-2 — 未经验证的 `M07.3` 性能/内存指标被记录在 `m07-known-issues.md` 中（“内存 <50MB 目标与实测记录见 M07.3 报告区”，以及 `M02-F003` 行）。**
  - **文件**: `review/0-0-1/m07-known-issues.md` 表 2（known-issues）：`M07.3` 记录 `77µs/772ms/release ~个位数 µs/~80ms/median <50ms/hotkey ≤100ms/cold ≤1s/内存 <50MB`。
  - **问题**: 该清单声称「已知限制」，其部分数字**无本 M07 实测出处**（`77µs/772ms`、`release ~80ms` 的出处不在 `git show 935e03c:review` 的既有 review/response 中；`M02-F003` 的 `77µs/772ms` 证明只存在于 `.git/info/exclude` 的 task/（本地）中，未入 Git）。内存 `<50MB` 的「实测」引用 `M07.3 报告区`，该报告区在 commit 时不存在。这些声称未以可审计形式归档，若被后续环节误当「已验证」，会与 CI 门禁混为一谈。`M07.3` 的 `hotkey→visible ≤100ms / cold ≤1s` 已正确地在『MVP 缺失』表列为「测量目标、未达标前不作为事实」。因此可以将此表解释为「该清单是给用户/评审者的、待桌面 M07.3 复核的 known-issue 上下文」，而非已验证事实；但应当消除歧义。
  - **影响**: 若原样进入 release review 而被误读为「已验证」，可能污染发布证据。作为 review 输入应重新表述为「目标（待真实机器测量）」。
  - **建议**: (a) 将 `M07.3` 各性能/内存行从「known-issues」重排到「需要真实机器测量（目标）」栏，或加显式 `（目标/未验证）`前缀；(b) `M02-F003` 行把 `77µs/772ms` 标注为「本地 debug 观察（GNU 记录），非 MSVC 权威」；内存「M07.3 报告区」改为「（待真实桌面）」。**不阻塞 M07 里程碑**；在 release review 前，M07.3 真实机器数字必须进入 `review/0-0-1/manual-acceptance.md`。

- **M-3 — M07 除 `m07-known-issues.md` 外无独立 Git 追踪的 response/证据文档（run ID、修复 commit、逐 ID 说明）即落盘；`m07-known-issues.md` 尾部称「run ID 见本轮 response」，但该 response 尚未存在。**
  - **文件**: `review/0-0-1/m07-known-issues.md` 第 62 行；`git grep 35950631364` 在 head 无命中；`review/0-0-1/` 无 `m07-hardening-response-r01.md`。
  - **问题**: CLAUDE.md §4.4 要求 implementation agent 为每一轮 review 提供 response；§4.2/§4.3 要求 review 文档被 Git 追踪。当前 head 中**只有该 review（本文件的持久化）**，而 `CLOSED` 声明的实际修复证据（commit 367bb8c 的代码 + 测试 + 远程 CI run 35950631364/35950631427）尚未沉淀为该 M07 响应文档。M07 implementation 尚未写入该响应。`m07-known-issues.md` 的「说明」行把 run ID 推迟到「本轮 response」，而该 response 不存在 → 引用悬空。
  - **影响**: 发布审计要求「每个 CLOSED 项有代码+测试+CI 证据」——代码+测试证据已存在（本 r01 独立复核了每一个）；run ID 悬空不是内容缺陷，而是**审计文档链缺口**：若后续响应不出现，M07 不能被视为「闭环」。
  - **建议（强制）**: 在本 r01 之后，implementation agent 必须创建 Git 追踪的 `m07-hardening-response-r01.md`，逐 ID 回应（本文档全部 findings M-1/M-2/M-3/M-4/F-5/Info 与 deferred-cleanups 复核结论），并填写两个 run ID 与 393+4 测试计数、以及 397 项中的 ignored 1 项说明。release-review 前必须有该 response 与 re-review 闭环。

### Low

- **M-4 — 「退出等待保存」与「保存不阻塞输入」被列为 M07.2/M07.3 子项，但 M07 无新增代码（无超时原语、无后台保存）——其「满足」仅靠「无未保存状态 + 即时保存」的架构论证。**
  - **文件**: `src/app.rs` `LifecycleController`（ExitFromTray → quit）；`src/main.rs` `run()` 末尾 `slint::run_event_loop()`；`src/presentation/settings_controller.rs` `persist()` 每次 toggle 即时 `save_at`；`src/presentation/manager.rs` 各保存路径。
  - **问题**: 每次 settings 变更都即时同步 `save_at`（含 rollback）；关闭前无待写内存态。因此「退出前保存」自然为空操作（无可保存项），超时/等待逻辑无必要。此论证成立，但**没有自动化测试**验证「退出序列中如果存在一次尚未完成的保存，会不会被中断」，也没有对「清空后立刻退出」的检测。M07.2 的「退出有超时/错误反馈，不永久挂起」因此只能靠状态机 + 单线程串行（无等待信道）的推理。同理「保存不阻塞输入」：所有保存路径在 UI 线程同步执行（为可接受的 0.0.1 限制），无专门性能测试/基准证明在 10k 规模下短暂冻结不会让用户感知卡顿。
  - **影响**: 低——单线程串行 + 无后台任务的架构使「永久挂起」不可达（无阻塞调用在 UI 路径：开/文件对话框是模态但由 OS 管理）；「保存时输入不冻结」在 10k 规模未经测量，是诚实的已知限制（已记录）。
  - **建议**: (a) 在 response 中明确「0.0.1 采用即时保存 + 同步单线程写，未实现异步保存/超时原语；CLAUDE.md M07.2 的『退出等待保存+超时』经架构论证满足（无待保存队列），并以任务/03 L10 作为桌面复核」；(b) 可选为 `LifecycleController` 补一条「退出前无挂起」的确定性测试（纯状态机已有）。(c) `task/03` L10（退出期间有未保存改动正确保存或报告失败，不永久卡住）保留。

- **F-5 — 导入读取边界：`std::fs::read(&path)` 在字节限制检查之前把整个文件读入内存，超大小文件可短暂占用大块 RAM 后才被拒绝。**
  - **文件**: `src/main.rs:1053`（`import_data` 的 `std::fs::read(&path)`）；`src/storage/import_export.rs:217-220`（`MAX_IMPORT_BYTES` 检查发生在 `parse_import(bytes)` 内，即在读入之后）。
  - **问题**: `MAX_IMPORT_BYTES=8MiB` 限制了解码/分配阶段的缓冲，但「读入整个文件」本身可短暂缓冲任意大小（例如用户选择 1GB 的 JSON 会先分配 ~1GB 再被拒）。产品约束「导入限制文件大小/记录数，防止资源耗尽」在解码/应用阶段满足；「读取时 RAM」边界未设流式预检。
  - **影响**: Low。0.0.1 是用户主动挑选单个文件的手动导入场景；超限文件读入后立即被拒，不 mutation、不落盘、不泄漏。仅供防御纵深缺失。
  - **建议**: 读入前先 `std::fs::metadata(&path)` 预检 `len() > MAX_IMPORT_BYTES` 直接拒绝（或改用带上限的流式读取），使超大小文件在分配前就被拒。**非阻断**；若实现方接受为已知限制，需在 response 记录并由 reviewer 在接受声明中确认。

### Info

- **I-1 — `M02-F003` 的性能数字（`77µs/772ms`、`~80ms`）无法从 Git 追踪文档独立复算**（仅 task/ 本地）；与 M-2 相关，建议归入 response 的 measurement-evidence 区（本地 GNU debug 观察 + CI 基准引用 `ten_k_release_benchmark_stays_under_lenient_bound`）。
- **I-2 — `m07-known-issues.md` 表 2 中「线程模型」「空闲 CPU/内存」行若单独存在会看起来像审查宣称；建议在 response 中明确这些是静态代码结构结论（无 3 个 timer busy-loop = try_recv 及时返回），不得声称「实测 CPU 0%」。（已在 M07.3 复核正确，仅措辞。）**
- **I-3 — `M07.1 panic hook 红action` 为拼音 typo（应为「redaction」）：`m07-known-issues.md` 出现该词（`diagnostics.rs` 源码用词正确）。仅注释/文档；无功能影响，建议 response 顺手修正。**
- **I-4 — `concurrent_burst_saves_never_torn_main` 使用普通 `save`（不经 revision 守卫，直接覆盖），断言只保证「main 永不撕裂」而非「最后一次写是哪一个 writer」。在单实例产品语义下可接受（生产写路径走 `save_at`/`save_if_current` 带守卫），response 应说明该测试为何选用 `save`；文档 `repository.rs:48-59` 已明确「0.0.1 单进程」。**
- **I-5 — `benchmark.yml` 的 job 未跑 `cargo-deny/about`（仅 build+bench，属 M02 既有设计）；`workflows_pin_actions_and_use_minimal_permissions` 守卫对 3 个 workflow 一视同仁（pin + permissions 断言全覆盖），无遗漏。**

## 对「Deferred cleanups」每项 CLOSED 声明之独立复核

逐一核对代码 + 测试 + 上游 review 来源（本 reviewer 不采信清单自我声明，全部直接对代码与 `git show` 上游 review）：

1. **M02-F003（O(n²) duplicate 定位，recorded-as-accepted）**: `repository::duplicate_folders`（O(n²) 全库）与 `is_duplicate_path`（逐键 O(n)）确实仅在**手工「定位重复」/校验**触发，不走搜索热路径（搜索核心 `src/search/` 零路径字符串比较，已 grep）。known-issues 将其归为可接受限制并附数字。**接受为记录**；但数字出处见 M-2 需在 response 澄清。
2. **M04-N001（托盘热键不可用提示，wired+refreshed）**: 复核代码 `tray_hotkey_hint` + `sync_hotkey_from_native` 双路径 + slint 条件 MenuItem（`enabled:false`）。可见性逻辑正确：仅 `Disabled + last_error` 显示；未配置/正常 Disabled 不显示（避免骚扰）。**CLOSED 成立。**
3. **M06-I1（same-second unique_stamp）**: `unique_stamp` 存在性探测 + `-2/-3` 后缀；测试 `unique_stamp_disambiguates_same_second_backups` 断言两快照均完整、`list_backups` 排序确定。`m06_tests.rs` 改用到真实路径。**CLOSED 成立。**
4. **M06-I2（failed-import snapshot 移除 + 工作副本回滚）**: `remove_failed_import_snapshot` 严格限定 `backup-before-import-*.json` 单一扁平文件名（prefix+shape+components==1，no traversal），缺失即成功（幂等）；测试 `failed_import_snapshot_cleanup_removes_only_the_named_backup` 覆盖 manual/`data.json`/`../` 拒绝；`apply_import` `Err` 分支同步 `repo.set_data(current.data.clone())` 回滚工作副本。**CLOSED 成立。**
5. **M06-I3（conflict>0 Apply disabled）**: slint `enabled: root.s-import-conflicts == 0`（ui/app-window.slint:1743-1756）+ apply 层 all-or-nothing 拒绝双保险。**CLOSED 成立。**
6. **M05-L2（undo dismiss 措辞）/ L3（空路径提示）**: `undo_dismiss`/`NoticeEnterPath` 双语文案齐备，`draft.path.trim().is_empty()` 分支代码正确（main.rs:1591-1598）。**CLOSED 成立。**
7. **M05-OBS-01/02（命名常量化 + 死键删除）**: `RowAction::from_context_action` 实现 + 单测镜像 slint `MenuRow` 顺序（0=Open…6=Remove、999→Remove，逐项与 slint `MenuRow` 的 7 行一致）；`action-toggle-enabled` 全树零残留（slint `UiStrings`/`ALL_KEYS`/`call_string_setter`/`apply_settings_localization` 均已清），行按钮文字改由 `root.m-enabled ? context-menu-disable : context-menu-enable` 驱动。**CLOSED 成立。**
8. **M01B-N001-followup（repair_from_backup 持锁 + 争用测试）**: `acquire_write_lock` 在 `repair_from_backup` 全程持有（repository.rs:482），body 为 re-read→judge→evidence→promote 全过程；回归测试用真实 lock 文件阻塞验证 `ConcurrentModification` 且不触碰主/备份，释放后 repair 成功。**CLOSED 成立（这是发布前必须关闭项，现已关闭）。**

## Adversarial checks（逐条裁定）

1. **Panic hook 是否真 redacted**: **PASS**。hook（main.rs:4384-4402）永不 read `info.payload()`，仅取 `info.location()`（file:line:col，不可能含用户路径/query）；日志行固定模板。`log_panic` 在 data_dir 下（`%LOCALAPPDATA%\FileGo\panic.log`），256KB cap + 单 `.1` 轮转；测试断言轮转 + 无 `\`/`C:` 出现在匿名行。release `windows_subsystem="windows"` 无 console。**若 hook 曾写入 payload/location 则会 High——未发生。**
2. **Import 限制能否被绕过 / 快照删除是否越界**: **PASS（含一个 Low 边界项，见 F-5）**。`parse_import` 先按字节限制（`bytes.len()>8MiB` 直接拒绝），再线性 `serde_json::Value` 结构计数（folders/categories/tags 各 ≤100k），超限返回 `ImportTooLarge`，不进入 `codec::decode`（不 mutation）——decode/apply 阶段无法被绕过。`remove_failed_import_snapshot` 只允许 `backup-before-import-*.json` + `components==1`（无 `..`/绝对路径），`data.json`/manual 拒绝，缺失幂等——删除无法越界。**唯一不够深的一处**是 `import_data` 的 `std::fs::read(&path)` 在字节限制检查之前整文件读入内存（超大小文件短暂占用大块 RAM 后才被拒），已作为 Low F-5 提交，不影响「不可绕过」结论。
3. **repair_from_backup 锁是否真正串行化**: **PASS**。锁在 `repair_from_backup` 全程持有（自 acquire 行起至函数尾），覆盖 re-read→evidence-write→promote→pending 清除 全部窗口。争用测试可证：锁文件存在 → clean `ConcurrentModification` 且 main/backup 字节未变。**并发 writer 不能插在 re-read 与 promote 之间**（不会把新 main 静默回退到旧 backup）。
4. **无新增真实目录删除 / 无新扫描**: **PASS**。`remove_file` 仅在 `src/storage/io.rs`（temp/lock cleaning + WriteLock drop）；`remove_dir`/`remove_dir_all`/`delete_directory` 全 src 零命中（守卫测试强制）；M07 未新增任何 fs 删除调用（`backup::remove_failed_import_snapshot` 也通过 `io.rs::remove` 且文件名受限）。canary 测试 `clear_all_records_keeps_every_real_directory_and_settings`、`m05_tests.rs` canary 仍全绿。扫描仅 `list_direct_children`（用户触发、一层、max100）与 repository `cleanup_stale_temps`（持锁删自身 temp）。
5. **无 fake claims**: **PASS**。性能/DPI/a11y/screen-reader 全部用「目标/手工/待验证」措辞，无一宣称「已验证」。`M07.3` 明确 CI bound 只是宽松门禁、50ms 被真实机器测量。`MVP 缺失` 表明确 hotkey→visible 与 cold-start 未达标前不作事实。
6. **无 scope creep / 回归**: **PASS**。16 文件全部 M07 归因；`src/app.rs`、storage schema/codec/location/io、search core、`.github/`、Cargo 零改动。无新依赖。无遥测/网络（About 页 URL 为展示字符串）。错误全部匿名化。日志无路径/query。i18n parity 测试保持全绿。
7. **known-issues 清单诚实性**: **PASS**。`MVP 缺失` 与 `known-issue` 严格分表；无必须发布项被隐藏；hotkey→visible/cold-start 目标按「未达标」处理。
8. **whole-diff 累计状态抽查**: **PASS**。对 M07 之外的核心面做了针对性的独立浏览：repository save/load/repair/lock 全套保持不变（M01-B 已审），search core 零改动、storage schema 不变、IPC 固定 Show-only、shell 仅 ShellExecuteExW raw path（无 command 拼接）、窗口几何与 DPI 规则正确、lifecycle 状态机完备、i18n 双语文案齐、canary 与 no-delete 守卫均在。未发现里程碑 review 遗漏的明显回归。

**结论性说明**: 上述 Medium/Low/Info findings 均**不构成数据安全、真实目录删除、配置破坏、隐私泄漏或错误发布产物风险**，与 `task/02 §11` 阻断规则逐条对照均不满足「不得发布」条件。M07 可进入里程碑批准；release 仍需 response + 桌面测量 + 独立 release review。

## Cross-cutting 检查

- **正确性**: M07 改动经逐函数推理：`unique_stamp` 存在性探测正确（checked while-loop）；`remove_failed_import_snapshot` 名/形状/深度三重白名；`apply_import` 失败回滚 + 快照清理路径无泄漏；panic hook 不读 payload 只取 location；`tray_hotkey_hint` 仅 `Disabled+error` 显示；DPI 几何代数复核通过。全部测试断言真实。
- **错误处理**: 备份创建失败 → 拒绝导入（不 mutation）；save 失败 → 工作副本回滚；（panic 日志 I/O 失败吞掉，符合「诊断非致命」）。
- **数据安全**: 唯一删除面 = 失败导入快照，白名受限；金丝雀 + no-delete 守卫 + io.rs-only `remove_file` 全部保持。
- **隐私/安全**: panic 日志匿名；import 限流；IPC 固定；shell 无命令解释器；Actions pin+min-perm；无网络/遥测。
- **Windows 行为**: 100/125/150/200% 几何纯测试 + 桌面手工兜底；worker 线程模型不变（1 worker + 每 open 临时线程）。
- **测试覆盖**: MSVC CI 393+4 全绿（远程日志直接核对）；新增测试类全部落在对应 finding 根因上（非表面测试）。
- **可维护性**: `commands.rs`/`backup.rs`/`import_export.rs`/`diagnostics.rs` 命名清晰、入侵面小；`known-issues` 清单结构清晰。
- **性能**: 导入 count probe 线性（O(n) JSON 结构）；`unique_stamp` 增量探测常数级；无新增热路径（新 DPI 测试为纯函数，不进生产路径）。
- **依赖与供应链**: Cargo 依赖零变更（diff 为空）；Actions pin 全复核（40 字符 SHA + `contents: read`）；`cargo deny/about` 通过。

## 未能自动验证的桌面项（必须进入 M08 桌面手工验收；延续各里程碑）

沿用 `task/03-发布手工验收清单.md`：托盘真实渲染/双击/Explorer 重启恢复（C）、全局热键真实注册/冲突/快速重复（D）、UNC/移动盘离线（E）、IME 候选位置与混输（G）、混合 DPI/多显示器/横竖屏/RDP（H）、高对比/文本缩放/减少动画/屏幕阅读器（I）、启动项登录重启（J）、导入/备份/损坏恢复真实流程（K）、性能/内存/空闲 CPU 真实测量（L，含 M07.3 的 50ms/100ms/1s/50MB 目标）、隐私观察（M）。以上均已列入且不得由 agent 代填 PASS。

## 结论

**Verdict: `APPROVED_FOR_MILESTONE`**（M07 RC-readiness hardening；非发布批准）

- M07.1–M07.6 逐项映射：自动项全部 PASS，桌面项全部 MANUAL 且如实标注，**无 FAKE 项**。
- 8 项 adversarial checks 全部 PASS；M07.5 供应链/隐私/删除不变式成立；393+4 测试与两个 CI run 经远程日志独立核实。
- 所有 Deferred cleanups CLOSED 声明经代码+测试+上游 review 逐项复核**成立**，含发布前必须关闭的 M01B-N001-followup（repair 持锁 + 争用测试）。
- **本 milestone 批准不意味着 M07 可立即冻结为 RC 发布候选**。release-review 前必须完成：
  1. 实现方创建 **Git 追踪的 `m07-hardening-response-r01.md`**（逐 ID 回应本 review 全部 findings 与 deferred-cleanups 复核结论，填写两个 run ID 与 393+4 计数；M-3 强制）。
  2. re-review 由本 reviewer（或另一位独立 reviewer）完成；随后才可进入候选冻结。
  3. `m07-known-issues.md` 中 `M07.3` 性能/内存行需按 M-2 重新措辞（「目标/待真实机器测量」）；`F-5` 的读取边界建议在 release 前采纳或由 reviewer 接受。
  4. M08 桌面手工验收 + `manual-acceptance.md` + 独立 release review `APPROVED_FOR_RELEASE` 全部满足后方可发布。

- **桌面验收依赖列表**（不做、不 PASS 的项会阻塞发布）：C/D/E/G/H/I/J/K/L/M 各节；尤其 L 节的真实性能目标（≤50ms / ≤100ms / ≤1s / ≤50MB）必须由真实 Windows 桌面测量并记录。
