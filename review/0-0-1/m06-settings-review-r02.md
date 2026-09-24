# FileGo M06 设置与数据管理 — 独立 Code Review r02 (复审)

## 元数据

- **版本**: 0.0.1 (目标)
- **主题**: `m06-settings`
- **轮次**: r02（对 r01 response / fix commit 的独立复审）
- **日期**: 2026-09-21
- **评审对象**: r01 全部 findings 的修复（H1/M1/M2/M3/L1/L2）与回归检查
- **Base**: `5e51113c0f03523e37e31fcad5c7d98a18f6a4dc`（r01 的 head，"feat: complete settings pages and data management"）
- **Head**: `3263f06761391d63fe0dcbfbca98df9493aabcfd`（"fix: enable tray Settings, back up before import, refuse dup-path overwrite"）
- **比较范围**: `5e51113..3263f06`（含 `d409431` "docs: record M06 settings review r01" 与 `3263f06` 修复提交）
- **文件范围 (9 files, +820/-38)**:
  - `review/0-0-1/m06-settings-review-r01.md`、`m06-settings-response-r01.md`（评审/响应文档）
  - `src/domain/settings.rs`（M2 docstring 修正、L1 注释）
  - `src/main.rs`（H1 `open_settings_window` + 托盘菜单使能 + M1 覆盖前快照 + M3 通知映射 + H1 守卫测试；L2 清理）
  - `src/presentation/i18n.rs`（M3 新键；L2 死键移除）
  - `src/presentation/settings_controller.rs`（M3 `ImportDuplicatePath`；L2 死枚举移除）
  - `src/storage/import_export.rs`（M3 冲突分类 + all-or-nothing 拒绝 + 兜底不变量 + 3 测试）
  - `src/storage/m06_tests.rs`（M1 快照测试）
  - `ui/app-window.slint`（H1 菜单使能注释、M3/L2 属性清理）

## Reviewer 角色与独立性声明

- **Reviewer**: 独立 code-review agent（本评审）。
- **独立性声明**: 本 agent **未参与** M06 的实现 (`5e51113`)、r01 response 文档撰写及 fix commit (`3263f06`) 的任何实现、测试编写、提交或 CI 触发；未修改任何源码、未提交、未运行 cargo。所有结论均来自对实际代码、diff、测试与 CI 证据的独立查阅，不采信实现报告中的任何断言。
- 本评审只产出此 Git 追踪的 review 文档，不兼任 implementation agent。

## CI 证据（独立复核）

| 项目 | 值 |
|---|---|
| Windows CI run | `35938791830` |
| workflow | Windows CI |
| head SHA | `3263f06761391d63fe0dcbfbca98df9493aabcfd` |
| branch | `feature/m00-foundation` |
| status/conclusion | completed / **success** |
| 测试数 (CI log) | lib：`running 375 tests` → `test result: ok. 374 passed; 0 failed; 1 ignored`；bin：`running 4 tests` → `4 passed; 0 failed`（1 ignored 即 `benchmark.rs` release-only 10k 基准）；doc 2 套件 0 测试 |
| clippy | `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-msvc -- -D warnings` 通过（日志无 clippy 警告） |
| fmt | `cargo fmt --all -- --check` 通过（本 run 若失败即不产出 release 产物） |
| MSVC Release 打包 | 本 run 下载到 `FileGo-0.0.1-windows-x86_64-3263f06....zip` + `.zip.sha256` artifact（run 35938791830 download） |
| Search benchmark run | `35938791802`，head 同 `3263f06`，结论 success；BENCH 实测：empty-query-default median=0.81ms、filtered-with-clone 58.63ms、pinyin-heavy 66.76ms、english-initials 54.15ms、edit-distance 63.45ms、multi-token 51.81ms；`ten_k_release_benchmark_stays_under_lenient_bound ... ok`（1 passed, 374 filtered out） |

**测试计数核对**: 本 repo `#[test]` 注解共 379 处（r01 时为 374，本次 fix 新增 5：main.rs H1 守卫、m06_tests.rs M1 快照、import_export.rs M3 三条）。CI lib 套件 374 通过 + 1 ignored（release 基准）+ bin 套件 4 通过（含 H1 守卫 `tray_settings_menu_item_is_enabled`）= 379，与源码计数一致。r02 请求的 "374+4" 口径核对无误。

## r01 Findings 逐条复审

### H1（High）— 托盘"设置"菜单项被禁用 — **CLOSED** ✅

**复验代码**:
- `ui/app-window.slint:2006-2013`：Settings `MenuItem` 的 `enabled: false` 已移除，仅保留注释说明并保留 `activated => { root.open-settings(); }`。与 base (`git show 5e51113:...)` 中 `enabled: false; // M06 wires the settings window.` 对比确认是**新增去门**而非遗漏。
- 回调链完整：`AppTray` 声明 `callback open-settings()`（slint:1982）→ `root.open-settings()`（slint:2012）→ `tray.on_open_settings`（main.rs:3147-3155）调用新增的 `open_settings_window`（main.rs:169-186）。
- `open_settings_window` 真实聚焦：`window.show()` 后走与 `bring_search_to_front` 相同的 winit raw-window-handle 获取 + `bring_to_front_bits(hwnd_bits, true)`（`window_focus.rs` 的 `ShowWindow/SW_SHOW → SetWindowPos HWND_TOP → SetForegroundWindow → AttachThreadInput+BringWindowToTop+SetActiveWindow` 前台回退链）。不是只挂个旗标。
- `window.upgrade()` 可用性：`SettingsWindow::new()`（main.rs:2919）先于托盘接线（3147）创建，adapter 持有 `settings_window.as_weak()`（main.rs:3113-3120）；`weak` 升级失败时 `open_settings_window` 提前 return，安全。
- 失焦隐藏：`hide_on_focus_loss` 仅接线到主搜索窗口（main.rs:3718 `settings_window.on_s_command_hide_on_focus_loss` 只写 settings + 主窗口），SettingsWindow 无 focus-loss hide 逻辑，打开后不会被隐藏。断言与代码一致。
- **守卫测试** `tray_settings_menu_item_is_enabled`（main.rs:4212-4250）：读取 `ui/app-window.slint` 源，定位 AppTray 组件中 `title: @tr("Settings")` 所属 MenuItem 块，断言不含 `enabled: false` 且仍含 `activated => { root.open-settings(); }`。我独立核对其块定位逻辑（`rfind("MenuItem {")` → 两个 `}` 闭合）对该 slint 文本成立，且断言字符串精确匹配 `enabled: false`（注释提及其名不会误伤）。该测试通过（bin 套件 4 passed 之一）。

**结论**: H1 修复真实、聚焦真实（native bring-to-front 链）、有守卫测试。**CLOSED**。

### M1（Medium）— import overwrite/apply 缺少显式"覆盖前快照" — **CLOSED** ✅ (Merge/Skip 豁免理由成立)

**复验代码**:
- `apply_import` adapter（main.rs:1104-1120）：`apply_import`（纯函数）成功返回后、`set_data`/`save_at` **之前**，若模式为 `Overwrite` 则 `backup::create_backup(&self.data_dir, &current, &stamp)` 以 `before-import-<YYYYMMDD-HHMMSS>` 做标准用户备份（复用同一 backup 列表、同一 durable 写路径 `backup.rs:64-73`）。
- 时点正确："此刻"快照 = 导入前文档；若 parse/校验失败根本到不了此分支（all-or-nothing 保持）。apply 后 main.rs:1134-1136 `list_backups` + `set_backups` 刷新数据页备份列表，快照立即可见/可恢复。
- **测试** `overwrite_import_snapshot_restores_the_pre_import_document`（m06_tests.rs:73-133）：真实 TempDir + 仓库，断言存在 `backup-before-import-*`、被 `list_backups` 列出、`read_backup` 解码 == **导入前的完整文档**（单条旧记录、旧 revision=2、`C:\docs`），且 live 文档 ≠ 导入前（真实变更发生）。
- **Merge/Skip 豁免**: 独立核对 `classify` —— Merge 模式 id 命中回 `Skipped`、path 冲突回 `Skipped`（import_export.rs:230-251），`apply_import` 中 Merge/Skip 不替换现有记录（Updated 分支被跳过）。故 Merge/Skip **不修改现有记录**（最多 Added 新记录），无需覆盖前快照。**该豁免与 CLAUDE.md "覆盖前备份" 精神一致**（备份针对的是"覆盖现有数据"），可接受。记录之。

**结论**: M1 修复真实、时点正确、有强断言测试；Merge/Skip 豁免理由成立。**CLOSED**。另见 I1（同秒 stamp 碰撞，Info 级）。

### M2（Medium）— `MonitorStrategy::ActiveWindow` docstring 过时 — **CLOSED** ✅

**复验代码**: `src/domain/settings.rs:79-86` docstring 现明确描述"active-window 显示器定位已在 `window_placement::placement_rect_for_active_window` 真实实现（`GetWindowRect` → monitor work area、DPI-aware），无效/不可用句柄回退 cursor 显示器（同 Mouse 策略）"，并标注 M06 review M2 修正。与 `window_placement.rs:193-241` 及 `main.rs:115-132` 的分派逻辑一致。diff 仅注释，**未改动任何功能代码**。
**结论**: **CLOSED**。（原功能代码非本 fix 引入，仍 real。）

### M3（Medium）— Overwrite 导入可产生重复路径记录 — **CLOSED** ✅

**复验代码**（import_export.rs 本 fix 修改 + 兜底不变量）:
1. **classify 语义**（:211-237）：id 命中后，若 (a) 传入 path 与 id 命中记录 A 的现 path 不同（`id_matched_path_differs`）且 (b) 与**另一条**现有记录 B 同 path（`collides_with_different_record`，按 M01.2 `same_path` = `path_key` class+normalized），则 Overwrite 模式回 `ItemStatus::Conflict`（预览显示冲突而非 updated）。判定与 `same_path` 语义一致（同一 path_key 比较），无大小写/规范化漂移。
2. **apply all-or-nothing**（:358-371）：`plan.conflicts` 非空即 `Err(ImportErrorKind::DuplicatePath)` —— **拒绝整个导入**，绝不放行非冲突记录而丢弃冲突记录（路径冲突语义二义、不能自动裁决，拒绝整批最安全）。
3. **兜底不变量**（:385-393, :399-415）`duplicate_paths(&data.folders)`：对合并后文档用 `path_key` 的 `class` + `normalized` 复合键（`format!("{:?}\u{1}{}", key.class, key.normalized)`，与 **repository.rs:880 `duplicate_folders` 完全一致**）做最终路径唯一校验，任一命中即 `DuplicatePath` —— 持久化文档结构上不可能出现重复路径。此兜底关键：`validate_and_normalize`（document.rs:20-80）只查重复 id/tag/category 引用与 favorites 上限，**不查路径**，故该兜底是路径唯一不变量在 import 路径上的真正强制者。
4. **i18n/通知**: `ImportErrorKind::DuplicatePath`（import_export.rs:64）→ `SNotice::ImportDuplicatePath`（settings_controller.rs:98）→ `Msg::MonoNoticeImportDuplicatePath`（i18n.rs:328）→ 键 `settings.notice.import_duplicate_path` 在 zh-CN（"导入被拒绝：将产生重复路径的记录"）与 en-US（"Import refused: it would create records with duplicate paths"）均有且各表唯一、在 `ALL_KEYS`（i18n.rs:854），i18n parity 测试（i18n.rs:1665-1701）兜底。slint 属性 `settings-notice-import-duplicate-path` 替换旧键，`main.rs:1150-1154` apply 失败按 kind 分派（`DuplicatePath`→专用通知，其余→原 `ImportUnresolvedReference`）。无路径/文件名泄漏（错误 Display 纯文案）。
5. **UI 反馈**: 预览显示 conflict 计数（`settings-data-import-conflicts`，slint:1722；`set_s_import_conflicts` main.rs:887）。Apply 按钮在冲突存在时仍可点击，点击后由 all-or-nothing 拒绝 + 通知给出明确反馈 —— 语义闭环。
6. **测试**（import_export.rs，3 条，均随 CI 通过）:
   - `overwrite_where_id_match_duplicates_another_records_path_is_refused`：id 命中 A、path 撞 B → 预览 conflict（updated_count=0）→ apply 返回 `DuplicatePath`，current 不变。
   - `non_colliding_overwrite_still_applies`：id 命中且 path 与自身一致（改名不改 path）→ 正常 Updated + Added，`C:\docs` 恰好 1 条（路径唯一保持）。
   - `own_export_overwrite_round_trip_is_still_accepted`：自身导出再 Overwrite 导入（相同 id + 相同 path 的"恢复同一文件"流）不误报冲突、无重复路径。

**all-or-nothing 选择评估**: 正确且安全 —— 被拒绝路径在"同一 id 换 path 撞另一条记录"下无法自动裁决"该信 id 还是该信 path"，整批拒绝 + 明确匿名通知是对用户最透明、对数据不变量最安全的行为。无真实目录删除、record-level、失败零 mutation。**结论: CLOSED**。

### L1（Low）— `launch_at_login_wired` 死字段（保留为 legacy） — **DOCUMENTED / CLOSED** ✅

**评估**: 保留并加注释的决策**成立**。核验：
- `AppSettings` 以 `#[serde(deny_unknown_fields)]` 解码（settings.rs:302）。
- `launch_at_login_wired` 参与默认序列化（settings.rs:364 `#[serde(default)]` + :433 默认 false；`settings_round_trip` 真实文件写出的 JSON 含该键）。
- **若删除该字段**：任何 M06/M05-era 磁盘文档（其 settings 对象含此键）解码即失败（deny_unknown_fields 拒绝未知键）→ 数据安全回归。故"保留字段以满足前向兼容"是在本 schema 约束下的**正确**取舍，不是偷懒保留。
- 已有测试印证：`old_document_without_m06_settings_fields_decodes_with_defaults`（m06_tests.rs:325，剥离 M06 键后仍可解码）；settings.rs:260 断言新文档 `launch_at_login_wired == false`。
- **无行为变化**：该字段从不被逻辑读/写（全仓 grep 仅定义、默认、测试断言；HKCU Run 值通过 `set_launch_at_login_os`/`launch_at_login()` 实时跟踪）。
**结论: DOCUMENTED（按 legacy forward-compat 语义保留）+ CLOSED**（无行为面）。

### L2（Low）— `ExportTargetExists` 通知与 4 个未接线回调 — **CLOSED** ✅

**复验代码**:
- `SNotice::ExportTargetExists` 已删除（settings_controller.rs:96 处替换为 `ImportDuplicatePath`）；全仓 grep `ExportTargetExists`/`export_target_exists` 仅剩 review 文档与 i18n.rs:327 一处说明注释（合法）。
- i18n 键/属性 `settings.notice.export_target_exists`、`settings-notice-export-target-exists`、`MonoNoticeExportTargetExists`、`set_settings_notice_export_target_exists`、`call_string_setter`/`apply_settings_localization` 对应条目已全部移除并替换为 M3 新键。zh/en parity 测试通过。
- slint 4 个死回调（`s-command-arm-clear-all`/`s-command-cancel-clear-all`/`s-command-arm-reset`/`s-command-cancel-reset`）已从 SettingsWindow 声明块删除（slint:1222-1229 区域），仅保留 `s-command-confirm-clear-all`/`s-command-confirm-reset`（slint:1227-1228）。两步确认状态确实完全由 slint 本地 `s-confirm-*` 属性管理（slint:1762-1773：`s-confirm-reset`/`s-confirm-clear-all` arming + cancel 均在 slint 内），Rust 仅接最终 confirm（main.rs:4087 `on_s_command_confirm_reset`、:4093 `on_s_command_confirm_clear_all`），行为不变。
- 注意：slint:1219 `s-command-cancel-import` 是导入取消（import-flow 中真实使用，slint:1734 触发），**不是**被删的 4 个之一，属误识别排除项；正确保留。
**结论: CLOSED**（死代码清理完整无残留）。

## 回归 / 范围检查（r02 新增观察）

- **diff 范围无越界**：`git diff 5e51113 3263f06 --name-only` = 8 个源码/UI 文件 + 2 个 review 文档（r01 review 于 `d409431` 入库、response 于 `3263f06` 入库）。全部为 M06 findings 修复的合理归因。数据安全核心文件 —— `src/storage/io.rs`、`repository.rs`、`codec.rs`、`schema.rs`、`location.rs`、`domain/document.rs`、`domain/path_semantics.rs`、`platform/windows/tray_open.rs`、`file_dialog.rs` **均零改动**（diff --stat 为空）。
- **数据安全核心无回归（自动）**: 金丝雀 `clear_all_records_never_deletes_real_directories`（m06_tests.rs:166-210 附近，真实临时目录 + 嵌套文件 + Unicode 目录名存活）、persist 双回滚（settings_controller 测试）、import all-or-nothing、export `FOS_OVERWRITEPROMPT`（file_dialog.rs，未改）、设置 round-trip 与前向兼容、canary 均随 CI 374+4 全绿通过。
- **无新增依赖**：`git diff ... -- Cargo.toml Cargo.lock` 为空。`deny.toml`/`cargo-about` 仓库在 CI 跑，无 license 新引入。
- **无 `#[allow]` 新增**：diff 中 `#[allow` 零新增。clippy `-D warnings` 通过（CI 日志）。
- **确定性**：`classify`/`duplicate_paths` 纯函数、顺序遍历、复合键确定（无 wall-clock/随机参与 tif-break）；`duplicate_paths` 与 repository `duplicate_folders` 用同一 key（`{:?}\u{1}{}` class+normalized），行为一致无分歧。
- **隐私/日志**：新增错误 Display 全部匿名固定文案（`ImportErrorKind::DuplicatePath` → "the import would duplicate a folder path"）；i18n 新文案不含用户路径/文件名；`open_settings_window` 无日志。H1 相关无敏感信息。
- **M2 working feature 未降级**：ActiveWindow 真实接线保持。

## 新增 Findings（r02 引入或新发现）

### Critical / High / Medium

无。

### Low

无（未发现需修复的新缺陷）。

### Info（观察项，不阻塞）

- **I1 — M1 快照 stamp 秒级粒度（同秒覆盖导入会碰撞）**：`before-import-<YYYYMMDD-HHMMSS>` 秒级分辨率（main.rs:1114-1119）。若用户在同一秒连续两次 Overwrite 导入，第二次 `create_backup` 会用 `File::create`（truncate）覆盖同一文件名。影响面：极小 —— 第二次导入覆盖了第一次的"导入前"快照点，但第一个导入本身已发生且 `data.json.bak` 仍保存更早状态；不会造成数据损坏或丢失（无删除）。可作为后续（M07.3）改进：stamp 加毫秒/序号。**不阻塞**。
- **I2 — 快照在保存失败时多留一份**：若 `apply_import` 成功计算但随后 `set_data`/`save_at` 失败，`before-import-*` 快照仍会留下（属冗余备份）。无害（备份只增不删），与原 M1 目的相符，反而增强可恢复性。**不阻塞**。
- **I3 — apply 按钮在 conflict 存在时不禁用**：预览显示冲突计数但 Apply 仍可点，点击后由 all-or-nothing 拒绝 + 弹出专用通知。语义上安全但 UX 可更早拦截（如 conflict>0 时禁用 Apply）。属设计取舍，非缺陷；记录给实现方后续可优化。**不阻塞**。

## Cross-cutting 检查（r02 复核）

- **正确性**: M3 的 classify→apply 一致性 —— `plan_import` 与 `apply_import` 用同一个 `classify`（import_export.rs:271, :326），预览与应用不可能分叉；最终 `duplicate_paths` 兜底独立于 classify，即使 classify 漏判也拦截。M1 快照内容=`current`（`settings.document()` 全量 StoredDocumentV1）恰为"导入前文档"，非 post-apply。
- **错误处理与数据安全**: 拒绝导入零 mutation（`apply_import` 先构建 `data` 再校验，`Err` 时 `current`/`folder` 局部引用未改动调用方）；快照失败 `let _ =` 忽略 —— I2 说明无害（备份可选增强，若备份失败导入仍继续，不违反"覆盖前备份"的主要路径；失败路径靠 `data.json.bak`）。H1 `open_settings_window` 在 weak 死亡/句柄无效时安全返回。
- **隐私**: 见上，全匿名。
- **安全**: 无新网络/提权面。
- **Windows 行为**: H1 聚焦链复用 M04 已评审的 `bring_to_front_bits`（window_focus.rs，无新 Win32 面）。
- **测试覆盖**: H1（源守卫）、M1（真实文件快照还原）、M3（3 条：拒绝/仍适用/自身 round-trip）均随 MSVC CI 374+4 通过；基准 run 35938791802 通过。
- **性能**: M3 `duplicate_paths` O(n)（一次性、经 `seen` 线性查同 path），对 10k 规模可忽略；`classify` 对每条 incoming 遍历 current（O(n·m)，与原有分类同复杂度），无新热路径。
- **可访问性**: 无新禁用项；settings 窗口现在有主入口（H1 修复可达性）。
- **可维护性**: 注释精炼指向真实代码；死代码清理彻底；`duplicate_paths` 与 repository 共用 key 语义（有注释互指）。

## 未能自动验证的 GUI/平台项（桌面手工验收，延续 r01 清单，不阻塞本里程碑批准）

1. **从托盘真实打开设置窗口**（H1 修复的直接验收点）—— 桌面点击"设置"确认窗口显示并前台聚焦；验证窗口不被失焦隐藏。
2. **native 文件对话框（save/open）的 FOS_OVERWRITEPROMPT** 在真实 Explorer 上提示覆盖确认。
3. **hotkey 录制真实的 Ctrl+Alt+Space** 捕获、冲突提示、暂停/恢复、清除（native machine 只能在桌面验证）。
4. **隐藏/清空 after-open、失焦隐藏** 在真实桌面上的行为（native 失焦钩子为 M07.1/桌面项）。
5. **多显示器/混合 DPI** 下 ActiveWindow 与 Mouse 策略的窗口定位。
6. **系统语言切换后的 locale 即时更新**（中文 IME 与字体渲染）。
7. **导入预览 + 应用** 在真实文件对话框下的端到端（含 overwrite/merge/skip 选择与重复路径冲突提示）。
8. **启动（silent/show-main）** 在登录启动场景下的真实表现。
9. **数据目录打开** 的 ShellExecuteExW 在桌面的实际效果。

以上均为 M07 / 真实桌面验收范围，M06 里程碑批准不要求在此轮完成；已在实现方 response 与 task M07 清单中记录。

## 结论

**Verdict: `APPROVED_FOR_MILESTONE`**

- r01 全部 findings 处置复核：**H1/M1/M2/M3 — CLOSED**（代码+测试+CI 证据齐全）；**L1 — DOCUMENTED（legacy 保留理由成立，正常关闭）**；**L2 — CLOSED**（死代码清理无残留）。
- M3 all-or-nothing 拒绝、M1 Merge/Skip 豁免均合理，且我看见兜底不变量 `duplicate_paths`（复合键与 repository `duplicate_folders` 完全一致）在 `validate_and_normalize` 不查路径的前提下真正封住了重复路径进入持久文档的所有路径。
- 修复 diff 严格限定在 M06-justified 文件；数据安全核心（金丝雀、rollback、原子持久化、round-trip、export FOS_OVERWRITEPROMPT、无真实目录删除、无 fake UI）**零改动**，随 MSVC CI 洞 374+4 全绿。
- 无新增 Critical/High/Medium/Low；仅 3 条 Info 观察（I1 秒级 stamp 碰撞、I2 冗余快照、I3 apply 按钮可提前禁用以改善 UX），均不阻塞，记录供 M07.3 优化。
- 遗留的诚实项目（native 失焦钩子、真实对话框/热键录制/混合 DPI/IME/locale/登录启动等）已明示为 M07 与真实桌面手工验收范围，不阻塞 M06 里程碑。
- 数据安全、错误处理、隐私、安全、Windows 行为、测试覆盖、性能、可维护性经独立复核无阻断缺陷。

**注意**: 本里程碑批准 ≠ release 批准。M06 里程碑可标记完成（配合 task 状态更新）；release 仍需候选 commit 冻结后的独立 release review + 用户真实桌面手工验收（`APPROVED_FOR_RELEASE`）。
