# M05 文件夹/分类/标签管理 — Review r01

- **版本**: 0.0.1
- **Topic**: m05-management
- **轮次**: r01
- **日期**: 2026-09-23
- **实现 agent**: (M05 实现轮) — 本 review 由独立 code-review agent 编写
- **Reviewer agent 声明**: 本 reviewer **未参与** M05 的实现（实现提交 49dbb77），仅做独立评审。评审期间未修改、未提交、未推送任何源代码，未运行 `cargo`。本文件为该轮 M05 评审的独立审计证据。

## 0. 被评审范围

| 项 | 值 |
|---|---|
| Base commit | `13a1aaa` (M04 windows review r02 approval) |
| Head commit | `49dbb77` (feat: add folder/category/tag management with safe remove) |
| 比较范围 | `git diff 13a1aaa..49dbb77` |
| 文件数 | 17 (src/domain/path_semantics.rs、src/presentation/{management,manager,commands,i18n,view_model,mod}.rs、src/storage/{repository,m05_tests,mod}.rs、src/domain/{settings,mod}.rs、src/platform/windows/{folder_picker,clipboard,mod}.rs、src/main.rs、ui/app-window.slint) |
| 新增/变化 | +6381 行 / -63 行 |

## 1. CI 证据

| Workflow | Run ID | Head | 结论 | 关键 job / 证据 |
|---|---|---|---|---|
| Windows CI | `35906554562` | `49dbb77a` | ✅ success | `fmt`、`clippy -- -D warnings`、`test`（lib 323 passed + bin 3 passed，1 ignored）、`release build`、EXE 打包、third-party licenses。全部在 `x86_64-pc-windows-msvc` |
| Search benchmark | `35906554541` | `49dbb77a` | ✅ success | `release 10k benchmark`：empty-query median=0.82ms；filtered-with-clone median=59.28ms；pinyin-heavy median=67.11ms；english-initials median=54.81ms；edit-distance median=62.60ms；multi-token median=51.47ms。MSVC target，Release 模式 |

**测试数核实**：CI 主 lib 测试 `running 324 tests` → `323 passed; 0 failed; 1 ignored`；bin `running 3 tests` → `3 passed`。合计 **326 passed**（327 个 `#[test]`，其中 1 个 ignored）。与实现 agent 声称的 “326 tests” 一致（323 库 + 3 bin，忽略 1 个）。判定：**与事实一致**。
（本机 `git grep` 静态统计为 327 处 `#[test]`，与 CI 的 326 通过 + 1 ignored 吻合。）

## 2. 需求映射（M05.1–5.5 + M01.2 + M03 右键补全）

图例：✅ PASS（已实现并有测试）/ ⚠️ PARTIAL（部分实现或仅逻辑）/ ❌ NOT-BOUND（未绑定 UI / 未接线）。

### 2.1 管理页（M05.1）

| 子项 | 状态 | 证据（file:line） |
|---|---|---|
| 导航（文件夹/分类/标签 三页） | ✅ | `ui/app-window.slint:697-709` 左边导航按钮；`manager.rs:671` ShowPage |
| 文件夹页 CRUD 列表 | ✅ | `ui/app-window.slint:707-732` FolderManagementRow；`manager.rs` refresh_rows |
| 筛选（名称） | ✅ | `app-window.slint:709-711`，`main.rs:1567` on_command_filter_name |
| 筛选（分类 / 启用状态） | ❌ NOT-BOUND | `management.rs:196-211` 逻辑实现，但 **UI 无分类筛选器/状态筛选器**，`command-filter-category`/`command-filter-enabled` 不存在 |
| 排序（名称 / 最近 / 添加） | ❌ NOT-BOUND | `management.rs:FolderSort`（逻辑完备），但 **UI 无排序选择器**，默认固定 Name |
| 单活动路径检查不后台扫描 | ✅ | `manager.rs:1240-1250` check_path 为手动单次 `std::fs::metadata`；`manager.rs:1537-1571` read_dir 一次性一层的受控清单。无后台扫描 |
| 重复定位（M01.2 语义） | ✅ | `repository.rs:duplicate_folders`、`repository.rs:is_duplicate_path`；`m05_tests.rs:439-486` |
| 无伪功能 | ❌ PARTIAL | 见 §3.2（若干逻辑项未绑定，但未出现“展示了不可用控件”的情形——见 Finding C3 说明） |

### 2.2 添加/编辑对话框（M05.2）

| 子项 | 状态 | 证据 |
|---|---|---|
| 选择器 + 粘贴 + 多拖拽 | ✅/❌ | 原生多选器 `folder_picker.rs:pick_folder_dialog` 已接线（main.rs browse）；粘贴 `clipboard.rs:read_text` 已接线；**OS 拖放未接线**（`folder_picker.rs` 文档注明的 seam；preview_batch 逻辑可用但无 native drop hook）→ 判定 **PARTIAL** |
| 逐项预览/校验/去重 | ✅ | `manager.rs` open_pick_paths → preview_batch；`management.rs` evaluate_candidate |
| ONE-LEVEL 导入：默认 OFF | ✅ | `settings.rs:one_level_import` + `#[serde(default)]`；settings.rs 测试 |
| ONE-LEVEL 导入：max 100 | ✅ | `management.rs:MAX_ONE_LEVEL_IMPORT=100`；`list_direct_children(…, limit)` 上限 |
| ONE-LEVEL 导入：hover 字面 “max 100” | ✅ | `management.rs:ONE_LEVEL_IMPORT_MAX_LITERAL="max 100"`；`manager.rs:835` hover 使用该常量 |
| ONE-LEVEL 导入：二次确认 | ✅ | `app-window.slint:866-877` `import-second-confirm` 展示；`ImportSecondConfirm` 文案 |
| ONE-LEVEL 导入：不递归 | ✅ | `manager.rs:list_direct_children` read_dir 一层 |
| 导入字段 alias/tag/note/weight/color | ❌ NOT-BOUND | 见 §2.6 / Finding C3 |
| 路径重新校验 | ✅ | `manager.rs:validate_draft`；sync_draft_view 每次编辑重算 |
| 离线取消 / 仍可保存 | ✅（逻辑） | offline UNC 仍 valid（`draft_editing_marks_unsaved_and_path_revalidates`） |
| 重复：取消/编辑既有/另存他名 | ❌ NOT-BOUND | `DuplicatePolicy` enum 存在但未接线 UI；`save_draft` 仅 default-block（DuplicateBlocked） |
| 未保存编辑提示 | ❌ NOT-BOUND | `FolderDraftView.unsaved` 存在（`manager.rs:201,889`），但仅测试覆盖，**无 UI 提示**（无 “unsaved changes” 警告） |

### 2.3 移除/撤销（M05.3）

| 子项 | 状态 | 证据 |
|---|---|---|
| “从 FileGo 移除” + 不改真实目录文案 | ✅ | `i18n.rs` remove_confirm/remove_never_deletes；“不会删除磁盘中的真实文件夹” |
| remove/disable/enable | ✅ | `repository.rs:remove_record / disable_record / enable_record` |
| 短时撤销跨保存边界确定性 | ✅ | `repository.rs:undo_remove_record`（仅当 previous+1==current）；`m05_tests.rs:253-314` 与 `manager.rs:undo_is_refused_after_an_intervening_save` |
| 无误触 Delete | ✅ | 移除入口为按钮（非键盘 Delete）→ `StartRemove`（立即移除 + 撤销横幅）。无 Delete 键误触面 |
| API: remove_record / disable_record | ✅ | 按 id 的记录级 API |
| canary：真实临时目录 remove/disable/clear-all 保留目录+内容 | ✅ | `m05_tests.rs:106-220` 用真实 `tempfile::TempDir` + fs 断言存活 |
| 撤销窗口 UNDO_WINDOW | ✅ | `management.rs:UNDO_WINDOW=8s` |

### 2.4 分类/标签（M05.4）

| 子项 | 状态 | 证据 |
|---|---|---|
| 分类 创建/重命名/删除/颜色 | ❌ PARTIAL | 创建/删除 UI 有（slint:771-778）；**重命名/颜色无 UI**；`confirm-delete-category` 未用 |
| ≤1 分类/文件夹 | ✅ | `FolderEntry.category_id: Option<...>` 单值；`document.rs` 校验 |
| 删除分类 → 未分类 | ✅ | `repository.rs:remove_category` 将 folder.category_id 置 None；`m05_tests.rs:321-342` |
| 标签 创建/重命名/合并/删除 + 用量计数 | ❌ PARTIAL | 创建/删除 UI 有（slint:793-806）；**重命名/合并无 UI**（`command-rename-tag`/`command-merge-tag` 未接线）；用量显示有（tags-usage） |
| 精确重复标签屏蔽 | ✅ | `manager.rs:create_tag` names_equal 拦截；管理测试 |
| 删除只清引用 | ✅ | `repository.rs:remove_tag`；m05_tests |
| 合并原子保存 | ✅ | `repository.rs:merge_tag` + 单次 save_at；`m05_tests.rs:368-406` |
| 名称比较/空白/大小写策略 | ✅ | `management.rs:names_equal`（trim + ASCII fold）；显示保留原名 |
| 显示保留原始 | ✅ | `FolderRow.display_name` 原样 |

### 2.5 右键上下文菜单（M05.5）

| 子项 | 状态 | 证据 |
|---|---|---|
| open / copy-path | ✅（M04 已存在） | `app-window.slint:283-285` command-open-selected / command-copy-path |
| copy-name / edit / pin / disable / remove-from-list | ❌ NOT-BOUND | `RowAction::{CopyName,Edit,TogglePin,ToggleEnable,RemoveFromList}` 与 `ExternalEffect`（view_model.rs:117-126）已加，`row_action` 映射完成（view_model.rs:502-520），`main.rs:242-283` effect 分支完成，但 **search 窗口 Slint 无任何可触发这些动作的右键菜单**（AppWindow 顶层仅 open/copy-path/overlay 回调，`app-window.slint:275-285`；ResultRow 仅 row-clicked/row-activated）。`request_context_action`/`ContextEffect`/`drain_context` 全部为死代码路径 |
| popup-safe | ✅（overlay gate） | `ViewState.overlay_open` 抑制失焦隐藏（view_model.rs:455,561） |
| clipboard unicode 往返 | ✅ | `clipboard.rs` write_unicode_text（已有）+ read_text 新加，UTF-16→String；含 1 MiB 上限与 NUL 扫描 |

### 2.6 M01.2 路径语义（path_semantics.rs）

| 规则 | 状态 | 测试 |
|---|---|---|
| 显示与比较键分离 | ✅ | `path_semantics.rs:1-30`（raw 保留，key 仅派生） |
| 非 naive 小写（仅 ASCII A-Z fold，`Ä≠ä`） | ✅ | `ascii_fold`；测试 `ascii_case_folding_is_not_unicode_case_folding` |
| 尾随分隔符规则 | ✅ | trailing_separator / drive_root_vs_drive_relative |
| 盘符大小写折叠 | ✅ | drive_letter_case_is_insensitive |
| dot-dot 截断（不越过根/UNC share） | ✅ | dot_dot_pops_components... |
| UNC server/share 边界 | ✅ | unc_server_and_share... |
| `\` 与 `/` 等价 | ✅ | forward_and_backslashes_are_equivalent |
| `\\?\` 前缀 | ✅ | extended_length_prefixes |
| 离线 ≠ 无效 | ✅ | offline_and_missing_paths_still_classify |
| 仅受控打开路径中 `%VAR%` 展开，M04 保持 raw | ✅ | path_semantics `expand_open_path` 注释；`tray_open.rs:142` ShellExecuteExW 直传 raw；测试 expansion_is_never_part_of_comparison_key |
| 不扫描 | ✅ | 无目录遍历 |

## 3. Findings（按严重级排序）

### Critical

**C1 — 设置页所有行动作的 id 经 slint `int` 传输（128 位 UUID 截断/零值碰撞）**
- 文件/行：`src/main.rs:627`（`r.id.as_uuid().as_u128() as i32`）→ `ui/app-window.slint:720`（`rows-ids[i]` int）→ `src/main.rs:1520/1533/1543/1553/1561`（`FolderId::from_uuid(Uuid::from_u128(id as u128))`）。
- 问题：UUID 的完整 128 位被强转为 32 位 int 进 Slint 再反解。低 32 位为 0 的 UUID（例如 `…-00000000` 结尾）会与 id 0 碰撞；截断本身使超过 32 位范围的身份无法被还原。随机 v4 UUID 碰撞概率低，但这里不是概率问题——它是**结构性错误映射**：目录列表定位用的是 `r.id.as_uuid().as_u128() as i32`，意味着任何高 32 位非零或低 32 位为 0 的记录在做 “编辑/移除/切换启用/切换置顶/检查” 时都会解出**错误或虚无的 FolderId**。
- 影响：编辑/移除按钮可能作用于错误记录，或 StartRemove(错误 id)（`remove_record` 对未找到返回 false，安全上不会错删别的记录；但 edit 可能打开错误的记录，toggle 可能改错记录，check 可能报告错路径可访问性）。移除是安全底线（操作的是错误 id 但绝不会触碰真实目录——仍符合绝对不变式，但数据正确性被破坏）。
- 复现：非验证可自动；人工可试：添加一个低 32 位为 0 的 UUID 记录（概率 2^-32，但演示路径可通过把 UUID 设成 `00000000-0000-0000-0000-000000000000` 之类的种子数据复现）。
- 建议：不要把 UUID 通过 `int` 传。slint 侧用行序号（index）回调，由 Rust 侧根据 index→FolderId 映射解析（category/tag 删除回调已采用此正确做法，见 `main.rs:1583-1596` 通过 index 取 `view.categories`）。行动作改成传递 index 而不是截断 id。

### High

**H1 — 右键上下文菜单（M05.5）的动作在 UI 中全部不可达（死代码）**
- 文件/行：`src/presentation/commands.rs:45-53`（新 RowAction）、`src/presentation/view_model.rs:502-520`（映射）、`src/main.rs:242-283`（effect 分支）vs `ui/app-window.slint:275-285`（AppWindow 仅有 open/copy-path/overlay 回调）+ `ui/app-window.slint:439-442`（ResultRow 仅 select/open）。
- 问题：M05.5 要求的 copy-name/edit/pin/disable/remove-from-list 在逻辑层全部就位（有枚举、有测试），但搜索窗口没有任何 UI 入口能触发 `RowAction::ContextMenu`，也没有任何按钮触发 `CopyName/Edit/TogglePin/ToggleEnable/RemoveFromList`。`request_context_action`（main.rs:276-291）、`ContextEffect`、`drain_context`（main.rs:542）与全部外部 effect 分支都是死代码。
- 影响：M05.5 整体未达 UI 完成度；“标记完成但 UI 不可达” —— 属于必须显式上报主 agent 的完成度缺口。
- 复现：找不到任何调用点（grep `RowAction::Edit` 等在非 view_model/commands 文件结果为 0）。
- 建议：新增右键菜单（或在行上提供按钮/键位）触发这些 RowAction，或将其从 “已实现” 移入 “M06/后续轮”。

**H2 — 添加/编辑对话框的多个字段在 UI 中未绑定（note/weight/color/tag/category/重复策略）**
- 文件/行：`ui/app-window.slint:641-664`（draft 只暴露 name/path/pin）+ `src/main.rs:641-690`（只 push display_name/path/note/color）+ `src/main.rs:1651-1677`（只接 name/path/toggle-pinned/set-enabled 回调）。
- 问题：`MCommand::{EditNote,SetDraftWeight,CycleDraftColor,SetDraftCategory,SetDraftTag}` 等虽在 manager 有 handler，但：
  - **note**：`set_draft_note` 有 push（main.rs:687），slint 有 `draft-note` property 与 `command-set-draft-note` 回调声明，但**对话框无 note 输入控件**，main.rs 也无 `on_command_set_draft_note` 接线 → 推入值显示不出来，用户无法编辑。
  - **weight/color**：MCommand handler 存在，但 slint 无控件；`draft-color` 被硬编码为固定蓝色（`main.rs:690`），无 CycleDraftColor 调用。
  - **tag/category**：draft 结构可带 tag_ids/category_id，但对话框无多选/单选控件。
  - **重复策略**：`DuplicatePolicy`（EditExisting / SaveAsDifferentName）纯声明未接线；`save_draft` 只做 default-block。
- 影响：实现 agent 自己所述 “逻辑已录入但 Slint 未全部绑定” 得到证实。这些是 **TRUE-M05 项未绑定 UI** 的完成度问题，不是“伪功能”（因为也没有展示不可用的控件，控件根本不存在）。
- 建议：在下轮实现中为对话框补齐这些字段控件与回调，或显式从 M05 范围摘除并记录为 M06/M5.x。

**H3 — 分类/标签 删除无确认步骤（confirm 字符串未使用）**
- 文件/行：`ui/app-window.slint:125-126,771-778,793-806`（category/tag 删除直接 Button→command-delete-category(index)）；`i18n.rs` 有 `confirm-delete-category`/`confirm-delete-tag` 键但无任何消费点。
- 问题：删除标签/分类是立即执行的，无确认；虽然不删真实目录（安全不变式不受影响），但对用户数据（分类/标签结构）是一次即时的、无撤销的变更。与 M05.3 移除的 “公告+撤销” 体验不一致。
- 影响：可用性/数据保护期望差距；非安全问题。
- 建议：绑定 `command-confirm-delete-category`/`tag` 的确认提示，或利用现有 confirm 字符串。

**H4 — `action_toggle_enabled` 按钮标签 i18n 映射错误**
- 文件/行：`src/main.rs:825`（`("action_toggle_enabled", Msg::ActionAdd)`）。
- 问题：启用/禁用切换按钮被映射到 “添加” 文案。zh: “添加”，en: “Add”。
- 影响：行上 “启用/禁用” 按钮显示为 “添加”，用户困惑；“无伪功能” 维度被打折扣。
- 建议：新增 `Msg::ActionToggleEnabled`，或复用 Enabled/Disabled 文案。

### Medium

**M1 — ONE-LEVEL 导入开关在 0.0.1 用户侧永远 OFF（无 UI 开启途径）**
- 文件/行：`settings.rs:218-245`（默认 false）、`main.rs:515-519/1363-1366`（只读该值）、全库无 setter。
- 问题：`one_level_import` 只在数据文件里存 + 逻辑层尊重，但没有任何设置页/开关能开启它。因此已实现的受控导入流程用户永远触发不到（`.view.one_level_import_setting` 恒为 false）。
- 影响：M05.2 的一项交付在运行时不可达（非扫描安全性不受影响——正因为默认 OFF 且无入口，绝对无扫描风险）。
- 建议：若 M05 目标包含可用的受控导入，需在设置页绑定开关（可放 M06 设置页，但 M05 验收需明确）；否则将其标记为“逻辑就绪、待设置页接线”。

**M2 — 分类筛选 / 启用筛选 / 排序选择器在 UI 未绑定**
- 文件/行：`management.rs:FolderFilter{FolderSort}`（逻辑完备）+ `ui/app-window.slint:709-711`（仅名称筛选）。
- 问题：filter.category/enabled 与 sort 有实现与测试，但 UI 无相应控件；列表固定按名称、无分类/状态过滤。
- 影响：M05.1 部分项 UI 未达。

**M3 — OS 拖拽添加未接线（M05.2 的多拖拽项）**
- 文件/行：`folder_picker.rs:28-39`（文档声明的 seam）+ `manager.rs:open_pick_paths`（逻辑已有）。
- 问题：Slint winit 后端丢弃 DroppedFile，所以没有 drop hook。preview_batch 可被未来 hook 调用，但当前无入口。
- 影响：多目录导入目前只能经原生多选器获得（选择器本身支持多选，可部分替代拖拽）。

**M4 — 存储守护测试（source_under_storage_and_domain…）未扫描 presentation 层**
- 文件/行：`src/storage/tests.rs:700-763`（只扫 src/storage/ 与 src/domain/）。
- 问题：本 review 人工确认 `src/presentation/{management,manager}.rs` 与 `src/main.rs` 中无任何 `remove_dir/remove_dir_all/delete_directory` 调用（仅 `metadata` 与 `read_dir`），因此当前安全性成立；但守护测试未覆盖这两个新大文件，未来若在 presentation 引入删除调用不会被自动拦住。
- 建议：扩大守护扫描根到 `src/presentation`（或至少在测试里 include_str 检查这两个文件）。

**M5 — `remove_file` 的语义文档与 guard 措辞**
- 文件/行：`src/storage/tests.rs:748` 允许 `remove_file` 仅限 storage/io.rs。
- 与原 M01 约束一致；无问题，仅记录（`repository.rs` 中无 remove_file）。

**M6 — category/tag 删除/重命名/合并 中“重命名与合并”在 UI 全部不可达**
- 文件/行：`main.rs:1583-1617` 仅接 delete-category/delete-tag；renaming/merge 命令（`on_command_rename_category` 等）不存在。
- 影响：M05.4 的 rename/merge 交付仅逻辑层。

**M7 — `import_max_hint` / `category_rename` / `tag_rename` / 分类状态筛选 等 i18n 键为死键**
- 文件/行：`main.rs:825-864`（`import_max_hint→Msg::ImportChildren`，`category_rename→CategoryCreate`，`tag_rename→TagCreate` 等）。
- 说明：这些映射把不同语义键映射到同一 `Msg`，且对应 `Msg` 无独立译文案 → 键存在但其文案不会与按钮语义区分。

### Low / 说明

**L1 — `ui/app-window.slint` 中 `command-set-enabled(bool)` 已声明但无绑定控件**
- `app-window.slint:649` 有回调声明、`main.rs:1673` 已接线 `on_command_set_enabled`，但对话框无启用开关控件 → 回调无触发面。（设计为给未来条目，记录以免误判已绑定。）

**L2 — `undo` 下拉横幅 “Cancel” 语义为“保持已移除”（丢弃撤销）**
- `manager.rs:cancel_remove` 注释明确：cancel 即 dismiss undo。合理，但文案上用 “取消” 可能让用户以为取消“移除”——建议文案区分（如 “放弃撤销”）。

**L3 — `open_manual` 与 `command-add` 路径**
- `main.rs:1500-1504` `on_command_add→OpenManual`（空路径手动输入）。选“添加”按钮时对话框路径为空、名称用空路径推导。提示文案 `field-not-empty` 对空列表与空名称复用。轻微。

## 4. 横切检查（Correctness / Error / Data safety / Privacy / Security / Windows / Tests / Perf / A11y / Maintainability）

- **Correctness**：管理核心逻辑（validation、duplicate、undo、merge）为纯函数 + 确定性，单元测试充分。条理性问题集中在 id 经 `int` 截断（C1）与 UI 绑定缺口（H1-H3）。
- **Error handling**：`RepositoryError` 匿名化（不暴露路径）；所有新 API 返回 Result/布尔；save_at 先编码再落盘（验证失败不触盘）。`undo_remove_record` 失败返回 `ConcurrentModification`，状态确定性丢弃。
- **Data safety（绝对不变式）**：**通过**。全仓 grep 确认 `remove_dir/remove_dir_all/delete_directory` 在全部 src 中为 0（除 guard 测试自身），`remove_file` 仅存在于 `storage/io.rs` 的临时文件清理与 WriteLock drop。`repository.rs:remove_record/remove_folder` 为纯内存 `retain`，未触碰存储路径。path_semantics 不做任何真实文件规范化。canary 测试（m05_tests.rs:106-220）用真实临时目录 + 文件 + 子目录证明 remove/disable/clear-all + save + reload 后目录与内容存活。`folder_picker` 仅读取选择，无删除。
- **Privacy**：`ManagerError`/`Notice` 匿名；日志不记路径；clipboard read 不记录内容；i18n 文案展示路径仅作为演示状态。
- **Security**：`%VAR%`/`~` 展开仅存在于受控 `expand_open_path`（relocate 预览），不写入记录、不进 ShellExecuteExW（M04 保持 raw）；无任何命令执行/RCE 面。
- **Windows 行为**：多选器 COM（SHCoCreateInstance/IFileOpenDialog）用法合理；clipboard UTF-16 往返一致；`read_text` 对空/非 Unicode 降级为空串。
- **Tests**：新增 57+ 个 `#[test]`（path_semantics 16、management 11、manager 18、m05_tests 12 及既有）覆盖正常/失败路径、undo 跨保存边界、canary 真实目录存活、merge 原子性、M01.2 语义矩阵。CI 326 passed。
- **Performance**：管理页每次 `refresh` 全量 clone document；列表规模=用户记录数，10k 基准显示可接受（见 §1）。无后台扫描。
- **A11y**：管理窗口按钮/文本使用标准 widget；未发现对比度/焦点回归（未做桌面实测，见 §6）。
- **Maintainability**：模块边界清晰（domain/presentation/platform 解耦）；`FolderDraft`/`MView`/`MCommand` 模式一致；slint 单文件合并有注释说明 slint-build 限制。代码量大（manager.rs 2123 行）但结构易读。

## 5. 未自动验证的 GUI/平台项（需人工桌面验收）

1. 原生文件夹多选器在真实 Windows 上的模态行为与多选返回。
2. 中文 IME 在“添加文件夹”路径输入框的组成行为。
3. 多显示器 / DPI 缩放下管理窗口布局（min 760×520）。
4. 分类/标签/文件夹列表滚动与大数量行渲染。
5. 撤销横幅 8 秒计时在真实 UI 上的倒计时/过期表现。
6. 托盘“添加文件夹”打开设置窗口 + 选择器的焦点顺序。
7. 从 FileGo 移除后资源管理器对应真实目录未被触碰（人工复核 canary 之外的日常路径）。

## 6. M05 完成度结论（TRUE-M05 中未 UI 绑定的项）

以下为 **确属 TRUE-M05、已进逻辑/状态、但当前 UI 未绑定** 的项，逐一列出（供主 agent 决定是否追加一轮再批准 M05）：

1. **M05.5 右键上下文菜单（copy-name/edit/pin/disable/remove-from-list）—— 全部不可达（H1）**。
2. **M05.2 添加对话框：note/weight/color/tag/category 字段 —— 未绑定控件（H2）**，且 **重复策略 EditExisting/SaveAsDifferentName 未接线**，**未保存编辑提示未绑定**。
3. **M05.4 分类/标签 重命名、标签合并 —— 无 UI**；**删除无确认步骤（H3）**。
4. **M05.1 分类/状态筛选、排序选择器 —— 无 UI**。
5. **M05.2 受控 ONE-LEVEL 导入 —— 逻辑就绪但用户侧无开启开关（M1）**。
6. **M05.2 OS 拖拽 —— slint seam 未接（M3）**。

**是否存在 “展示了但不可用的伪功能”？** 经核查：slint 中声明的 M05 控件均有其回调接线（如 `command-toggle-pin`→main 接线）；未绑定项以“控件根本不存在”为主，而非“有控件无功能”。唯二接近“有 UI 无成效”的：① `action-toggle-enabled` 按钮显示“添加”文案并在行上调用的是启用切换（H4，功能可用但文案误导）；② `command-set-enabled` 声明未接控件（L1，无控件）。因此**没有发现展示但不可用（伪功能）** 的新控件；主要问题是 **“标记完成但未达 UI 完成度”** 的缺口。

## 7. 结论

**Verdict: CHANGES_REQUESTED**

理由：
- **绝对安全不变式全部成立**：全仓无任何真实目录/文件删除可达路径；canary 用真实临时目录证明 remove/disable/clear-all/save/reload 后真实内容存活；list_direct_children 只读一层且默认 OFF。数据安全维度通过。
- 但存在 **1 Critical（C1：行 id 经 `int` 截断导致设置页行动作可能作用于错误记录）**，以及 **4 High（H1 右键菜单不可达、H2 添加对话框字段未绑定、H3 分类/标签删除无确认、H4 按钮文案错映射）**。Critical 未关闭禁止进入发布；High 亦需本里程碑处理或由 reviewer/主 agent 明确接受。
- **Milestone 批准评估**：实现 agent 的自述（“逻辑已录入但未全部绑定”）属实，且覆盖面比自述更广（排序选择器、分类/状态筛选、重命名/合并、删除确认、未保存提示均未绑定）。M05 是否值得在**本轮**追加一次实现轮补齐 UI，取决于主 agent 对 M05 验收边界的裁定；本 reviewer 建议**追加至少一轮**将 C1 与 H1/H2/H3 中最核心的绑定补齐后再批准，否则 M05 的 “用户可见完成度” 与任务描述不符。

**复审请求**：请主 agent 组织对本文档逐条 response；阻断项（C1、H1-H3）在修复后重新跑 MSVC CI 并交由独立 reviewer 复审。

## 8. 附录：被评审文件清单（head 49dbb77）

`src/domain/path_semantics.rs`（新）、`src/domain/mod.rs`、`src/domain/settings.rs`、`src/presentation/management.rs`（新）、`src/presentation/manager.rs`（新）、`src/presentation/commands.rs`、`src/presentation/i18n.rs`、`src/presentation/mod.rs`、`src/presentation/view_model.rs`、`src/storage/m05_tests.rs`（新）、`src/storage/mod.rs`、`src/storage/repository.rs`、`src/platform/windows/folder_picker.rs`（新）、`src/platform/windows/clipboard.rs`、`src/platform/windows/mod.rs`、`src/main.rs`、`ui/app-window.slint`
