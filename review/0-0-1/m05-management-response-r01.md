# M05 文件夹/分类/标签管理 — Response r01

- **版本**: 0.0.1
- **Topic**: m05-management
- **轮次**: r01（response）
- **日期**: 2026-09-21
- **实现 agent**: M05 实现轮（对应 review `m05-management-review-r01.md`）
- **被回应的 review**: `review/0-0-1/m05-management-review-r01.md`（verdict: CHANGES_REQUESTED）
- **评审 commit**: `49dbb77`（即 M05 实现提交）
- **修复后 commit**: `XXXXXXX`（本 response 关闭后的新提交，见文末）
- **本 agent 声明**: 本 agent 为 implementation agent，**未参与** 该轮 review 的撰写与批准；仅针对 reviewer 的 findings 逐条修改与回应。

---

## 0. 修复范围总览

本次修改覆盖以下文件（均为 M05 实现与该 response 之间的变更）：

| 文件 | 改动 |
|---|---|
| `src/presentation/manager.rs` | C1 索引→ID 解析助手 `folder_id_at`；M1 `SetOneLevelImport` 命令 + store 方法 + controller 持久化；H2 `ResolveDuplicate` 命令（编辑现有/另存为他名）；`FolderDraftView` 增加 `color`；相关回归测试（6 个新测试） |
| `src/presentation/management.rs` | （无改动，纯逻辑已存在） |
| `src/presentation/i18n.rs` | H2/H3 新增 14 个 Msg 变体与 zh/en 目录、ALL_KEYS |
| `src/storage/repository.rs` | `set_one_level_import` 持久化方法 |
| `src/storage/tests.rs` | M4：fs 删除守护扩展到 `src/presentation/` 与 `src/platform/` |
| `src/main.rs` | C1 五个文件夹行动作 handler 改为 索引→ID 解析（删除截断还原）；H1 `command-context-action` 接线 + rows-pinned/rows-enabled 推送 + 上下文菜单 i18n；H2 草稿字段推送（note/weight/color/category/tags/duplicate/unsaved/enabled）与各回调接线；H3 confirm-delete-category/tag 回调；M1 one-level-import 回调；M2 filter-category/filter-enabled/set-sort 回调；H4 toggle-enabled 文案修正；settings 本地化新增条目与 setter；slint 新属性推送 |
| `ui/app-window.slint` | H1 右键上下文菜单（PopupWindow + MenuRow + overlay 同步）；C1 删除 `rows-ids` 整型传输、文件夹行动作改传行索引；H2 添加/编辑对话框全字段（note/weight/color/category/tags/duplicate 确认/未保存提示/启用开关）；H3 分类/标签删除两步确认；M1 one-level-import 开关；M2 分类/状态筛选 + 排序选择器 |

---

## 1. Findings 逐条回应

### C1（Critical）—— 已接受（ACCEPTED）

**Finding**: 设置页行内 `id as i32` 截断 128 位 UUID，低 32 位为 0 或超过 32 位的记录在编辑/移除/切换/检查时会解析出错误或虚无的 FolderId。

**评估**: ACCEPTED。这是结构性错误映射，必须消除所有把 128 位 FolderId 经 Slint `int` 往返的路径。

**修复**:
1. `src/presentation/manager.rs` 新增纯助手 `folder_id_at(&[FolderRow], index) -> Option<FolderId>`：UI 传**行索引**，Rust 从当前 `view.rows` 快照解析 `index → FolderId`（与 category/tag 删除已采用的 index→id 模式一致）。
2. `ui/app-window.slint` 的 `FolderManagementRow` 调用全部改为 `command-edit(i)`/`command-remove(i)`/`command-toggle-enabled(i)`/`command-toggle-pin(i)`/`command-check(i)`（传行索引），并删除 `rows-ids: [int]` 属性。
3. `src/main.rs` 五个 handler 不再 `Uuid::from_u128(id as u128)`，而是 `folder_id_at(&view.rows, index)`；越界索引解析为 `None` → no-op（绝不作用于错误记录）。
4. 任何 Slint `int` 边界都不再承载 FolderId。

**回归测试**（`src/presentation/manager.rs` tests）:
- `folder_id_at_resolves_row_index_to_the_real_folder_id`：索引解析出完整 128 位 ID；越界索引 `None`。
- `index_resolution_never_truncates_a_128_bit_id`：构造低 32 位全零 UUID，证明 `u128 as i32 == 0` 截断碰撞，而 index 解析返回完整 ID（即被评审的“低 32 位=0 记录必须拿到正确记录”的直接回归）。

**确定性**：索引在单次 push 内稳定；解析为纯函数；无路径/无日志；无 `#[allow]`。

---

### H1（High）—— 已接受（ACCEPTED）

**Finding**: M05.5 右键菜单动作（copy-name/edit/pin/disable/remove-from-list）在 UI 不可达，`request_context_action`/`ContextEffect`/`drain_context` 为死代码。

**评估**: ACCEPTED。逻辑层已具备（RowAction → ExternalEffect → ContextEffect 路由），缺的是触发入口。

**修复**:
1. `ui/app-window.slint` 新增右键上下文菜单：
   - `ResultRow` 增加 `row-context-requested(int, length, length)` 回调；TouchArea `pointer-event` 在右键释放（`PointerEventButton.right && PointerEventKind.up`）时上报行索引 + 窗口绝对坐标。
   - 新增 `ContextMenuPopup`（PopupWindow 组件）承载 7 项（打开/复制路径/复制名称/编辑/置顶或取消置顶/禁用或恢复/从 FileGo 移除），行状态经 `menu-pinned`/`menu-enabled` 决定置顶/禁用文案。
   - AppWindow 把弹层锚定在点击坐标，`command-set-overlay(true)` 打开 overlay gate；`changed context-menu-open` 在弹层任何路径关闭时清除 overlay（outside click/Escape/项选择），保证窗口恢复正常（Enter 可打开、Esc 可隐藏）。
2. `src/main.rs` 新增 `on_command_context_action(index, action)`：先 `SelectIndex`，再按动作码派发 `RowAction::{Open,CopyPath,CopyName,Edit,TogglePin,ToggleEnable,RemoveFromList}`；后四项经既有 `apply_effect` → `request_context_action` → `ContextEffect` 路由到设置窗口 controller 的 `drain_context`（Edit/TogglePin/ToggleEnable/Remove 均已在主程序接线）。至此 review 标记的死代码路径全部变成可达入口。
3. 搜索窗口推送 `rows-pinned`/`rows-enabled`（搜索集仅含启用记录，enabled 恒 true；当从设置页禁用后记录不再出现于搜索，文案“禁用”语义正确）。
4. popup-safe：overlay gate 在弹层显示期间保持，失焦不触发隐藏。

**验证**：弹层编译通过；`closed-`逻辑由既有 ViewModel 测试覆盖（`context_menu_actions_emit_selected_row_effects_and_keep_overlay` 等）。OS 级真实右键与弹层位置需桌面人工验收（见 §5）。

---

### H2（High）—— 已接受（ACCEPTED）

**Finding**: 添加/编辑对话框多个字段未绑定：note 无控件、weight/color 无控件且颜色硬编码蓝、category/tag 无选择、DuplicatePolicy 未接线、未保存编辑无提示。

**评估**: ACCEPTED。

**修复**:
- **note**：`ui/app-window.slint` 对话框新增 `LineEdit`（text= draft-note）→ `command-set-draft-note`；`src/main.rs` 新增 `on_command_set_draft_note` → `MCommand::EditNote`；适配推入 `draft.note`。
- **weight**：对话框新增 `SpinBox(min=-100,max=100)` → `command-set-draft-weight` → `MCommand::SetDraftWeight`；适配推入 `draft.manual_weight`。
- **color**：`FolderDraftView` 新增 `color` 字段并由 `sync_draft_view` 携带 `draft.color`；适配用 `draft_color_to_slint` 推送实际颜色（不再是硬编码蓝）；对话框显示色块 + “颜色”按钮 → `command-cycle-draft-color` → `MCommand::CycleDraftColor`（循环选择调色板并持久化）。
- **category**：对话框 `ComboBox`（`draft-categories-model` = 未分类 + 各分类）`current-index <=> draft-category-index` → `command-set-draft-category` → Rust 解析索引→`Option<CategoryId>` → `MCommand::SetDraftCategory`。
- **tags**：对话框按 `draft-tags-model` 渲 CheckBox 行，`draft-tags-state` 驱动勾选，toggle → `command-toggle-draft-tag(i, on)` → Rust 解析索引→TagId → `MCommand::SetDraftTag`。
- **DuplicatePolicy**：当 `draft-duplicate` 为真时对话框显示三个决策按钮（取消/编辑现有/另存为他名）→ `command-resolve-duplicate(code)` → `MCommand::ResolveDuplicate(DuplicatePolicy)`；controller 新增 `resolve_duplicate`：`EditExisting` 打开拥有同路径的既有记录，`SaveAsDifferentName` 在当前名称下持久化为独立记录（均保持“绝不触碰真实目录”不变式），`Cancel` 维持默认阻止。`save_draft` 重构出 `commit_draft` 供两路共用。
- **unsaved 提示**：`draft-unsaved` 推入，对话框顶部显示“有未保存的修改”警告；关闭（取消）在未保存时两步确认（放弃修改 / 继续编辑）。

**回归测试**（`src/presentation/manager.rs`）:
- `duplicate_policy_edit_existing_opens_the_existing_record`
- `duplicate_policy_save_as_different_name_adds_an_independent_record`
- `duplicate_policy_cancel_blocks_without_change`

---

### H3（High）—— 已接受（ACCEPTED）

**Finding**: 分类/标签删除无确认步骤（`confirm-delete-category`/`confirm-delete-tag` 字符串未使用）。

**评估**: ACCEPTED。

**修复**:
- `ui/app-window.slint` 分类/标签行的删除按钮改为两步确认：首次点击 arm `confirm-category-index`/`confirm-tag-index` 为当前行，行内显示“确认删除分类？…”/“确认删除标签？…” + 取消；确认才调用 `command-confirm-delete-category(i)`/`command-confirm-delete-tag(i)`。
- `src/main.rs` 新增 `on_command_confirm_delete_category`/`on_command_confirm_delete_tag`（按索引→CategoryId/TagId 执行 `MCommand::DeleteCategory`/`DeleteTag`）。两条取消回调为 UI 自清（Rust 侧无副作用）。
- 删除语义保持不变：删分类 → 该分类下文件夹变未分类；删标签 → 仅清除引用。真实目录永不触碰（不变式未回归）。

**验证**：`repository` 既有语义测试（`category_delete_clears_references_and_keeps_folders`、`tag_delete_clears_only_associations_and_keeps_folders`）仍绿；UI 两步确认属桌面验收项。

---

### H4（High）—— 已接受（ACCEPTED）

**Finding**: 行内启用/禁用切换按钮被 i18n 映射为 “添加”（`main.rs:825` `("action_toggle_enabled", Msg::ActionAdd)`）。

**评估**: ACCEPTED。

**修复**:
- `ui/app-window.slint` `FolderManagementRow` 的启用/禁用按钮文案由行状态决定：`row-enabled ? context-menu-disable : context-menu-enable`（即“禁用”/“恢复”），不再读取 `action-toggle-enabled`。
- `src/main.rs` 中 `("action_toggle_enabled", Msg::ActionAdd)` 改为 `Msg::DisabledLabel` 并加注释（该静态属性已不再被按钮引用，作为兜底绝不显示“添加”）。

---

### M1（Medium）—— 已接受（ACCEPTED）

**Finding**: ONE-LEVEL 导入开关在 0.0.1 用户侧永远 OFF（无 UI 开启途径）。

**评估**: ACCEPTED。

**修复**:
- `src/storage/repository.rs` 新增 `set_one_level_import(bool)` 持久化（写入内存工作副本 settings.one_level_import，调用方 `save_at` 落盘）。
- `src/presentation/manager.rs` 新增 `MCommand::SetOneLevelImport(bool)` + `ManagementStore::set_one_level_import` + controller `set_one_level_import`：持久化、更新 `view.one_level_import_setting`；关闭时解散 pending import offer。
- `ui/app-window.slint` 文件夹页新增一行：`Switch`（`checked= one-level-import-on`）→ `command-set-one-level-import`，并带二次说明文案；默认 OFF。
- `src/main.rs` 新增 `on_command_set_one_level_import` → `MCommand::SetOneLevelImport`；适配推送 `view.one_level_import_setting`。
- 受控导入纯逻辑（`child_import_offer`、`list_direct_children` 一层、max 100、二次确认、不递归）保持不变。

**测试**: `one_level_import_toggle_persists_and_controls_the_offer`（默认 OFF → 打开 → 关闭，store/view 一致）。

---

### M2（Medium）—— 已接受（ACCEPTED）

**Finding**: 分类筛选 / 启用筛选 / 排序选择器在管理页 UI 未绑定。

**评估**: ACCEPTED。

**修复**:
- `ui/app-window.slint` 文件夹页顶栏新增三个 `ComboBox`：
  - 分类：`filter-category-model`（全部/未分类/各分类），`current-index <=> filter-category-index` → `command-filter-category(index)`。
  - 启用状态：全部/已启用/已禁用 → `command-filter-enabled(index)`。
  - 排序：按名称/按最近使用/按添加时间 → `command-set-sort(index)`。
- `src/main.rs` 三个 handler 按索引解析到既有 `FolderFilter{category,enabled}` / `FolderSort` 纯逻辑（`MCommand::SetFilterCategory/SetFilterEnabled/SetSort`）。
- 适配在 `sync_ui` 推送三组索引（由 `view.filter`/`view.sort` 反解），并在分类变化后重算，保证模型/索引一致。

**验证**: 纯逻辑既有测试（`management_rows_filter_and_sort`）仍绿；结合的 UI 行为属桌面验收项。

---

### M3（Medium）—— 记录（RECORDED，不改代码）

**Finding**: Slint 1.18 winit 不透传 OS `DroppedFile`，管理窗口拖拽添加不可达。

**评估**: RECORDED（按 reviewer 建议保留 seam、不伪造）。

**说明**:
- 不新增代码；`folder_picker.rs` 的文档化 seam 继续保留（`preview_batch` 可被未来拖拽 hook 调用）。
- 工作入口保持不变并确认可用：原生多选器（`IFileOpenDialog` 多选）、剪贴板粘贴（`paste_into_path`）、手工路径输入。三者构成 0.0.1 的添加链路。
- 该限制列入前向迁移项，不应阻止 M05 批准。

---

### M4（Medium）—— 已接受（ACCEPTED）

**Finding**: fs 删除守护测试未扫描 `src/presentation/` 与 `src/platform/`。

**评估**: ACCEPTED。

**修复**: `src/storage/tests.rs` 的 `source_under_storage_and_domain_has_no_fs_delete_api_calls` 扫描根扩展到 `src/storage`、`src/domain`、`src/presentation`、`src/platform` 四个目录：
- `remove_dir`/`remove_dir_all`/`delete_directory` 不得在任何实现源出现；
- `remove_file` 仅允许 `src/storage/io.rs`（临时文件清理 + WriteLock drop），repository 与其余任何目录禁止。
- 测试文件自身（`tests.rs`）豁免（断言内含这些 token）。
- 守护把 reviewer 的人工核验（`management.rs`/`manager.rs`/`folder_picker.rs` 无删除 API）升级为结构性约束，CI 自动拦截未来回归。

**验证**: 扩展后的守护测试通过；全仓 grep 确认实现源中 `remove_dir*`/`delete_directory` 为 0。

---

### M5（Medium，说明）—— 记录（RECORDED）

Review 说明项：`remove_file` 仅限 storage/io.rs 的语义与 guard 措辞。本次未改动该边界；扩展后的 M4 守护仍保持 `remove_file` 仅限 io.rs。

---

### M6（Medium）—— 部分接受（PARTIALLY_ACCEPTED，重命名/合并 UI 留 M06）

**Finding**: 分类/标签“重命名与合并”在 UI 全部不可达。

**评估**: PARTIALLY_ACCEPTED。本次修复了其中与本轮 High 直接相关、成本低且与 H3 合并实施最自然的部分（删除确认 H3）。分类/标签的**重命名与标签合并**仍无 UI 入口（`on_command_rename_category` 等从未接线）。

**理由**: H2/H3 已把添加/编辑/删除 UI 补齐；重命名/合并属于二级编辑面，若在本轮追加会扩大 diff 与回归面。为保持 response 诚实：此项明确记录下来，作为 **M06 范围**（设置页重命名/合并 UI）。纯逻辑已存在且有测试（`RenameCategory`/`RenameTag`/`MergeTag`/`merge_tag` 原子性），风险集中在 UI 接线而非数据语义。请 reviewer 裁定是否在本轮接受该范围调整；如需本轮补齐，授权后可在下一轮快速追加。

---

### M7（Medium，说明）—— 记录（RECORDED）

**Finding**: `import-max-hint`/`category-rename`/`tag-rename`/分类状态筛选等 i18n 键为死键（映射到同一 Msg 且无独立译文）。

**说明**:
- `import-max-hint`：UI 的导入邀约 hover 直接用 `ONE_LEVEL_IMPORT_MAX_LITERAL`（“max 100”），不走该 i18n 键；`import-max-hint` 属性当前无消费者。
- `category-rename`/`tag-rename`：本轮未新增重命名 UI（见 M6），对应 `UiStrings.category-rename`/`tag-rename` 仍只被“新建”文案填充；待 M06 重命名 UI 接入后即可消费。
- 分类状态筛选已在本轮 M2 接入（`FilterStatusAll/Enabled/Disabled` 独立译文）；`SortName/Recent/Added` 亦已独立。
- 非数据/非安全问题，记录为低优先清理项。

---

### L1（Low）—— 已修复（FIXED）

**Finding**: `command-set-enabled(bool)` 已声明但无绑定控件。

**修复**: H2 对话框中新增“已启用”`CheckBox`（`draft-enabled` push + `command-set-enabled` 接线），关闭该死回调。

---

### L2（Low）—— 记录（RECORDED）

**Finding**: undo 下拉横幅 “Cancel” 语义为“保持已移除”（丢弃撤销）。

**说明**: 属可用性文案建议，非逻辑缺陷；`manager.rs:cancel_remove` 的注释已明确“cancel = dismiss undo”。文案区分可放入后续文案调整轮；不阻塞本轮批准。为保持行为一致，未擅自改动文案语义。

---

### L3（Low）—— 记录（RECORDED）

**Finding**: `open_manual` 与 `command-add` 路径：空路径手动输入时名称用空路径推导，`field-not-empty` 文案复用。

**说明**: 轻微；空路径会在对话框校验并提示“路径无效”，不会保存空记录。记录为低优先打磨项。

---

## 2. 未自动验证的 GUI/平台项（需桌面人工验收）

1. 右键上下文菜单在真实 Windows 上的弹出位置、项点击、outside/ Esc 关闭后的 overlay 复位。
2. H2 对话框各字段（SpinBox 聚焦、CheckBox 行、颜色循环色块、分类 ComboBox）在真实 UI 的交互。
3. H3 两步删除确认的可见反馈。
4. M1 one-level-import 开关与受控导入的二次确认流程。
5. M2 筛选/排序选择器在列表变化后的模型/索引一致性。
6. 中文 IME 在名称/路径/备注输入框的组成行为。
7. 多显示器 / DPI 缩放下设置窗口布局。

## 3. 未解决事项

- OS 拖拽添加（M3，RECORDED —— Slint 1.18 winit 不透传 DroppedFile，保留 seam）。
- 分类/标签重命名与标签合并 UI（M6，M06 范围）。
- 死 i18n 键清理（M7，低优先）。
- L2 文案区分、L3 空路径打磨（低优先）。

## 4. GNU 验证证据（快速反馈；非 MSVC 权威）

本地 GNU 工具链：`rustc 1.92.0`、`cargo 1.92.0`、target `x86_64-pc-windows-gnu`，rustfmt/clippy 均 installed（预检通过）。
按 `RUSTUP_AUTO_INSTALL=0` + 固定 toolchain 执行；因本机兼容问题使用了已记录的 workaround（`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` 并将 `/d/mingw64/bin` 加入 PATH，规避 winresource/windres 查找问题）。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | ✅ |
| `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` | ✅ |
| `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` | ✅ 329 passed（lib）+ 3 passed（bin），1 ignored |
| `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` | ✅ |

> 新增 6 个测试（C1 ×2、H2 ×3、M1 ×1），lib 测试数由 323 → 329。

GNU 通过仅为快速反馈；**发布/权威验证以 MSVC CI 为准**（见下）。

## 5. CI 证据（MSVC 门禁 —— 待推送后补充）

当前提交尚未推送（implementation agent 需推送授权后触发 CI）。推送后补充：

- run ID / URL：
- head SHA：
- 关键 job（fmt / clippy / test / release / EXE 打包 / third-party licenses）结论：

## 6. 请求复审

请独立 reviewer 对以下内容进行 r02 复审：
1. **C1（必须关闭的 Critical）**：index→FolderId 解析 + 回归测试，确认不再有任何 FolderId 经 Slint `int` 截断往返。
2. H1 右键上下文菜单（触发、overlay 复位、action 路由）。
3. H2/H3/H4 的 UI 绑定与文案。
4. M1/M2/M4 的接线与守护扩展。
5. 已拒绝/部分接受项（M6 PARTIAL、L2/L3/M7 RECORDED）的理由是否可接受。

**Response 结论**: 本 response 为核心阻断项（C1、H1-H4）与 M1/M2/M4 提供 ACCEPTED 闭环；M3 RECORDED；Lows 已处理或逐条记录。请 reviewer 复核后给出 r02 结论。
