# FileGo 0.0.1 MVP 完成度审计（completion audit，agent c）

> - **版本**: 0.0.1（目标）
> - **主题**: `mvp-completion-audit`
> - **轮次**: 首个完成度审计
> - **日期**: 2026-09-24
> - **被审计 head**: `88ac84d8781907f1cc3613eb3f193fba101bc0c0`（`docs: record RC candidate remediation and artifact manifest`）
> - **担保 RC head**: `a16d28b015ac5816677f3bba961f8e89962fc119`（`fix: capture GUI-subsystem version through .NET process API`）
> - **比较范围**: `a16d28b..HEAD` = **仅 2 个 review 文档**（`review/0-0-1/manual-acceptance.md`、`review/0-0-1/rc-remediation-notes.md`），无任何代码/CI/产物变更（本审计独立核对 `git diff --name-only` 与 `--stat`）。

## 1. 角色与独立性声明

- **审计者（本 agent，agent c）**: 独立完成度审计 reviewer。
- **独立性声明**: 本 agent **未参与** M00–M07 任何实现、修复、测试编写、提交或 CI 触发，也未参与任何 review/response 文档的撰写；本审计期间**未编辑任何源代码、未提交任何代码**，只以只读方式核验 Git 历史、源代码、测试与 CI 证据。除本审计文档外未写入任何 Git 追踪文件（task/ 清单属本地未追踪文件）。
- **本地 Rust 验证**: 依 CLAUDE.md §3.1 授权，以 GNU `1.92.0-x86_64-pc-windows-gnu` 做了快速反馈测试（只读 `cargo test`，不视为 MSVC 权威；写 target/ 为 git-ignored 构建产物，不属于"实施"）。

## 2. GNU 本地测试证据

| 项 | 值 |
|---|---|
| Toolchain | `1.92.0-x86_64-pc-windows-gnu`（预检 `RUSTUP_AUTO_INSTALL=0`；rustc 1.92.0 / cargo 1.92.0） |
| 目标 | `x86_64-pc-windows-gnu` |
| 会话 workaround | `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` + `D:\mingw64\bin` PATH（既有 shlwapi workaround） |
| 命令 | `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` |
| 结果（本审计独立运行） | **lib: 394 passed; 0 failed; 1 ignored**（ignored = `ten_k_release_benchmark_stays_under_lenient_bound`，release-only）+ **bin: 4 passed; 0 failed** |
| **GNU 总计** | **398 passed, 0 failed, 1 ignored** |
| 工作树/提交 | head `88ac84d`，clean（`git status --porcelain` 空） |

**与 MSVC CI / RC 期望比对**:
- M07 修复 head 权威 MSVC CI（run `35958301326`，head `596472f`→同 fix 代码 `e3e86a5`）日志：`394 passed; 0 failed; 1 ignored`（lib）+ `4 passed`（bin）。**本审计 GNU 计数 394+4 与之逐字一致。**
- M07 r01 头（`367bb8c`）CI：`393+4`。当前 GNU 是多 1 个（F-5 新增 `import_file_len_pre_check_refuses_oversized_files_before_buffering`）后的 394+4，与修复后基线一致，无回归。
- **结论**: GNU 398 passed 与最后绿色 CI/release-candidate 期望**一致**（`a16d28b` 相对 `367bb8c` 无产品代码变更，仅 release 文档+release.yml；`88ac84d` 相对 `a16d28b` 纯 review 文档）。

## 3. 需求/验收来源

- `task/01-里程碑任务清单.md`（本地，git-ignored）
- `task/02-验收与测试矩阵.md`（本地，git-ignored；§12 自动项→测试名映射）
- `review/0-0-1/m07-known-issues.md`（Git 追踪）
- `review/0-0-1/rc-remediation-notes.md`（Git 追踪）
- 全部 `review/0-0-1/m0X-*-review/response-r0Y.md`

## 4. 完整清单：里程碑逐项 → 关闭声明 → 证据 → 裁定

> 约定：`CLOSED`＝在本 head 上已有代码+测试（或 review 闭环）证据；`MANUAL`＝仅真实桌面验收可覆盖（已进入 task/03，非缺口）；`RECORDED/DISPOSED`＝reviewer 已明确接受为已知限制/设计决策；`PARTIAL`＝主体关闭但有残留观察；`NOT-CLOSED`＝未关闭。**本审计未发现任何需要实现方修复才能继续的 NOT-CLOSED 编码缺口**（唯一的残留为文档/状态同步观察，见 §6）。

### 4.1 M00 工程基础（DONE，r03 APPROVED_FOR_MILESTONE）

| 项 | 关闭声明 | 独立证据 | 裁定 |
|---|---|---|---|
| M00 全套（Cargo/CI/ADR/生命周期） | r03 `APPROVED_FOR_MILESTONE`（review-r03） | `review/0-0-1/m00-foundation-review-r03.md` 逐项 CLOSED；代码 `src/app.rs` lifecycle + app.rs 8 测试 | **CLOSED** |
| M00 桌面验收（托盘/热键/DPI…） | 任务清单「留发布前验收」 | 进入 task/03 B–M 各节 | **MANUAL** |

### 4.2 M01 数据模型与安全持久化（DONE，M01-A/B 均 APPROVED）

| 项 | 关闭声明 | 证据（file:test） | 裁定 |
|---|---|---|---|
| M01-A 全量（schema/验证/round-trip/隐私） | r02 批准（head 3d88acf/716abd9） | `src/storage/tests.rs`（25 项）、`src/domain/settings.rs` 范围校验 | **CLOSED** |
| M01-B 安全保存/故障矩阵/损坏保留 | r02 批准（head f501442） | `src/storage/repository_tests.rs` fault matrix 33 项（含 F001–F005） | **CLOSED** |
| **M01B-N001-followup（repair 加锁）** | M07 声称 CLOSED（known-issues） | `src/storage/repository.rs:470-535` `repair_from_backup` 全程持有 `acquire_write_lock`（`repository.rs:482` `let _lock = self.acquire_write_lock()?;`）；回归测试 `src/storage/repository_tests.rs:651` `repair_from_backup_contends_with_a_concurrent_writer_via_the_lock`（锁存在→`ConcurrentModification` 且 main/backup 字节不变；释放锁→Repaired with evidence） | **CLOSED**（代码+真实双状态断言） |
| M01.2 路径语义（folded into M05） | M05 任务清单 [x] | `src/domain/path_semantics.rs` 16 项测试（`path_key`/`same_path`/`expand_open_path`；PathClass UNC/盘符/尾分隔符/`\\?\`）；`src/storage/repository.rs:876` `duplicate_folders` 用 class+normalized 复合键 | **CLOSED** |
| M01.5 导入导出核心（folded into M06） | M06 数据管理与 review | `src/storage/import_export.rs`：`parse_import`/`export_bytes`/`plan_import`（overwrite/merge/skip）/`apply_import`（all-or-nothing）；测试 17 项含 `own_export_round_trips_through_parse`、3 个 mode、冲突拒绝、资源上限 | **CLOSED** |
| M01.4 reset 先备份 | task/01 仍 `[ ]`（下文 §5） | 每条 `save_at`→`save_locked` 先 `replace_backup_with_current_main()`（`repository.rs:318`）再 rename——reset/clear-all 落地同路径；fault 测试 `save_fault_backup_staging_preserves_previous_backup`（BackupWrite/Sync/Replace 三故障点旧 `.bak` 字节不变）；`backup.rs` 显式用户备份 create/list/restore 测试在位 | **CLOSED（实现语义）+ PARTIAL（task/01 勾选状态未同步，见 §6-F1）** |

### 4.3 M02 搜索/排序/筛选（DONE，M02-A/B 均 APPROVED）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M02-A 搜索核心（含 F001 High tie-break 方向） | r02 批准（head 093941a/377186c） | `src/search/{scoring,matching,keys,mod,tests}.rs`；F001 `tiebreak` 方向已修并有 3 表驱动+1 端到端；F003 编辑距离 64 字符护栏 | **CLOSED** |
| M02-B 筛选/空查询/10k 基准 | r02 批准（head 3829525） | `src/search/filter.rs`、`empty_query.rs`、`benchmark.rs`（`ten_k_release_benchmark_stays_under_lenient_bound` #[ignore]，由 benchmark.yml release 跑） | **CLOSED** |
| **M02-F003 O(n²) duplicate**（`duplicate_folders` 全库定位） | m07-known-issues 记录为可接受 | `repository.rs:876-897` 仅「定位重复」按钮触发、不在搜索热路径；搜索核心 `src/search/` 零路径字符串比较（grep）；known-issues 已重写为「本地 debug 快速反馈/待真实机器」（M-2 CLOSED） | **RECORDED（reviewer 已接受；仅显式触发）** |
| **M02-F004 / M04 Show-vs-Toggle** | M04 r02 记录为显式设计决策 | `main.rs:3148-3153` HotkeyShow/ActivateFromSecondInstance → 恒 `LifecycleCommand::Show`；`LifecycleCommand::Toggle` 仅 tray（`main.rs:3030`）；r02/m04-response F004 RECORDED（含切 Toggle 为一行的说明） | **RECORDED（设计决策，reviewer 接受）** |
| **M02-F005 MSVC >50ms → M07.3 真实机器** | m02-filter-bench r02 `CLOSED-as-recorded`；M07.3 待实测 | run `35950631427` BENCH：pinyin 38.90/edit-distance 37.34ms（hosted）均在 500ms 放松界内；known-issues 明确「尚未实测量化，列为（目标/待真实机器测量）」；权威数字落 `manual-acceptance.md` | **MANUAL（真实机器测量，进 task/03 f-2）** |

### 4.4 M03 主搜索窗口（DONE，r02 APPROVED）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M03.1–M03.5 全部验收点 | r02 批准（head 1228d6d） | `ui/app-window.slint`（600/480/760、56px、rows-full-path、ime-composing）；`view_model.rs` 18+ 测试；i18n parity 5 项；theme WCAG AA 4 项 | **CLOSED** |
| F001 结果计数恒 0 / F002 tooltip | r03 期 M03 r02 CLOSED | `src/main.rs:1716` `results_count_label`（u16 饱和）+ 单测 `results_count_label_is_derived_from_the_row_count`（4493）；slint `row-full-path` tooltip | **CLOSED** |
| **原生 IME preedit 接线** | M03 记录留 M04/M07 桌面 | Slint 1.18 无公开 preedit 事件（slint:16-23、view_model:28-38 文档化）；presenter 层 `set_ime_composition` 双层 gate + 3 测试；真实候选位置为桌面项 | **MANUAL**（task/03 g 节） |
| **真实打开/剪贴板（OpenEntry/CopyPath）** | M03 stub → M04/M05 真实接线 | `src/platform/shell_open.rs`（ShellExecuteExW raw path，7 测试）、`src/platform/windows/clipboard.rs`（CF_UNICODETEXT） | **CLOSED**（代码）+ **MANUAL**（真实打开体验） |
| **原生 widget 深色调色板** | M03 记录留 M07/M08 手工 | `ui/app-window.slint:310-316` 注明 Slint 1.18 无公开 color-scheme Window API；自定义 token 由 `ResolvedTheme::resolve` 解析；深色切换为桌面项 | **MANUAL**（task/03 I3/I4） |

### 4.5 M04 Windows 集成（DONE，r02 APPROVED）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M04.1–M04.5 生命周期/热键/单实例/定位/打开 | r02 批准（head d507174）；F001/F002 High 关闭 | `src/platform/windows/mod.rs`（两段式启动、顶层 popup）；`hotkey.rs` 19 测试；ipc 11 + activator 3 + single_instance 2；`window_position.rs` 11；`shell_open.rs` 7 | **CLOSED** |
| **M04-N001 热键不可用托盘提示**（M07 声称 CLOSED） | known-issues CLOSED | `src/main.rs:945-969` `sync_hotkey_from_native` + `:1874` `tray_hotkey_hint`（仅 Disabled+last_error 显示）；slint `hotkey-unavailable-hint` 条件 MenuItem（`ui/app-window.slint:2051-2054`）；启动（main.rs:3088）/设置保存（main.rs:3307,4137-4174）双路径刷新 | **CLOSED**（代码+slint 接线） |
| 真实热键/双实例/混合 DPI/托盘/前台/Explorer 重启/UNC | M04 记录留 M07/M08 手工 | 桌面项：task/03 e-1/e-2/e-3 与 c-2 | **MANUAL** |
| M04.6 Explorer shell verb（后置可选） | 任务清单「后置可选，不阻塞」 | 0.0.1 未实现（Backlog）；未创建 ADR（`docs/adr/` 无 006）；无任何 HKCU\Software\Classes 写入（grep 零命中） | **RECORDED（0.0.1 范围外，Backlog）** |

### 4.6 M05 管理（DONE，r02 APPROVED）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M05 全套（CRUD/对话框/上下文/分类标签/canary） | r02 批准（head bb8d0c7/556d61e）；C1 Critical 关闭 | `folder_id_at` 128 位索引解析（manager.rs:598）；`m05_tests.rs` canary ×2 + disable 保留真实目录；`manager.rs` resolve_duplicate 三 policy | **CLOSED** |
| **M05-L2/L3 / OBS-01/02** | M07 known-issues CLOSED | `undo_dismiss`/`NoticeEnterPath` 双语文案（i18n）/`draft.path.trim().is_empty()`（main.rs:1609）；`RowAction::from_context_action` 命令常量+单测镜像菜单顺序（commands.rs:67-94） | **CLOSED** |
| **M6 重命名/合并 UI**（folded into M06） | M05 r02 DEFERRED→M06 | `repository.rs:731/813/837` rename_category/rename_tag/merge_tag；settings_controller 内 rename/merge 命令 + 重复名拦截（M06 r01 复核 3 项均 ✅） | **CLOSED**（M06 已交付） |
| **OS 拖拽 seam** | M05 r02 RECORDED | Slint 1.18 不透传 DroppedFile（folder_picker.rs 文档化 seam）；粘贴/多选可用；无伪入口 | **RECORDED（Slint 限制，桌面项 task/03 d-1 E3）** |
| **L2/L3/OBS** 与 duplicate-policy UI | M05 r02 记录/修复 | 见上；duplicate 三按钮接线 `on_command_resolve_duplicate`（0/1/2→policy） | **CLOSED** |

### 4.7 M06 设置与数据管理（DONE，r02 APPROVED）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M06.1–M06.6（设置页/启动项/搜索外观/快捷键/数据/关于） | r02 批准（head 935e03c）；H1/M1/M2/M3 关闭 | `settings_controller.rs`（round-trip/clamp/rollback/hotkey 录制/rename-merge）；`file_dialog.rs` FOS_OVERWRITEPROMPT；`tray_open.rs` run_value_for 引号 | **CLOSED** |
| **M06-I1/I2/I3**（M07 声称 CLOSED） | known-issues CLOSED | I1 `backup.rs:83` unique_stamp `-2/-3` + 测试 `unique_stamp_disambiguates_same_second_backups`；I2 `backup.rs:114` remove_failed_import_snapshot（白名单）+ 测试 `failed_import_snapshot_cleanup_removes_only_the_named_backup` + apply Err 回滚；I3 slint `enabled: root.s-import-conflicts == 0`（app-window.slint:1750） | **CLOSED**（逐项代码+测试） |
| hide_on_focus_loss | M07 known-issues 记 M08 手工 | 设置持久化+主窗接线（main.rs:3900、818）；真实 native 失焦钩子为桌面项 | **MANUAL** |
| 真实对话框/热键捕获/混合 DPI/IME/locale/启动项登录 | M06 记录留 M07/M08 手工 | task/03 d-6/e-1/H/J/g 节 | **MANUAL** |

### 4.8 M07 加固（DONE，r02 APPROVED / RC-ready）

| 项 | 关闭声明 | 证据 | 裁定 |
|---|---|---|---|
| M07.1 配置读取失败不退出/损坏保留/panic 脱敏 | r02 CLOSED | `open_repository`→`StartupDataStatus`+Data 页双语通知；`install_redacted_panic_hook`（main.rs:4406）+ `diagnostics::log_panic` 256KB cap+轮转（`panic_log_is_capped_and_rotated`） | **CLOSED** |
| M07.2 并发/生命周期 | r02 CLOSED | `rapid_successive_saves_all_persist_in_order`（1453）、`concurrent_burst_saves_never_torn_main`（1509）、`rapid_toggling_persists_every_command_no_lost_update` | **CLOSED** |
| M07.4 100/125/150/200% 几何 | known-issues CLOSED | `window_placement.rs` `physical_rect_for_monitor_uses_the_target_monitor_dpi` 等 | **CLOSED**（纯几何）+ **MANUAL**（真实显示） |
| M07.5 安全/隐私/供应链 | r02 通过 | `src/storage/tests.rs:709` 无 fs 删除 API、`:791` 无 shell/命令解释器、`:877` workflows pin、`:941` release 无 write；`import_export.rs` 8MiB/100k 上限+pre-check | **CLOSED** |
| M07.6 回归映射 | r02 通过 | `task/02 §12` 映射抽查（见 §5） | **CLOSED（+ 观察 F2）** |
| **M-1..M-4/F-5/N-1** | r02 逐项 CLOSED / N-1 引用已修 | **N-1 修复确认**：`review/0-0-1/m07-hardening-response-r01.md:55-57` 三行 M06-I1/I2/I3 来源已由 `r01` 改为 **`m06-settings-review-r02.md`**（提交 `b503c0c`，如 r02 建议在后续文档提交中顺带修正）；`git grep m06-settings-review-r01.md` 于该 response 仅剩 M05-L2/L3（正确地指向 r01）。**N-1 已修复。** | **CLOSED**（N-1 笔误在 head 已修） |
| M07.3 性能/内存目标 | known-issues 明确「待真实机器」 | hosted BENCH 数字仅回归门禁；50ms/100ms/1s/50MB 全为测量目标（task/03 f-2）；权威数字→manual-acceptance | **MANUAL** |

### 4.9 M08 RC / 验收 / 发布（IN_PROGRESS，未完成项全部合法）

| 项 | 状态 | 说明 |
|---|---|---|
| M08.1 RC 冻结+产物核验 | DONE（rc-remediation-notes + manual-acceptance 候选身份表） | head `a16d28b` run `35981840362` success；EXE/ZIP hash 已核；PE 调试目录清除、无主机路径、`--version` FileGo 0.0.1 |
| M08.2 发布文档 | DONE | release-notes/README/THIRD_PARTY_LICENSES 在案 |
| M08.3 用户手工验收 | **PENDING（用户执行）** | `manual-acceptance.md` 只填候选身份，结果空；须用户按 task/03 逐项 PASS |
| M08.4 独立 release review `APPROVED_FOR_RELEASE` | **PENDING** | 必须由独立 release reviewer 对实际候选 commit 给出 |
| M08.5 tag/GitHub Release | **PENDING（需用户授权）** | 未创建 tag/Release |

## 5. task/02 §12 自动项 → 测试名映射抽查（无静默遗漏）

本审计对 `task/02 §12` 每行自动项在 head 代码/测试中抽查命中：

| task/02 自动项 | 命中证据 |
|---|---|
| 生命周期状态机 | `src/app.rs` tests（8 项） |
| 单实例/第二实例 | `src/platform/ipc.rs`（11 项）、`single_instance/activator.rs`（3）、`single_instance.rs`（2） |
| 热键规则/状态机/mock 失败 | `src/platform/hotkey.rs`（19）、`hotkey_adapter.rs`（5） |
| 托盘不可用热键提示（N001） | `tray_hotkey_hint`（main.rs）+ hotkey 状态机测试；可见性为 slint 绑定（编译期） |
| 安全保存/故障注入/备份恢复/损坏保留 | `repository_tests.rs`（33） |
| repair 加锁争用 | `repair_from_backup_contends_with_a_concurrent_writer_via_the_lock` |
| 快速连续/并发保存 | `rapid_successive_saves_all_persist_in_order`、`concurrent_burst_saves_never_torn_main` |
| 设置快速切换不丢命令 | `rapid_toggling_persists_every_command_no_lost_update` |
| 导入资源上限/原子/路径去重 | `import_export.rs` 17 项（含 F-5 三项/8MiB/100k/all-or-nothing） |
| 覆盖导入快照唯一 stamp（I1）/恢复（M1） | `unique_stamp_disambiguates_same_second_backups`、`overwrite_import_snapshot_restores_the_pre_import_document`（m06_tests.rs） |
| 导入冲突 Apply 禁用（I3） | slint `enabled: root.s-import-conflicts == 0`（编译期） |
| 无真实目录删除（canary） | `m05_tests.rs` canary ×2、`clear_all_records_keeps_every_real_directory_and_settings`、`source_under_storage_and_domain_has_no_fs_delete_api_calls` |
| 无 shell/命令解释器 | `implementation_sources_do_not_invoke_a_shell_or_command_interpreter` |
| 无路径/query 进日志 | `diagnostics.rs` tests、`repository_error_display_contains_no_stored_content` |
| GH Actions pin/最小权限 | `workflows_pin_actions_and_use_minimal_permissions`、`release_workflow_never_runs_untrusted_code_with_write` |
| 搜索核心 | `src/search/` 60+ 测试 |
| 10k release 基准 | `ten_k_release_benchmark_stays_under_lenient_bound`（#[ignore]，benchmark.yml） |
| 几何/DPI | `window_position.rs`（11）、`window_placement.rs` scale 测试 |
| i18n parity | `i18n.rs` tests |
| 数据目录位置 | `location.rs` 纯 base_dir 注入 + m06_tests（base_dir 注入）；guard 无 exe 相对路径常量 |
| 日志轮转 | `panic_log_is_capped_and_rotated` |
| IPC 固定消息 | `ipc.rs` tests（11） |
| 设置 round-trip/前向兼容/回滚 | `settings_controller.rs` tests、`m06_tests.rs`、`settings.rs` tests |
| 开机启动幂等/引号 | `tray_open.rs` tests（run_value） |
| 清除全部/重置与 records 分离 | `m06_tests.rs`（reset-vs-clear、canary） |

- **观察 F2（信息级）**：`task/02` §12 急项中 `ipc.rs tests（13）` 与 head 静态计数 11 不一致（M04 里程碑时 13，后续 M07 维护删减/合并导致 11；`activator.rs` 3 项保持）。测试存在且 CI 全绿，**非能力缺口**；仅为本地映射文档计数陈旧，建议实现方在 task/02 顺带刷新（不影响发布证据）。

## 6. Findings

### 编码/行为缺口

**无 Critical / High / Medium 缺口。** 全部原「延期至后续 phase」的项经本审计核验在 head 上已真实关闭或合法转为 MANUAL/RECORDED（见 §4 裁定列）。

### 观察项

- **F1（信息，task 状态同步）**：`task/01` M01.4 `reset 也先备份` 仍是 `[ ]`。实现语义上已满足——每条 `save_at` 经 `replace_backup_with_current_main()` 先保留旧 main 为 `.bak` 再原子替换（reset 与 clear-all 走同一保存路径，fault 测试已覆盖三触发点）；且用户可见备份 create/list/restore 全在位。**建议实现方在发布前把该勾选更新为 [x] 并在 task 记录中注明依据（`.bak` pre-change 备份 + `save_fault_backup_staging_preserves_previous_backup`）**。这是本地非追踪文档的状态同步，不构成发布阻断。
- **F2（信息，task/02 计数陈旧）**：§5 观察，`ipc.rs` 测试计数 13→11（文档陈旧，非能力缺口）。建议顺带刷新。
- **F3（记录，待手工）**：M07.3 的 50ms/100ms/1s/50MB 数字按 known-issues 与 M-2 处置**不得**在真实桌面实测前当作已达标；权威数字必须进入 `review/0-0-1/manual-acceptance.md`。属正常门禁，非缺陷。

## 7. task/03 发布手工验收清单（Mandate 2 产物）

- 本地文件：`task/03-发布手工验收清单.md`（**git-ignored，未纳入 Git，不提交**）。
- 重写为可执行清单，保持与 `review/0-0-1/manual-acceptance.md` 的 B–M ID 体系一致（B1-B7, C1-C10, D1-D10, E*, F*, G1-G8, H1-H10, I1-I12, J1-J6, K1-K11, L1-L10, M1-M5），并按本任务要求使用 **a)–h) 逻辑分节**：
  - a) 安装与启动（B + C：静默启动/托盘、Explorer 重启托盘恢复、双实例激活、退出）
  - b) 搜索（F + G4：拼音全拼/首字母/英文首字母/编辑距离/多 token/模糊、收藏≤5、空查询、筛选、复制、无结果、不可访问提示、高亮）
  - c) 打开与右键（E 部分 + F14-20 键盘 + M5：双击/Enter、本地/UNC 打开、失败保持窗口+重试/复制、右键 7 项、剪贴板 Unicode、删除语义「从 FileGo 移除」+ canary 硬门禁）
  - d) 管理/设置（E 添加/编辑/分类标签 + I 设置页 + K 数据页：add 多选 picker/粘贴/拖拽、one-level max100 开关+hover、duplicate 三策略、字段/颜色/权重/别名、分类标签 rename/merge、筛选排序、设置 round-trip、import/export/backup、恢复默认 vs 清空分开、数据位置）
  - e) Windows 集成（D + H + J + C11/12：热键注册/触发/冲突/pause、混合 DPI、多显示器定位、前台 fallback、托盘动作、开机启动、登录场景）
  - f) 性能与资源（L + f-2 权威测量：10k<50ms、hotkey→可见<100ms、冷启动<1s、idle CPU/RAM/磁盘、内存<50MB、退出保存）
  - g) IME/a11y（G + I 相关：微软拼音 composition/候选、深色、缩放/高对比、减少动画、纯键盘、屏幕阅读器如实记录）
  - h) 数据安全隐私（M + K 相关：覆盖前备份、损坏恢复、reset/清空不删真实目录、日志无路径、导出不含私密、无遥测）
- 每项含「如何测 / 期望 / 需记录字段」；**结果不预填**，用户在 `review/0-0-1/manual-acceptance.md` 记录。

## 8. 桌面手工验收（manual-acceptance.md）一致性

- `review/0-0-1/manual-acceptance.md` 候选身份表（RC SHA `a16d28b…`、run `35981840362`、EXE/ZIP hash、test date 2026-09-24）为 artifact 核验事实，非验收结果。
- 清单表格仍为空白行（每个必测项待用户 PASS/FAIL/BLOCKED/NOT_TESTED）。已含"Required real-machine measurements"区（10k/hotkey/cold/idle CPU/idle disk/memory），与 task/03 f-2 对齐。
- **注意**：manual-acceptance.md 候选身份表的 RC SHA 为 `a16d28b`；当前 head `88ac84d` 相对它仅 2 份 review 文档（无功能/产物变化）。最终独立 release review 将对实际候选（release 前再确认 tag/CI/产物三 SHA 一致）执行。

## 9. 结论

- **独立判定**：**`MVP 完成度 = 完整`**——每个 M00–M07 里程碑任务项在 head `88ac84d` 上均有实现+测试（或闭环 review）证据；**本审计未发现任何必须在发布前由 implementation agent 关闭的编码缺口**。所有「曾在早期 review 中被延期」的项（M01B-N001 repair 锁、M02-F003、F004、F005、M03 IME/dark、M04 N001、M05 L2/L3/OBS/重命名合并/拖拽、M06 I1/I2/I3、M07 全套）均已真实关闭或合法转为 MANUAL/RECORDED。
- **GNU 测试**：398 passed（394 lib + 4 bin），0 failed，1 ignored；与最后 MSVC 绿色 CI/RC 期望**一致**。
- **head vs RC**：`a16d28b..88ac84d` 仅 2 个 review 文档，**确认仅文档差异**。
- **剩余门禁（均为合法未完成项，非缺口）**：① 用户真实桌面手工验收（task/03 + manual-acceptance.md）；② `M07.3` 真实机器性能/内存在 manual-acceptance 目标（50ms/100ms/1s/50MB）实测；③ 独立 release code-review 对实际候选 commit 给出 `APPROVED_FOR_RELEASE`；④ 用户授权后 tag/Release（M08.5）。CLAUDE.md §4.5 硬门禁在这些完成前不满足，**不得**创建 tag/GitHub Release。
- 建议实现方（发布前，非阻断）：更新 task/01 M01.4 勾选与 task/02 ipc 计数（F1/F2）。

---
*本审计为"完成度审计"，不是 release 批准；`APPROVED_FOR_RELEASE` 仅能由独立 release code-review agent 按 CLAUDE.md §4.5/§6 对实际候选 commit 给出。*
