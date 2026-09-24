# FileGo M07 稳定性、异常、性能、无障碍与安全加固 — implementation response r01

## 元数据

- **版本**: 0.0.1（目标）
- **主题**: `m07-hardening`
- **轮次**: r01 response（对 `m07-hardening-review-r01.md`，verdict: `APPROVED_FOR_MILESTONE`）
- **日期**: 2026-09-24
- **实现 agent**: implementation agent（本 response）
- **对应 review**: `review/0-0-1/m07-hardening-review-r01.md`
- **修复前 commit（review 对象 head）**: `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1`（`feat: harden stability, resources, security and RC readiness`）；base `935e03c`
- **修复后 commit（代码修复提交）**: `e3e86a5`（`fix: refuse oversized imports before buffering and record exit-save rationale`，即 2 个代码文件：`src/storage/import_export.rs`、`src/main.rs`）
- **比较范围（代码）**: `367bb8c..e3e86a5`（`src/storage/import_export.rs` +75 行、`src/main.rs` +7 行）
- **本 response 文档 commit**: 单独随同提交，记录上述修复 SHA 与 CI 证据（沿用本仓 m04/m05 的「fix 提交与 response 提交分离」convention；本文件即该记录）
- **文件范围**: `src/storage/import_export.rs`、`src/main.rs`、`review/0-0-1/m07-known-issues.md`、`review/0-0-1/m07-hardening-response-r01.md`
- **Rust 工具链**: 本地 GNU `1.92.0-x86_64-pc-windows-gnu`（仅快速反馈，非 MSVC 权威）+ 远程 MSVC CI `x86_64-pc-windows-msvc`

> 本 response 由 implementation agent 撰写；独立 code-review agent 未参与本轮实现的
> 改动、测试编写与提交，其 r02 复审将独立查验代码/diff/测试与 CI 证据（见「请求复审」节）。

## 一句话总结

`m07-hardening-review-r01.md` 的全部 findings（M-1/M-2 Medium、M-3 Medium 程序性、M-4 Low、
F-5 Low、I-1..I-5 Info）均已处理。**所有数据安全核心未改动**（repair 持锁、panic hook、
import 限制语义、canary、rollback 原样保留）；F-5 仅在读入前**新增**一个 metadata 长度预检
（不改变 post-read 权威上限）；M-2 的 known-issues 数字已重写为「本地快速反馈/目标待真实机器」；
M-3 由本 response 文档落盘闭环。

## CI 证据（GNU 本地快速验证 + 远程 MSVC CI）

| 项 | 值 |
|---|---|
| GNU fmt | `cargo fmt --all` + `cargo fmt --all -- --check` PASS |
| GNU clippy | `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` PASS |
| GNU test | `394 passed; 0 failed; 1 ignored`（lib）+ `4 passed`（bin）——即 **393+4 + 新增 F-5 1 个测试**，共 394 个 lib 用例 |
| GNU release | `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` PASS |
| MSVC CI run（review 对象 head `367bb8c`） | **Windows CI `35950631364`**（fmt、clippy `-D warnings`、test、release build、EXE/版本、deny、about、portable artifact）→ **success** |
| MSVC benchmark run（head `367bb8c`） | **Search benchmark `35950631427`**（release 10k bench）→ **success** |
| 远程测试计数（run `35950631364` 日志直接提取） | `393 passed; 0 failed; 1 ignored`（lib）+ `4 passed`（bin） |
| head `367bb8c` 的 BENCH 行（run `35950631427`） | empty-query median=0.44ms；filtered-with-clone 35.49ms；pinyin 38.90ms；english-initials 33.02ms；edit-distance 37.34ms；multi-token 29.69ms（全部远低于宽松 500ms 门禁） |
| 修复 head CI（fix head `e3e86a5`/docs head `596472f`） | **Windows CI `35958301326`** + **Search benchmark `35958301284`**（head `596472f`，推送后由本 response 记录的 run ID 确认；`.github/**` 零改动，见 I-5/备注） |
| 备注 | 本机 GNU 会话沿用既往 workaround：`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` + `D:\mingw64\bin` PATH（`shlwapi`/windres drift），与既往会话一致，本轮无新增 |

## Findings 逐条回应

### M-1（Medium）— CLOSED 表缺少旁证出处 / 引用悬空印象 — **ACCEPTED**

**评估结论**: 接受。reviewer 已独立核实每个被引用的 ID 在既有 review 中真实存在、无误标、
无悬空；问题是文档链性：清单自身需在 response 中自含出处。下面把 CLOSED 表逐项锚定到
**上游 review 文档**（+ 修复代码/测试证据 + 本轮 CI run），使出处自含：

| CLOSED 项 | 上游来源 review 文档 | 代码/测试证据 | CI |
|---|---|---|---|
| M04-N001（托盘热键不可用提示） | `review/0-0-1/m04-windows-review-r02.md` Finding N001 | `tray_hotkey_hint`（main.rs:1861-1885）+ `sync_hotkey_from_native`（main.rs:945-969）+ slint `hotkey-unavailable-hint`（ui/app-window.slint:2044-2054） | run `35950631364` |
| M06-I1（before-import 快照同秒 stamp 碰撞） | `review/0-0-1/m06-settings-review-r01.md` Finding I1 | `backup::unique_stamp` `-2/-3` 后缀；`unique_stamp_disambiguates_same_second_backups` | run `35950631364` |
| M06-I2（保存失败冗余快照 + 工作副本回滚） | `review/0-0-1/m06-settings-review-r01.md` Finding I2 | `remove_failed_import_snapshot`（白名单文件名受限，无 traversal）；`failed_import_snapshot_cleanup_removes_only_the_named_backup`；`apply_import` Err 分支回滚工作副本 | run `35950631364` |
| M06-I3（冲突时 Apply 禁用） | `review/0-0-1/m06-settings-review-r01.md` Finding I3 | slint `enabled: root.s-import-conflicts == 0`（ui/app-window.slint:1743-1756）+ apply 层 all-or-nothing | run `35950631364` |
| M05-L2（undo 横幅 Cancel 语义）/ L3（空路径提示） | `review/0-0-1/m05-management-review-r01.md` Finding L2/L3（r02 记录 RECORDED） | `undo_dismiss` / `NoticeEnterPath` 双语文案；`draft.path.trim().is_empty()` 分支（main.rs:1591-1598） | run `35950631364` |
| M05-OBS-01（action 码耦合）/ OBS-02（死键删除） | `review/0-0-1/m05-management-review-r02.md` Finding OBS-01/OBS-02 | `RowAction::from_context_action` + 单测镜像 MenuRow；`action-toggle-enabled` 全树零残留 | run `35950631364` |
| M01B-N001-followup（repair_from_backup 加锁） | `review/0-0-1/m01b-persistence-review-r02.md` Finding M01B-R02-N001（评审明确延期 + 后续任务） | `acquire_write_lock` 在 `repair_from_backup` 全程持有（repository.rs:482）；`repair_from_backup_contends_with_a_concurrent_writer_via_the_lock` | run `35950631364`（发布前必须关闭项，现已关闭） |
| M07.1/2/4/5 各项 | 本 review r01 的 M07.1–M07.6 映射表 | 逐项测试见 review 文档 | run `35950631364` / `35950631427` |

**结论**: 每个 CLOSED 声明的「来源 review + 代码 + 测试 + CI run」四要素现已在 Git 追踪的
response 中自含，无悬空引用。

### M-2（Medium）— known-issues 性能/内存数字无实测出处 — **ACCEPTED**

**评估结论**: 接受。`77µs/772ms`、`release ~80ms`、内存 `<50MB` 等数字仅存在于本地
task/（`.git/info/exclude`）与本地 GNU debug 快速反馈，不是 Git 可复算的 MSVC 断言；若被
误读为「已验证」会污染发布证据。

**修复**: `review/0-0-1/m07-known-issues.md` 表 2（known-issues）三项重写为
**「本地 debug 快速反馈 / （目标·待真实机器测量）」**：
- `M02-F003` 行：明确「性能数字为本地 debug 快速反馈（~77µs / ~772ms / release ~80ms），
  **非 MSVC 权威、非 Git 可复算产品断言**；为（目标/待真实机器测量），权威数字由 M07.3/桌面
  性能验收实测后补入 `review/0-0-1/manual-acceptance.md`」。
- `M02 review F005` 行：明确「hosted MSVC 数字非产品断言，**尚未实测量化，列为（目标/待真实
  机器测量）**」。
- `空闲 CPU/内存` 行：明确「**静态代码结构结论**（3 个 50ms drain timer 仅 `try_recv`，
  空 channel 立即返回），**不得声称「实测 CPU 0%」**；内存 `<50MB` 为**优化目标**，权威记录
  计划补入 `manual-acceptance.md`，此前不作为已达标事实」。

**验证**: 仅文档措辞变更；`cargo fmt/clippy/test/build` 不在受影响面（无代码改动），
远程 CI 由本 response 记录的 fix 头 run 复核。

### M-3（Medium）— 缺少 Git 追踪的 response 文档（run ID + 逐 ID 回应 + 393+4）— **RESOLVED（本 response 落盘）**

**评估结论**: 接受（程序性）。r01 时 `m07-known-issues.md` 尾部称「run ID 见本轮
response」，但该 response 不存在 → 引用悬空。本 response 即该文档，现已 Git 追踪。

**修复**:
- 创建 **`review/0-0-1/m07-hardening-response-r01.md`**（本文件），逐 ID 回应 review 全部
  findings（M-1/M-2/M-3/M-4/F-5/Info）与 deferred-cleanups 复核结论。
- 权威 **CI run ID**：
  - **Windows CI `35950631364`**（head `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1`，success）
  - **Search benchmark `35950631427`**（head `367bb8c1b3d15c4c0ee92f8ccf73d11345c365d1`，success）
- **测试计数**：`393 passed; 0 failed; 1 ignored`（lib）+ `4 passed`（bin）。
  - 其中 **ignored 的 1 项** = `search::benchmark::tests::ten_k_release_benchmark_stays_under_lenient_bound`
    （`#[ignore]` 门禁测试，仅由 `benchmark.yml` release 模式显式运行，见 run `35950631427`）。
  - 本 fix 新增 F-5 测试后为 **`394 passed`**（lib）+ `4 passed`（bin）。
- `review/0-0-1/m07-known-issues.md` 尾部说明行已改为指向本 response 记录的 run ID。

### M-4（Low）— 「退出等待保存」/「保存不阻塞输入」无超时原语、无后台保存 — **ACCEPTED（架构论证记录）**

**评估结论**: 接受观察。0.0.1 架构为**即时保存 + 同步单线程写**，未实现异步保存/超时原语；
「退出前无未保存数据」由架构满足，且 M07 无需新增代码。

**架构论证（已记录于代码注释）**:
- 每次 settings 变更（toggle/persist）都即时 `save_at`（`src/presentation/settings_controller.rs`），
  `save_at`/`save` 均为**编码先行 → 加锁 → 原子替换**，失败时工作副本回滚（`src/storage/repository.rs:900-918`）。
- 退出路径 `LifecycleController::ExitFromTray → window.quit() → quit_event_loop()`
  （`src/app.rs:91-94`、`src/main.rs:52`）走与其它 mutation 完全相同的单线程串行命令路径；
  **关闭时不存在待写内存态**，因此「退出等待保存」自然为空操作——没有可被中断的进行中保存，
  无挂起可达性（UI 路径无阻塞调用；文件对话框为 OS 模态，由系统管理）。
- 保存同步在 UI 线程，为 0.0.1 可接受限制，**无专门性能测试/基准证明 10k 下不卡顿**——
  如实记录为已知限制，真实桌面复核项留在 task/03 **L10**（退出期间有未保存改动正确保存或
  报告失败、不永久卡住）与 L8（输入不冻结）。
- **明确定位**: 未引入 timed primitive（超时/等待原语）。在「无待保存队列 + 同步小写」
  的规模下不需要；若未来引入后台保存/批量异步入队，再补超时与等待语义。

**修复**: `src/main.rs` `tray.on_quit_requested` 新增注释块记录上述论证（含 M-4 索引），
无功能代码改动。`LifecycleController` 既有状态机测试（`exiting_is_terminal_for_all_commands`、
`exit_failure_retains_retryable_state`）保持。

### F-5（Low）— 导入读取边界：`fs::read` 在字节限制检查前整文件缓冲 — **ACCEPTED（已修复）**

**评估结论**: 接受。`MAX_IMPORT_BYTES=8MiB` 约束了解码/分配阶段，但「读入整个文件」本身可短暂
缓冲任意大小文件（如 1GB JSON 先分配 ~1GB 再被拒）。属防御纵深缺口，产品约束的不可绕过性未被破坏。

**修复**（`src/storage/import_export.rs` + `src/main.rs`）:
- `import_export.rs` 新增 **`import_file_len_allowed(len: u64) -> bool`**：`len <= MAX_IMPORT_BYTES`
  即放行。`main.rs` `import_data` 在 `std::fs::read` **之前**用 `std::fs::metadata` 预检；超限
  直接 `ImportTooLarge` 通知 + `DismissDataFlow`（与 parse 超限分支一致），**不读文件**。
- **保持原有 post-read 检查为权威上限**: `parse_import(bytes)` 仍对实际读到的字节数做
  `bytes.len() > MAX_IMPORT_BYTES` 检查（文件可在 stat 与 read 之间变化）；F-5 预检只
  **新增**拒绝面，未改动任何既有限制语义、未触碰已评审导入路线。
- 文档注释同步更新（模块 doc 与 `import_data` 代码注释均标注 F-5）。

**测试**（`src/storage/import_export.rs`）:
- 新增 **`import_file_len_pre_check_refuses_oversized_files_before_buffering`**：
  - `len > MAX_IMPORT_BYTES` → 预检拒绝（不读文件即可判定）；
  - `len == MAX_IMPORT_BYTES` / `0` → 预检放行（权威检查交给 `parse_import`）；
  - `MAX_IMPORT_BYTES + 1` 字节内容实际被 `parse_import` 拒绝为 `ImportTooLarge`，
    与预检预测一致。

**回归**: 既有 import 测试 8 项（含 `import_file_over_the_byte_limit_is_refused_without_mutation`）
保持全绿；无 mutation、无路径/query 泄漏、无新依赖、无 `#[allow]`。

### I-1（Info）— `M02-F003` 性能数字无法从 Git 独立复算 — **RECORDED（并入 M-2）**

**评估结论**: 记录。`77µs/772ms`、`~80ms` 为本地 GNU debug 快速反馈，非 MSVC 权威。
M-2 修复已令 `m07-known-issues.md` 明确标注「本地 debug 快速反馈 /（目标·待真实机器测量）」，
并把权威数字的落盘点指定为 `review/0-0-1/manual-acceptance.md`。CI 基准仍引用
`ten_k_release_benchmark_stays_under_lenient_bound`（run `35950631427`）。

### I-2（Info）— 线程模型/空闲 CPU 行措辞 — **RECORDED（并入 M-2）**

**评估结论**: 记录。已按 M-2 修复：`m07-known-issues.md` 空闲 CPU/内存行明确为**静态代码结构
结论**，不得声称「实测 CPU 0%」；无 3 个 timer busy-loop = `try_recv` 及时返回（main.rs:3114-3138、
3251-3261、4330-4340）。

### I-3（Info）— 「红action」拼音 typo — **RECORDED（已修正）**

**评估结论**: 记录并修正。`m07-known-issues.md` M07.1 行的「panic hook 红action」改为
「panic hook 脱敏（redaction）」。`diagnostics.rs` 源码用词本已正确，无功能影响。

### I-4（Info）— `concurrent_burst_saves_never_torn_main` 用普通 `save` — **RECORDED（测试契约说明）**

**评估结论**: 记录。该测试（`src/storage/repository_tests.rs:1509-1559`）用普通 `save`
直接覆盖，断言只保证「main 永不撕裂」而非「最后一次写是哪一个 writer」——这是**单实例产品
语义下的正确选择**：生产写路径走 `save_at`/`save_if_current` 带 revision 守卫；`save` 的
直接覆盖正是为了压力测试「锁串行化 + 原子替换」下永不分叉，而非模拟带守卫的竞争修订语义。
`repository.rs:48-59` 模块文档已明确「0.0.1 单进程」（文档见本 response M-4 引用的
Concurrency contract 段）。

### I-5（Info）— `benchmark.yml` 未跑 deny/about — **RECORDED（既有设计，不改）**

**评估结论**: 记录。`benchmark.yml` 仅有 build+bench，不跑 `cargo-deny`/`cargo about`，
属 M02 既有设计（`ci.yml` 全量门禁已覆盖 deny/about；benchmark 只做性能回归补充）。
`workflows_pin_actions_and_use_minimal_permissions` + `release_workflow_never_runs_untrusted_code_with_write`
守卫对 3 个 workflow 一视同仁（全 full-SHA pin + `permissions: contents: read` +
dispatch-only + 无 release job），`workflows_pin_actions…` 结构守卫已断言覆盖。**本轮 fix
零 `.github/**` 改动**，无需 workflow 重验。

## 对「Deferred cleanups」CLOSED 声明之交待

review r01 已在「对『Deferred cleanups』每项 CLOSED 声明之独立复核」逐一核读代码 + 测试 +
上游 review 来源并裁定成立（8 项全部成立，含发布前必须关闭的 M01B-N001-followup）。本
implementation agent 对其中每一 CLOSED 的代码与测试未做任何改动（**数据安全核心零改动**，见
上方 M-1 锚定表）。唯一需要澄清的数字出处问题已并入 M-2/I-1 处理。

## 未解决事项与待人工验证项

- `M07.3` 性能/内存目标（≤50ms / ≤100ms / ≤1s / ≤50MB）为**待真实机器测量**，M-2 已重写为
  目标措辞；权威数字必须进入 `review/0-0-1/manual-acceptance.md`（M08 桌面手工验收 + 独立
  release review），此前不作为已达标事实。
- 桌面手工项延续 M08：C/D/E/G/H/I/J/K/L/M（托盘真实渲染、热键真实注册/冲突、UNC、IME、
  混合 DPI、a11y、启动项、导入/备份/损坏恢复真实流程、性能测量、隐私观察）。
- F-5 的「文件在 stat 与 read 之间变化」竞态：post-read `parse_import` 权威检查兜底，无需
  额外原语；不视为未解决缺陷。

## 请求复审

本 implementation agent 请求独立 code-review agent 对 **fix commit** 进行 r02 复审：
- 复核代码/diff（重点：F-5 预检的**新增**性质——post-read 权威上限语义未变、`import_file_len_allowed`
  纯函数正确性、反映射保持一致；M-4 主窗退出注释；数据安全核心零改动）；
- 复核新增测试 `import_file_len_pre_check_refuses_oversized_files_before_buffering` 与
  GNU 本地证据（394+4）+ 远程 MSVC CI run（head = fix commit）；
- 复核 M-1 锚定表 / M-2 known-issues 重写 / M-3 本 response 的 run ID 与 393+4 计数；
- 确认 PASS 项无回归（repair 持锁、panic hook、import 限制语义、canary、rollback）。
