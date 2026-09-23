# FileGo M06 设置与数据管理 — 独立 Code Review r01

## 元数据

- **版本**: 0.0.1 (目标)
- **主题**: `m06-settings`
- **轮次**: r01
- **日期**: 2026-09-21
- **评审对象**: M06 完整设置页 + 数据管理 (import/export/backup/reset/clear-all) + M05 延期项 (重命名/合并)
- **Base**: `556d61ec440700d59856110a4e23cdba44ddbd67` (M05 approval head, "docs: record M05 management review r02 approval")
- **Head**: `5e51113c0f03523e37e31fcad5c7d98a18f6a4dc` ("feat: complete settings pages and data management")
- **比较范围**: `556d61e..5e51113`
- **文件范围 (19 files, +6845)**:
  - `src/domain/settings.rs` (+215)
  - `src/main.rs` (+2181)
  - `src/platform/windows/{file_dialog.rs (new), mod.rs, tray_open.rs, window_placement.rs}`
  - `src/presentation/{settings_controller.rs (new), i18n.rs, manager.rs, mod.rs}`
  - `src/search/{mod.rs, scoring.rs, search_entry.rs}`
  - `src/storage/{backup.rs (new), import_export.rs (new), m06_tests.rs (new), mod.rs, repository.rs}`
  - `ui/app-window.slint` (+738)

## Reviewer 角色与独立性声明

- **Reviewer**: 独立 code-review agent (本评审)。
- **独立性声明**: 本 agent **未参与** M06 (commit `5e51113`) 或任何更早里程碑的实现、调试、测试编写、提交或 CI 触发；未修改任何源码、未提交、未运行 cargo。所有结论均来自对实际代码、diff、测试与 CI 证据的独立查阅，不采信实现报告中的任何断言。
- 本评审只产出此 Git 追踪的 review 文档，不兼任 implementation agent。

## CI 证据

| 项目 | 值 |
|---|---|
| Windows CI run | `35934630570` |
| workflow | Windows CI |
| head SHA | `5e51113c0f03523e37e31fcad5c7d98a18f6a4dc` |
| branch | `feature/m00-foundation` |
| status/conclusion | completed / **success** |
| 测试数 (CI log) | `test result: ok. 370 passed` (主套件) + `3 passed` (bench 套件正常 run) |
| 10k 基准 run | `35934630644` (release 10k benchmark), conclusion **success**, head 同 `5e51113` |
| 基准数值 (CI log) | empty-query median 0.63ms / filtered-with-clone 46ms / pinyin-heavy 52.5ms / english-initials 44ms / edit-distance 53ms / multi-token 42ms — 均在宽松 bound 内，真实 release 跑 |

**测试计数核对**: `src/` 中 `#[test]`/`#[tokio::test]` 共 374 处。CI 主套件 370 + bench 套件 3 = 373 通过 + 1 个 `#[ignore]` (benchmark.rs:309 `ten_k_release_benchmark_stays_under_lenient_bound`, release-only) = 374。计数与 CI 日志一致。

## 需求映射 (M06.1–6.6 + M05 延期 + adversarial items)

标记: ✅ PASS / ⚠️ PARTIAL / ❌ NOT-BOUND / 🚫 FAKE

### M06.1 设置窗口结构

| 项 | 结果 | 证据 (file:line) |
|---|---|---|
| 默认 900×640, min 760×520 | ✅ | `ui/app-window.slint:989-992` (SettingsWindow preferred/min) |
| 左导航 9 页 | ✅ | `ui/app-window.slint:1246-1290` (9 个 nav Button, page 0..8) ; `settings_controller.rs:46-57` SPage 枚举 |
| 右内容滚动、无嵌套弹窗 | ✅ | 各页 `ScrollView` + 内嵌 `Rectangle` 卡片, 无 `Window` 嵌套 (`app-window.slint:1301-1793`) |
| 设置即时生效 | ✅ | `main.rs:265-276` apply_settings 立即重建 ViewModel; `settings_controller.rs:683-718` persist |
| 表单草稿关闭提示 | ✅ | M05 H2 `draft-unsaved` 警告 + 两步放弃 (`app-window.slint:1806-1812,1901-1905`); 任务清单 M06.1 明示此即"存在表单草稿时关闭提示" |
| 危险操作底部 + 二次确认 | ✅ | `app-window.slint:1756-1774` (数据页底部 clear-recent/reset/clear-all) |
| 无多模态焦点/失焦问题 | ✅ | 全部内嵌面板, 无独立 Dialog 窗口 |

### M06.2 常规

| 项 | 结果 | 证据 |
|---|---|---|
| launch-at-login (HKCU, 幂等, 失败回滚无虚假成功) | ✅ | `main.rs:3595-3611` (先写 HKCU, 成功才 persist + 更新 OS 状态 + tray glyph; 失败 `StartupWriteFailed`); `tray_open.rs:45-90` set_launch_at_login 幂等; `settings_controller.rs:692-718` 写失败视图+工作副本双回滚 |
| silent start / show-main-at-startup | ✅ | `main.rs:2753-2780` (持久化配置驱动启动可见性: `!silent_start || show_main_window_at_startup`) |
| start-notification 默认关 + 文档化 | ✅ | `settings.rs:343-346,421-422` (默认 false 且无此字段); `i18n.rs:1038-1042` "首版不提供启动通知" 诚实文案 |
| hide/clear-after-open 即时生效 | ✅ | `main.rs:3631-3649` (toggle 后 refresh main window); `settings_controller.rs:521-531` |
| hide_on_focus_loss 持久化 | ✅ | `settings.rs:318` + `settings_controller.rs:529-531` persist; native 失焦钩子为 M07 手工验收项 (任务清单 M06.2 明示) |
| monitor strategy Mouse/ActiveWindow 真实接线 | ✅ | `window_placement.rs:193-241` `placement_rect_for_active_window` 真实 GetWindowRect → monitor work area, 无效句柄 fallback cursor; `main.rs:115-132` place_window 读取 MONITOR_STRATEGY 并真实分派 |
| 语言 system/zh-CN/en-US + 显式 | ✅ | `settings.rs:87-94`; `main.rs:1665-1671` locale_for; `main.rs:684-695` SetLanguage 即时重本地化两个窗口 |
| restore-defaults 保留 folder (除非全重置) | ✅ | `settings_controller.rs:724-728` (仅设置快照→persist, 不动数据); `m06_tests.rs:225-260` 测试; 数据页 clear-all 独立 (下述) |

注意: `settings.rs:79-82` `MonitorStrategy::ActiveWindow` docstring 写着 "0.0.1 映射到同一 cursor fallback; 仅持久化 forward-compat" — 与实际代码 (真实 active-window 显示器布局) **不符**, 属过期注释 (见 M2)。

### M06.3 开机启动

| 项 | 结果 | 证据 |
|---|---|---|
| run_value_for 引号 (空格 + Unicode) | ✅ | `tray_open.rs:45-51`; 测试 `tray_open.rs:290-308` (含 Unicode 路径断言) |
| enable/disable 幂等 | ✅ | `tray_open.rs:44-90` (`RegSetKeyValueW` 写 / `NULL data` 删除; `ERROR_FILE_NOT_FOUND` 良性) |
| 状态读取 ↔ 菜单 check 同步 | ✅ | 启动 `main.rs:2845-2852` 读 HKCU 设 glyph; 托盘 toggle `2868-2874` 写后同步; 设置页 toggle `3604-3606` 同步 tray glyph |
| 失败→清除/不假成功 | ✅ | `main.rs:3595-3611`, `settings_controller.rs` launch_at_login_matches_os |
| silent 参数文档化 | ✅ | `settings.rs:348-351` 文档注释; 任务清单 M06.3 "silent_start 设置驱动启动可见性; Run 值=纯 exe 路径" |

### M06.4 搜索/外观/快捷键

| 项 | 结果 | 证据 |
|---|---|---|
| 全部字段开关即时生效 | ✅ | 每条 Slint SwitchRow/ComboBox → SCommand → `settings_controller.rs:543-623` persist → `main.rs:696-711` refresh_search_settings (重建 ViewModel) |
| aliases 开关真实接线 | ✅ | `search_entry.rs:84-95` (FieldKeys 用 `settings.search_aliases` 门控) — 不是死开关 |
| recent_sort_first 接入 tiebreak | ✅ | `search/mod.rs:78-84` tiebreak 传 settings; `scoring.rs:226-264` 于 pinned 后插入 last_opened_at 降序 (tier 内); 测试 `scoring.rs:437-475` |
| 外观: theme/size/width/font-scale 实际应用 | ✅ | `main.rs:668-737` SetTheme→ui_set_theme + SetFontScale→UiTheme.font_scale; `main.rs:91-137` place_window 应用 SEARCH_WINDOW_WIDTH; `apply_settings_window_width`; row-height→`app-window.slint:1397` compact-rows 绑定 |
| show-path / show-cat-tag | ✅ | `main.rs:712-719` 直接 set 到主窗口 |
| transparency/high-contrast = 诚实后续版本 disabled | ✅ | `app-window.slint:1632-1633` `SwitchRow { future: true }` — future=true 时不渲染 Switch 且显示 "后续版本" (`app-window.slint:881-905`); 无 `t-toggle` 接线; i18n "半透明窗口/后续版本" |
| 搜索历史无虚假功能 | ✅ | `app-window.slint:1577` history_future 文案; `main.rs:1175-1178` clear_recent 注释明示"搜索历史首版未保存" |
| 数字 clamp + 恢复默认 | ✅ | `settings.rs:269-294,455-480`; `settings_controller.rs:1210-1223` 测试 (clamp 100/1/2/150/80) |
| 快捷键录制/验证/冲突保持旧/暂停/恢复/清除/恢复默认 + 运行状态 + 匿名 last_error | ✅ | `settings_controller.rs:732-782` 录制状态机 + validate; `main.rs:1247-1288` record_hotkey_key (native set_hotkey; 冲突 keep-old + runtime Disabled + 错误); pause/resume/clear/restore `main.rs:3886-3929`; runtime 从 native machine 投影 `main.rs:909-928`; `window.set_s_hotkey_error` 只渲染匿名 `hotkey_error_text` (`main.rs:1715-1718`) |

### M06.5 数据

| 项 | 结果 | 证据 |
|---|---|---|
| data-location + 安全打开 | ✅ | `main.rs:833` 显示; `main.rs:951-960` open_data_dir (ShellExecuteExW 复用边界) |
| export: save dialog, 从不静默覆盖 | ✅ | `file_dialog.rs:36-69` `IFileSaveDialog` + `FOS_OVERWRITEPROMPT` (OS 级覆盖确认); `main.rs:964-994` export_bytes → `std::fs::write` |
| import: parse→validate→preview→apply all-or-nothing | ✅ | `import_export.rs:172-174,262-348`; `main.rs:999-1053` (parse 失败→无 mutant); `apply_import` 重新 validate union (`import_export.rs:344-346`) |
| overwrite/merge/skip 语义清晰 | ✅ | `import_export.rs:106-119,188-235,239-251`; 测试 `import_export.rs:448-540` |
| 自身 round-trip | ✅ | `import_export.rs:411-417` own_export_round_trips_through_parse |
| backup create/list/restore | ✅ | `backup.rs:64-101`; 测试 `backup.rs:171-225`; `main.rs:1141-1173` (create/list), `1105-1138` restore (set_data + save_at 原子) |
| clear-recent (无虚假搜索历史) | ✅ | `main.rs:1179-1206` (只清 open_count/last_opened_at; 无历史则 no-op); 文案明示 |
| reset-settings 与 delete-all-records 分离 | ✅ | 两个独立命令 (`on_s_command_confirm_reset` vs `on_s_command_confirm_clear_all`, `main.rs:4020-4031`); `settings_controller.rs:724-728` reset 仅设置; `m06_tests.rs:225-260` 测试 |
| clear-all 两步确认 + 数量 + 重申不删真实目录 | ✅ | `app-window.slint:1768-1774` (arm→confirm; 显示 `s-folder-count` 于行 1704; never-deletes 文案行 1774); `main.rs:1224-1242`; 金丝雀 `m06_tests.rs:166-210` (真实临时目录 + 嵌套文件 + Unicode 目录名存活) |
| destructive 失败保留旧状态 | ✅ | `settings_controller.rs:692-718` persist 双回滚; `m06_tests.rs:225-260`; canary 保留语义 |

### M06.6 关于

| 项 | 结果 | 证据 |
|---|---|---|
| version/arch/link/license/privacy | ✅ | `main.rs:893-897` (`version::display` / x86-64 / github 链接 / MIT / privacy 摘要) |
| 无在线检查/遥测 | ✅ | Cargo 无 network 依赖 (仅 repository URL); 全仓 grep 无 reqwest/ureq/telemetry |
| 不伪装签名 | ✅ | About 无签名宣称; 全仓无 signed/Signature 字样 (除注释) |

### M05 延期项

| 项 | 结果 | 证据 |
|---|---|---|
| category rename UI | ✅ | `app-window.slint:1463-1469` (行内 per-row rename); `main.rs:4034-4053`; `settings_controller.rs:786-813` (重复名拦截) |
| tag rename UI | ✅ | `app-window.slint:1506-1512`; `main.rs:4056-4076` |
| tag merge UI (两步) | ✅ | `app-window.slint:1514-1527` (arm→目标名编辑→确认); `settings_controller.rs:844-870`; `repository.rs:828-849` (引用原子迁移, 去重) |

### Adversarial items (见下节逐条裁决)

全部 **PASS/澄清**，除下述 findings。

## Findings (按严重级)

### Critical

无。

### High

**H1 — 托盘"设置"菜单项被禁用，设置窗口无可达的直接入口** (功能可用性 / M06.1 导航集成)
- **文件/行**: `ui/app-window.slint:2007-2011` (`MenuItem { title: @tr("Settings"); enabled: false; // M06 wires the settings window. }`)
- **问题**: 托盘菜单 "Settings" 项在 `.slint` 中硬编码 `enabled: false`，且该项在 **M06 前后均被禁用**（`git show 556d61e:ui/app-window.slint` 亦为 `enabled: false`；`on_open_settings` 回调两者皆有，但永不可达）。注释"// M06 wires the settings window"表明本应在 M06 启用，但未启用，也无任何运行时 enable 逻辑。设置窗口目前只能通过"托盘 > 添加文件夹"（`main.rs:3074-3081`）或搜索窗口右键记录 > 编辑（`main.rs:3203`）打开，没有直接的"打开设置"入口。
- **影响**: M06 的主要交付（设置 UI）在真实桌面上缺乏主入口。作为独立 review 的观察：这不是 M06 diff 引入的回归，但闭包 M06 时该项仍未启用，"M06 wires" 的承诺未兑现 —— 必须修复或显式记录为已知限制。
- **复现/证据**: MenuItem `enabled=false` 为静态编译值，base 与 head 一致；`main.rs` 无任何对 settings menu enabled 的赋值。
- **建议**: 移除 `enabled: false`（恢复默认启用）并保留 `tray.on_open_settings` 的 `show()`；属一行修改，需重新 CI + 本 reviewer 复审。

### Medium

**M1 — import overwrite/apply 不满足 CLAUDE.md "覆盖前备份" 的显式快照** (数据安全 / 可恢复性)
- **文件/行**: `main.rs:1058-1101` (apply_import) — 直接 `set_data`+`save_at`，没有在覆盖前自动创建 `backup-*.json` 快照。
- **问题**: CLAUDE.md §7 明确"覆盖前备份"。当前 overlay 模式的 import 执行前不会自动写一个用户可见备份；恢复点依赖用户先手动"创建备份"。（repository 的内部 `data.json.bak` 仍是 pre-write 的旧主文件副本，但那是最后一次保存的备份，不是"本次覆盖前"的语义快照。）
- **影响**: 若用户选择 Overwrite 导入一个被篡改/误edit 的文件并应用，覆盖前没有自动快照，恢复只能靠手动的内建备份。风险等级中等（导入本身 all-or-nothing、记录级、不触真实目录，故不是 Critical）。
- **证据**: `apply_import` 中无 `backup::create_backup` 调用；`create_backup` 仅在用户点击"创建备份"按钮 (`main.rs:1141-1158`) 且仅在 Data 页手动调用。
- **建议**: 在 `apply_import` 成功写入之前（或 set_data 之前）对当前文档 `create_backup` 一次；或在任务文档中显式记录"覆盖前备份=用户手动创建"的决策并取得 reviewer 接受。若接受延期，需 reviewer 明确同意并记入已知限制。

**M2 — `MonitorStrategy::ActiveWindow` docstring 与实际接线不符** (可维护性 / 误导)
- **文件/行**: `src/domain/settings.rs:79-82`
- **问题**: docstring 称 "0.0.1 maps this to the same cursor-monitor fallback; the strategy is persisted for forward-compat"，但代码 (`window_placement.rs:193-241`, `main.rs:115-128`) 真实实现了 active-window 显示器定位并有无效句柄 fallback。
- **影响**: 文档与行为矛盾；后续维护者可能误判该策略未实现而"清理"掉真实代码。
- **建议**: 更新 docstring 反映真实行为。

**M3 — Overwrite 导入可产生重复路径记录 (id 优先分支绕过路径去重)** (正确性边界)
- **文件/行**: `src/storage/import_export.rs:294-334` (apply_import), `classify` 于 `:188-235`
- **问题**: Overwrite 模式下，若导入记录 id 命中当前记录 A（→ `Updated`，按 id 替换），但其 path 同时与另一条当前记录 B 冲突，替换后文档会出现两条同 path 记录。`AppData::validate_and_normalize` (`document.rs:20-80`) 检查重复 id/tag/category 引用，但**不检查重复路径**，故该状态可通过验证并持久化。
- **影响**: 数据不变量 (M01.2 "不可通过字符串小写化判断路径等价"、仓库 `is_duplicate_path` 维护唯一路径) 在特定导入文件下被绕过。记录级问题，不触真实目录，不删数据；但会在"最近打开/搜索"与路径冲突语义中出现两条同路径记录。
- **复现/证据**: 构造 incoming: 记录1 `id==A.id, path==B.path`（A、B 为当前两条不同记录）。classify 记录1 → id_present → Updated；apply 按 id 替换 A → A 现在 path==B.path；两条同 path 记录并存。
- **建议**: 在 `apply_import` 的 union 验证中加入路径级去重/冲突检测（例如 Overwrite 模式下若 id 命中但 path 冲突另一条，按 path-collision 分支处理或拒绝 `UnresolvedReference`/新增 `DuplicatePath` 错误）。或在 `validate_and_normalize` 增加记录级 path 唯一性校验（需评审其与 M01.2 大小写/规范化语义的交互）。

### Low

**L1 — `launch_at_login_wired` 字段只写默认，从不参与逻辑** (死字段)
- **文件/行**: `src/domain/settings.rs:352-355,423` (定义/默认 false; 仅测试访问)
- **问题**: 该字段此前语义"launch-at-login 已接线标志"，M06 后 HKCU 状态由 `launch_at_login_os`/`set_launch_at_login_os` 实时跟踪，`launch_at_login_wired` 成为从不写、从不读的持久化字段。
- **影响**: 轻微 schema 噪音；无安全影响。保留以兼容旧文档可接受，但应注释为 legacy/未来使用。
- **建议**: 加注释说明其为 legacy forward-compat 字段；或若销毁则需保留 `#[serde(default)]` 读取兼容。

**L2 — 冗余/未使用的 SNotice 与回调** (维护性)
- **文件/行**: `settings_controller.rs:96` `ExportTargetExists` 从未产生（导出用 OS 覆盖提示，不会走到该枚举）；`app-window.slint:1224-1229` 的 `s-command-arm-clear-all` / `s-command-cancel-clear-all` / `s-command-arm-reset` / `s-command-cancel-reset` 声明于 slint 但从未被控件触发（两步确认的状态完全由 slint 本地属性 `s-confirm-*` 管理），main.rs 也未接线。
- **影响**: 维护噪音；`ExportTargetExists` 映射 (`main.rs:1625-1627`) 与 i18n 键为死代码。不影响行为。
- **建议**: 删除未用回调/枚举，或补齐接线并加测试。

### Info / 观察项

- **I1 — 设置窗口无 on_close_requested**：设置窗口关闭只 hide（无退出语义），符合托盘 app 设计；无数据丢失面。
- **I2 — settings 持久化延迟**：每次 toggle 立即 `save_at`（每次一个 revision + 原子写）。快速连续切换会在每次 toggle 触发磁盘写，但不会丢数据；符合"即时生效"要求。M07.3 "保存不阻塞输入"留待 M07 复核。
- **I3 — 持久化失败时的进程内不一致边缘**：`persist()` 失败会把 store 工作副本回滚到 `previous`。仓库 `save_locked` 在磁盘写成功后才 `self.document = Some(...)`，因此失败时工作副本保留的是 persist 设置的"半状态"——但 persist 已把 store 重新 `set_settings(previous)`。若磁盘写失败发生在 `replace_backup` 之后的 `rename` 阶段，磁盘上是旧数据、工作副本是 previous、视图是 previous，三者一致。**结论: 修复正确** (见 adversarial #10)。
- **I4 — 性能基准真实**：release 10k 基准在 CI 真实跑 (commit 35934630644)，数值真实；在 Debug / GNU 本地不会替代。
- **I5 — thread-safety**：M06 无新线程；所有状态下 UI 线程 + `Rc<RefCell>`；repository 通过 `data.json.lock` 跨进程串行写。M06 未引入死锁新面。

## Cross-cutting 检查

### 正确性
- Manager / settings 两个 controller 共享同一 `Rc<RefCell<DocumentRepository>>`，import/backup-restore/clear-all 后 `manager.reload_from_store()` 重新读取 (`manager.rs:667-672`, `main.rs:1097-1100`)。
- search/recent tie-break 确定性强 (`scoring.rs:226-264`)，无 wall-clock 参与（`None` last-opened 回退 open_count→id）。

### 错误处理与数据安全
- persist 双回滚正确（见 adversarial #10）；launch-at-login 写失败→不 persist→UI 回弹；
- import parse 失败 → 无 mutation；apply re-validate 作为第二道 all-or-nothing 门；
- 备份/导出/导入/clear-all **全部 record-level**，无一触达真实目录路径的删除 API（见 #1）。

### 隐私
- 日志 (`eprintln`) 全部为匿名固定文案 (`main.rs:1796,1801,2709,4126,...`)；错误枚举 Display 无路径 (`backup.rs:121-123` 有测试断言);
- hotkey last_error 只渲染匿名 `hotkey_error_text`。
- 数据目录路径仅在 UI 显示（用户自己的数据位置），不进日志。

### 安全
- 无网络/遥测/自动更新；
- HKCU per-user Run 值带引号，无注入面（值=应用自身 exe 路径）。

### Windows 行为
- 原生对话框 (IFileOpen/SaveDialog) 走 COM，CLSID 有测试锁定 (`file_dialog.rs:135-145`)；
- ActiveWindow 布局用真实 GetWindowRect + monitor work area + DPI，无效句柄回退 cursor；
- 无真实删除 API 面 (见下)。

### 测试覆盖
- m06_tests.rs (6): 真实文件 round-trip、set_data 替换、clear-all 金丝雀、noop、reset-vs-clear 分离、旧文档前向兼容解码；
- settings_controller.rs 测试 (17): round-trip、全开关持久化、clamp、失败回滚、hotkey 录制/无效/禁用、restore 保留数据、rename/merge、import preview、launch sync 等；
- import_export.rs (7)、backup.rs (3)、scoring recent_sort (新增)、tray run_value (新增)、window_placement fallback (新增)。
- 全部 CI 通过 (370+3)。

### 性能
- 无新热路径；recent tie-break 为 O(1) 比较插入；import plan O(n²)/O(n·m) 分类对 10k 规模可接受，且 apply 一次性进行。

### 可访问性
- 无新禁用项之外的可达性问题；future 占位符禁用并有标签（诚实）；未做键盘焦点走查（非自动化项）。

### 可维护性
- controller/view 解耦清晰，平台副作用在 adapter；i18n 两语种键 parity 测试 (`i18n.rs:1663-1700`)。
- 除 L1/L2 外无死代码面。

## 未能自动验证的 GUI/平台项 (手工验收清单)

1. **设置窗口真实打开路径**: 托盘 Settings 菜单被禁用 (H1) — 桌面验收时必须打开设置窗口验证。
2. **native 文件对话框 (save/open) 的 FOS_OVERWRITEPROMPT** 在真实 Explorer 上提示覆盖确认。
3. **hotkey 录制真实的 Ctrl+Alt+Space** 捕获、冲突提示、暂停/恢复、清除 (native machine 只能在桌面验证)。
4. **隐藏/清空 after-open、失焦隐藏** 在真实桌面上的行为 (native 失焦钩子为 M07.1/桌面项)。
5. **多显示器/混合 DPI** 下 ActiveWindow 与 Mouse 策略的窗口定位。
6. **系统语言切换后的 locale 即时更新** (中文 IME 与字体渲染)。
7. **导入预览 + 应用** 在真实文件对话框下的端到端 (含 overwrite/merge/skip 选择)。
8. **启动 (silent/show-main)** 在登录启动场景下的真实表现。
9. **数据目录打开** 的 ShellExecuteExW 在桌面的实际效果。

## 结论

**Verdict: `CHANGES_REQUESTED` (进入下一轮之前需修复 H1，评审 M1/M2/M3)**

- H1 (settings 菜单禁用) 必须在批准前修复 —— 它使 M06 主交付无法从主入口访问；
- M1 (覆盖前备份) 需实现"覆盖前自动快照"或由 reviewer 明确接受其延期理由并记录；
- M2/M3 为一处注释修正和一处路径不变量边界加固，建议本轮关闭；
- L1/L2 为清理项，可合并处理。

数据安全核心 (无真实目录删除、all-or-nothing 导入、原子持久化、失败保留旧状态、金丝雀测试) 经独立核对为**真实且有效**；没有发现 Critical 缺陷。

**注意**: 本里程碑层级的批准 ≠ release 批准；release 需另轮独立 release review + 手工验收。
