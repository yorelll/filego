# FileGo M06 设置与数据管理 — implementation response r01

## 元数据

- **版本**: 0.0.1 (目标)
- **主题**: `m06-settings`
- **轮次**: r01 response（对 `m06-settings-review-r01.md`）
- **日期**: 2026-09-21
- **实现 agent**: implementation agent（本 response）
- **对应 review**: `review/0-0-1/m06-settings-review-r01.md` (verdict: `CHANGES_REQUESTED`)
- **修复前 commit**: `5e51113c0f03523e37e31fcad5c7d98a18f6a4dc`（M06 实现 head，review 的 base 为 `556d61e`）
- **修复后 commit**: （见末尾，commit SHA 在推送后补全）
- **比较范围**: `5e51113..<fix-head>`

> 本 response 由 implementation agent 撰写；独立 code-review agent 未参与本轮的实现、
> 测试编写或提交，其 r02 复审将独立查验代码/diff/测试与 CI 证据（见"请求复审"节）。

## 一句话总结

r01 的全部 6 条 finding（H1 High、M1/M2/M3 Medium、L1/L2 Low）均已处理：
**H1/M1/M2/M3/L1/L2 全部 ACCEPTED**。PASS 项（数据安全核心、原子持久化、persist 回滚、
import all-or-nothing、export FOS_OVERWRITEPROMPT、设置 round-trip 与前向兼容、金丝雀测试、
匿名错误、无路径/查询日志）未做任何回归。

## CI 证据（GNU 本地快速验证 + 远程 MSVC CI）

| 项 | 值 |
|---|---|
| GNU fmt | `cargo fmt --all -- --check` PASS |
| GNU clippy | `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` PASS |
| GNU test | `374 passed; 0 failed; 1 ignored`（lib）+ `4 passed`（bin，含新增 H1 守卫测试） |
| GNU release | `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` PASS |
| MSVC CI run | （推送后补全 run ID/URL；head SHA = fix commit） |
| toolchain | Rust `1.92.0-x86_64-pc-windows-gnu`（GNU 仅快速反馈，不代表 MSVC 权威） |
| 备注 | GNU 会话需 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` + `D:\mingw64\bin` PATH（`shlwapi`/windres drift），与本仓既往会话一致 |

## Findings 逐条回应

### H1（High）— 托盘"设置"菜单项硬编码禁用，设置窗口无可达直接入口 — **ACCEPTED**

**评估结论**: 接受。该问题真实存在：`.slint` 中 `enabled: false` 将托盘 Settings
项静态禁用，M06 前后一致，`tray.on_open_settings`（`main.rs`）虽已接线但永不可达，
设置窗口只能通过"添加文件夹"/行内编辑（它们内部会 `window.show()`）间接打开，缺少主入口。

**修复**:
- `ui/app-window.slint`：移除 Settings `MenuItem` 的 `enabled: false`（原注释 `// M06 wires
  the settings window` 的承诺兑现），保留 `activated => { root.open-settings(); }`。
- `src/main.rs`：新增 `open_settings_window()`，在 `tray.on_open_settings` 中调用——
  `window.show()` 后走与 `bring_search_to_front` 相同的原生 `bring_to_front_bits`
  前台焦点回退链（`SetForegroundWindow` → `AttachThreadInput` + `BringWindowToTop`），
  确保窗口不只是"映射"而是前台聚焦。
- **失焦隐藏保证**：SettingsWindow 没有任何 focus-loss hide 接线（`hide_on_focus_loss`
  仅作用于主搜索窗口；原生失焦钩子本就是 M07 桌面手工验收项，见 r01 手工清单 #4），
  因此设置窗口打开后不会被失焦逻辑隐藏 —— 满足"no focus-loss hide while open"。

**测试**: `src/main.rs` 新增 `tray_settings_menu_item_is_enabled`（bin 测试）：
读取 `ui/app-window.slint` 源文件，定位 AppTray 组件中 `title: @tr("Settings")`
所属的 `MenuItem` 块，断言其不含 `enabled: false` 门且仍调用
`activated => { root.open-settings(); }` —— 防止死门被重新引入。

### M1（Medium）— import overwrite/apply 不满足"覆盖前备份"的显式快照 — **ACCEPTED**

**评估结论**: 接受。CLAUDE.md §7 明确"覆盖前备份"；r01 前 Overwrite 导入直接
`set_data`+`save_at`，恢复点依赖用户手动"创建备份"，内部 `data.json.bak` 是
"上次保存"的备份而非"本次覆盖前"的语义快照。

**修复**: `src/main.rs` `apply_import`：在 Overwrite 模式、实际写盘（`set_data` +
`save_at`）**之前**，用标准备份机制（`storage::backup::create_backup`，复用同一备份
列表，可在数据页列出/恢复）对**当前（导入前）文档**写入
`backup-before-import-<YYYYMMDD-HHMMSS>.json`。apply 后刷新数据页备份列表使快照立即可见。
- all-or-nothing 语义保持：parse 失败/apply re-validate 失败根本走不到该分支，
  快照只在"即将真正变更"的前一刻创建；导入本身不触真实目录，快照只是记录级副本。
- Merge/Skip 模式不覆盖现有记录（`classify` 只回 Skipped/Added），无需快照。

**测试**: `src/storage/m06_tests.rs` 新增
`overwrite_import_snapshot_restores_the_pre_import_document`：以真实
`TempDir` + `DocumentRepository` 复现"当前文档 → 规划/apply → 覆盖前快照 → set_data +
save_at"，断言：
- 存在 `backup-before-import-*.json` 且被 `list_backups` 列出；
- `read_backup` 解码结果 == **导入前的**完整文档（单条旧记录、旧 revision）；
- 导入后活动文档 ≠ 导入前文档（真实发生了变更）。

### M2（Medium）— `MonitorStrategy::ActiveWindow` docstring 与实际接线不符 — **ACCEPTED**

**评估结论**: 接受。r01 指出的注释过期属实：docstring 称"0.0.1 maps this to the same
cursor-monitor fallback; persisted for forward-compat"，但 `window_placement.rs:193-241`
(`placement_rect_for_active_window`，真实 `GetWindowRect` → monitor work area、DPI-aware)
与 `main.rs:115-132` (`place_window` 读取 `MONITOR_STRATEGY` 分派) 是真实接线。

**修复**: `src/domain/settings.rs` ActiveWindow docstring 更新为真实行为：
active-window 显示器定位已实现（指向 `placement_rect_for_active_window`），无效/不可用
句柄时回退 cursor 显示器（同 Mouse 策略），并标注 M06 review M2 修正。**未改动任何
功能代码**（working feature 未降级）。

### M3（Medium）— Overwrite 导入可产生重复路径记录（id 优先分支绕过路径去重）— **ACCEPTED**

**评估结论**: 接受。复现正确：incoming 记录 `id==A.id` 但 `path==B.path`（A、B 为当前
两条不同记录）时，`classify` 因 id 命中回 `Updated`，`apply_import` 按 id 替换 A，
替换后 A 与 B 同 path，`validate_and_normalize` 只查重复 id/tag/category，不查重复路径，
双重导入存储即可持久化 —— 违反 M01.2/仓 `is_duplicate_path` 的路径唯一不变量。

**修复（选择 all-or-nothing）**: `src/storage/import_export.rs`
- `classify`：Overwrite 模式下，若 id 命中记录 A 但传入 path 与**另一条**当前记录 B
  冲突（且 A 自己的 path 与传入 path 不同），回 `ItemStatus::Conflict`（预览即显示
  conflict，不再伪装成 updated）。
- `apply_import`：`plan.conflicts` 非空即返回新错误 `ImportErrorKind::DuplicatePath`
  —— 拒绝**整个**导入（all-or-nothing），绝不只删冲突记录而悄悄应用其余记录
  （路径冲突语义二义，无法自动裁决，最安全）。
- 兜底不变量 `duplicate_paths(&data.folders)`：对合并后文档按 M01.2（与仓
  `duplicate_folders` 相同的"class + normalized"复合键）做最终路径唯一校验，
  任一命中即 `DuplicatePath` —— 持久化文档结构上不可能出现重复路径。
- 新 i18n 键/文案 `settings.notice.import_duplicate_path`（中/英），
  `settings_controller.rs` 新增 `SNotice::ImportDuplicatePath`，`main.rs`
  `apply_import` 的 `Err(DuplicatePath)` 映射到该通知（其余错误保持原
  `ImportUnresolvedReference`）。

**测试**（`src/storage/import_export.rs`）:
- `overwrite_where_id_match_duplicates_another_records_path_is_refused`：构造 id 命中 A、
  path 撞 B 的 incoming；预览显示 conflict（非 updated）；apply 返回 `DuplicatePath`，
  当前文档不变（无 mutation）。
- `non_colliding_overwrite_still_applies`：id 命中且 path 与自身记录一致（改名不改
  path）的 Overwrite 仍正常 apply，路径唯一（`C:\docs` 恰好 1 条）。
- `own_export_overwrite_round_trip_is_still_accepted`：自身导出再 Overwrite 导入
  （相同 id + 相同 path 的常规"恢复同一文件"）不误报 conflict、不产生重复路径。

### L1（Low）— `launch_at_login_wired` 死字段 — **ACCEPTED（保留并记录为 legacy）**

**评估结论**: 接受观察（该字段从不写、从不读），但**保留字段**并文档化，而非删除。
理由：`AppSettings` 以 `#[serde(deny_unknown_fields)]` 解码，M06 及之前写出的所有
磁盘文档都序列化此键；直接删除字段会让已有用户数据文件解码失败（回归数据安全核心）。
HKCU Run 值已是启动项唯一事实来源（`set_launch_at_login_os` 实时跟踪 + 启动/切换时读
注册表，r01 M06.3 表 PASS）。

**修复**: `src/domain/settings.rs` 为该字段补 legacy 说明注释（"M06 review L1：legacy
forward-compat；从不参与逻辑；因 deny_unknown_fields 保留以兼容既有文档；始终为
default false"）。未改任何行为。

### L2（Low）— 冗余/未使用的 `ExportTargetExists` 通知与 4 个未接线回调 — **ACCEPTED（移除）**

**评估结论**: 接受，均为死代码：
- `SNotice::ExportTargetExists` 从未产生（导出走 `FOS_OVERWRITEPROMPT` OS 级确认，
  不可能走到该枚举）；
- `app-window.slint` 声明的 `s-command-arm-clear-all` / `s-command-cancel-clear-all` /
  `s-command-arm-reset` / `s-command-cancel-reset` 从未被控件触发（两步确认完全由
  slint 本地 `s-confirm-*` 属性管理），main.rs 也未接线。

**修复**:
- `settings_controller.rs`：删除 `ExportTargetExists`，替换为真实使用的
  `ImportDuplicatePath`（M3）。
- `i18n.rs` / `main.rs` / `app-window.slint`：删除 `settings.notice.export_target_exists`
  i18n 键与 `settings-notice-export-target-exists` slint 属性（及
  `MonoNoticeExportTargetExists`、`apply_settings_localization`/`call_string_setter` 对应
  条目），替换为 `...import_duplicate_path` 键/属性。
- `app-window.slint`：删除 4 个未触发的 arm/cancel 回调声明，保留并接线
  `s-command-confirm-clear-all` / `s-command-confirm-reset`（slint 内两步确认按钮继续
  只调 confirm 回调，本地属性管理 arming，行为不变）。

## 未解决事项与待人工验证项（延续 r01 手工清单，未在本轮解决）

- 原生失焦隐藏钩子（M07）、native 文件对话框（save/open FOS_OVERWRITEPROMPT）真实
  Explorer 表现、hotkey 录制真实 Ctrl+Alt+Space 捕获/冲突/暂停/恢复、hide/clear
  after-open 与失焦隐藏桌面行为、多显示器/混合 DPI 下 ActiveWindow/Mouse 定位、
  系统语言切换后的 locale 即时更新（中文 IME/字体）、导入预览+应用端到端、
  启动（silent/show-main）登录场景、数据目录打开的 ShellExecuteExW 桌面效果。
- H1 的"从托盘真实打开设置窗口"是桌面必测项（r01 手工清单 #1），需真实桌面验证。

## 请求复审

本 implementation agent 请求独立 code-review agent 对 **fix commit** 进行 r02 复审：
- 复核代码/diff（重点：H1 托盘菜单使能与 `open_settings_window` 前台聚焦、
  M1 覆盖前快照时机、M3 路径冲突拒绝的 all-or-nothing 语义与 `classify`/`apply`
  一致性、L1/L2 清理是否完整无残留）；
- 复核新增测试（H1 守卫、M1 快照、M3 三条）与实际 CI（MSVC run + GNU 本地证据）；
- 确认 PASS 项无回归（金丝雀、rollback、round-trip、前向兼容）。

**H1 是必须关闭的 High**，请优先复审。
