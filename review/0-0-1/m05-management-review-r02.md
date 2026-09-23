# M05 文件夹/分类/标签管理 — Review r02

- **版本**: 0.0.1
- **Topic**: m05-management
- **轮次**: r02（对 r01 response/修复的复审）
- **日期**: 2026-09-24
- **Reviewer agent 声明**: 本 reviewer **未参与** M05 的实现与该轮 response 的编写（实现提交 49dbb77，修复提交 bb8d0c7，response 文档由 implementation agent 撰写）。评审期间**未修改、未提交、未推送任何源代码**，**未运行 `cargo`**（仅使用 `git show`/`git grep`/`git diff` 只读检查与 `gh.exe` CI 证据查询）。本文件为该轮 M05 复审的独立审计证据。
- **被复审的 response**: `review/0-0-1/m05-management-response-r01.md`（对 review-r01 verdict CHANGES_REQUESTED 的逐条回应）
- **复审结论正文**: `APPROVED_FOR_MILESTONE`（非 release 批准，release 需最终独立 release review）

## 0. 被复审范围

| 项 | 值 |
|---|---|
| Base commit | `49dbb77`（M05 实现提交，r01 被评审 commit） |
| Code head commit | `bb8d0c7`（`fix: close M05 review C1/H1/H2/H3/H4 and M1/M2/M4`，代码+response 文档） |
| Doc head commit | `c1d5f1d`（`docs: fill M05 response commit SHA and CI run IDs`，纯文档，仅改 response 文档 9 增 6 删） |
| 比较范围 | `git diff 49dbb77 bb8d0c7`（代码）+ `git diff bb8d0c7 c1d5f1d`（文档） |
| bb8d0c7 改动文件 | 7：`src/main.rs`、`src/presentation/i18n.rs`、`src/presentation/manager.rs`、`src/storage/repository.rs`、`src/storage/tests.rs`、`ui/app-window.slint`、`review/0-0-1/m05-management-response-r01.md` |
| c1d5f1d 改动文件 | 1：`review/0-0-1/m05-management-response-r01.md` |
| 核心不变项 | `src/storage/m05_tests.rs`、`src/domain/path_semantics.rs`、`src/presentation/management.rs`、`src/presentation/view_model.rs` 在 49dbb77..bb8d0c7 之间 **0 行 diff**（字节级未变） |
| 新增行 | `+1464 / -58`（bb8d0c7，含 response 文档）；纯代码约 +1400 |

## 1. CI 证据（MSVC 门禁，与 CLAUDE.md §3.2/§3.4 一致）

| Workflow | Run ID / URL | Head | 结论 | 关键 job / 证据 |
|---|---|---|---|---|
| Windows CI | `35921184606` | `bb8d0c7` | ✅ success | `fmt, clippy, test, release, package` 单 job：`cargo fmt --all -- --check` ✅；`cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-msvc -- -D warnings` ✅；`cargo test --workspace ... --target x86_64-pc-windows-msvc` ✅；`cargo build --workspace ... --release --locked ...` ✅；EXE 打包 + artifact 上传（FileGo-0.0.1-windows-x86_64-bb8d0c70...zip, 7.99 MB）+ `cargo deny check advisories licenses` ✅ |
| Search benchmark | `35921184624` | `bb8d0c7` | ✅ success | `release 10k benchmark` job，MSVC target |

**测试数核实（本轮）**：CI 日志 `running 330 tests` → `test result: ok. 329 passed; 0 failed; 1 ignored`（lib）+ `running 3 tests` → `3 passed`（bin）。合计 **332 passed（329 lib + 3 bin），1 ignored**。与 r01 的 326 passed（323 lib + 3 bin）相比，**新增 6 个测试**：`folder_id_at_resolves_row_index_to_the_real_folder_id`、`index_resolution_never_truncates_a_128_bit_id`（C1 ×2）、`duplicate_policy_edit_existing_opens_the_existing_record` / `duplicate_policy_save_as_different_name_adds_an_independent_record` / `duplicate_policy_cancel_blocks_without_change`（H2 ×3）、`one_level_import_toggle_persists_and_controls_the_offer`（M1 ×1）。与 response §4 声称的 “329 lib passed”、“新增 6 个测试” 完全一致。**判定：与事实一致。**

**doc-only 门禁验证**：`c1d5f1d`（纯 response 文档补填）后分支 run 列表确认**未触发任何新 run**（最近 12 个 run 最高 head 为 bb8d0c7），paths-ignore 门禁生效。记录之。

**MSVC target 一致性**：CI 日志确认 `TARGET: x86_64-pc-windows-msvc`，toolchain `1.92.0-x86_64-pc-windows-msvc`（rust-toolchain.toml 固定），fmt/clippy/test/release/deny 全部在同一 MSVC target 下执行。

## 2. Findings 逐条复审（r01 → r02 处置）

图例：**CLOSED**（修复核实通过）/ **RECORDED**（记录接受，不改代码或延期）/ **DEFERRED**（明确延期并记录原因）。

### C1（Critical）— **CLOSED**

- **核实方式**：`git grep` 确认 `rows-ids: [int]` 属性在 `ui/app-window.slint` 中**已删除**（grep 零命中）。`folder_id_at` 定义于 `src/presentation/manager.rs:598-604`：`pub fn folder_id_at(rows: &[FolderRow], index: usize) -> Option<FolderId> { rows.get(index).map(|row| row.id) }` —— 索引解析全量 128 位 id，越界返回 `None`。
- **5 个文件夹行动作**：`src/main.rs` 的 `on_command_edit/-remove/-toggle_enabled/-toggle_pin/-check`（1728/1745/1759/1773/1785 处）全部 `folder_id_at(&s.manager.view().rows, index as usize)`，`if let Some(...)` 才分发，越界 no-op。slint 侧 `FolderManagementRow` 的 `m-id` 传入 `i`（行索引，slint:976），回调 `command-edit(id)` 等把索引传回 Rust。**128 位 UUID 不再经 Slint `int` 往返。**
- **残留截断路径检查**：`git grep 'as i32|as_u128|from_u128'` 全仓确认——所有残余 `as i32` 均为计数/索引/尺寸（rows_count、categories_count、usage、index 等），**无任何 FolderId 从 u128 强转 int**；`as_u128() as i32` 只出现在回归测试 `index_resolution_never_truncates_a_128_bit_id` 内（故意断言 `low32_zero.as_uuid().as_u128() as i32 == 0` 以固定该截断碰撞必须由索引解析规避）。
- **回归测试存在性**：两个 C1 测试均在 `src/presentation/manager.rs` tests 尾部，读码确认：`folder_id_at_resolves_row_index_to_the_real_folder_id`（索引解析出完整 id；越界 `None`）+ `index_resolution_never_truncates_a_128_bit_id`（低 32 位全零 UUID `0x1234_5678_9ABC_DEF0_1234_5678_0000_0000`，断言 `as i32 == 0` 碰撞，而 `folder_id_at(&[row], 0) == Some(low32_zero)` 返回完整 id）。
- **结论**：结构性错误映射已消除，Option 语义为“越界 no-op 绝不错记录”。C1 **CLOSED**。

### H1（High）— **CLOSED**

- `.slint`：`ResultRow` 新增 `row-context-requested(int, length, length)` 回调，`TouchArea.pointer-event` 在 `PointerEventButton.right && PointerEventKind.up` 时上报（slint:211-216）；新增 `ContextMenuPopup`（PopupWindow）含 **7 项**：打开/复制路径/复制名称/编辑/置顶·取消置顶（`menu-pinned?`）/禁用·恢复（`menu-enabled?`）/从 FileGo 移除（slint:317-380）。`close-policy: close-on-click-outside`。
- AppWindow：`row-context-requested(index,x,y)` handler 内 `command-select-index(index)` + `command-set-overlay(true)` + 锚定 `ctx-x/ctx-y` + `context-menu.show()`（slint:602-613）；`context-menu` 组件 `item-clicked(index)` 时 `close()` + `command-set-overlay(false)` + `command-context-action(root.ctx-menu, index)`（slint:699-712）。`ctx-menu-open`（`context-menu.is-open` 别名）`changed` 在弹层任何路径关闭（item 选择/outside 点击/Escape）时 `command-set-overlay(false)` 复位 overlay（slint:416-424）。
- `src/main.rs`：`on_command_context_action(index, action)` 先 `ViewCommand::SelectIndex` 再按 action 码分发 `RowAction::{Open,CopyPath,CopyName,Edit,TogglePin,ToggleEnable,RemoveFromList}`；Edit/Pin/Enable/Remove 经 `apply_effect` → `request_context_action` → `ContextEffect`（channel 发送）→ settings 侧 50ms 定时器 `drain_context`（main.rs:2120-2130）→ `EditFolder/TogglePin/ToggleEnable/StartRemove`。`drain_context`（main.rs:563-594）携带完整 FolderId（`row.entry_id` UUID）。
- **死代码复活确认**：`request_context_action`（main.rs:280-292）、`ContextEffect`（main.rs:495-508）、`drain_context` 现在全部有真实触发面（右键菜单 → SelectIndex → RowAction）。`ContextMenu` RowAction 在 `apply_effect` 中不再出现为无效分支的悬空路径。
- 搜索侧推送 `rows-pinned`/`rows-enabled`（main.rs:398-418，enabled 恒 true 因搜索集仅含启用记录——语义正确）。
- ViewModel 回归测试 `context_menu_actions_emit_selected_row_effects_and_keep_overlay` 存在（view_model.rs:906-946，逐 action 断言 effect 映射且 overlay 保持）。
- **结论**：M05.5 全部 7 项动作现均可达且路由正确；overlay 复位逻辑健全。H1 **CLOSED**（OS 级真实右击弹出位置/outside/Escape 行为属桌面人工验收，见 §6）。

### H2（High）— **CLOSED**

逐一核对“每字段有 widget 且有 command handler”：

| 字段 | Widget（slint，均真实渲染） | Command handler（main.rs） | MCommand 分发 |
|---|---|---|---|
| note | LineEdit `draft-note` + `edited`（1130-1131） | `on_command_set_draft_note`（2000） | `EditNote` |
| weight | SpinBox `-100..100`, `draft-weight`（1146-1147） | `on_command_set_draft_weight`（2008） | `SetDraftWeight` |
| color | 色块 Rectangle `draft-color` + 颜色按钮（1155-1158） | `on_command_cycle_draft_color`（2016） | `CycleDraftColor` |
| category | ComboBox `draft-categories-model` + `draft-category-index`（1138-1140） | `on_command_set_draft_category`（2022，索引→`Option<CategoryId>`） | `SetDraftCategory` |
| tags | CheckBox 逐行 `draft-tags-model/state`（1160-1166） | `on_command_toggle_draft_tag`（2042，索引→TagId） | `SetDraftTag` |
| enabled | CheckBox `draft-enabled`（1170-1176）→ 关闭 L1 | `on_command_set_enabled`（1991） | `SetDraftEnabled` |
| duplicate | 三按钮 取消/编辑现有/另存他名（1182-1186） | `on_command_resolve_duplicate`（2058，0/1/2→policy） | `ResolveDuplicate` |
| unsaved | `draft-unsaved` 顶部警告条（1097-1100）+ 两步 Cancel（1192-1195） | `draft-confirm-cancel` UI 内自管理 | — |

- **color 非硬编码**：`cycle_draft_color`（manager.rs:1031-1052）为 8 色 PALETTE 循环（真实调色板，`None`=无色）；`draft_color_to_slint`（main.rs:814-821）把 `FolderColor` 映射为真实 ARGB —— `Some(color) => from_argb_encoded(color.0)`，仅 `None` 时回退默认蓝色。**方框色块与持久化值一致**（非固定蓝）。
- **DuplicatePolicy 接线**：`resolve_duplicate`（manager.rs:1077-1115）三种 policy 均实现：`Cancel`→block、`EditExisting`→`open_edit` 同路径既有记录、`SaveAsDifferentName`→同路径按当前独立名称落记录；`save_draft` 默认仍 block。三个回归测试 `duplicate_policy_*` 读码确认存在且语义正确（edit_existing 打开 id=10 "Documents" 且记录数不变；save_as 生成独立记录 folders=2；cancel 保持 1）。
- **i18n**：新增 14 个 Msg 变体（FieldNote/Weight/Category/Tags/Color、DuplicateCancel/EditExisting/SaveDifferent、UnsavedChanges/DiscardChanges/DiscardNo、ConfirmDeleteCategory/ConfirmDeleteTag）全部有 zh/en 目录与 ALL_KEYS。
- **结论**：H2 的“逻辑已录入但未全部绑定”缺口全部补齐，字段控件真实存在且有 handler。H2 **CLOSED**。

### H3（High）— **CLOSED**

- slint 分类行（1039-1044）：首击 `category-delete` 把 `confirm-category-index = i` arm 当前行，行内改显 “确认删除分类？分类下的文件夹将变为未分类”（`confirm-delete-category`）＋“取消”；确认才回调 `command-confirm-delete-category(i)`。标签行对称（1073-1078，文案 “确认删除标签？文件夹的该标签引用将被移除”）。
- main.rs（1900-1918 / 1924-1942）：`on_command_confirm_delete_category/tag` 按索引→`CategoryId/TagId` → `MCommand::DeleteCategory/DeleteTag`；索引越界 `None` 不动作。
- **语义保持**：删除仍走 `repository.remove_category`（文件夹置未分类）/ `remove_tag`（仅清引用），复用 M05.3 不变式（真实目录永不触碰）。既有语义测试 `category_delete_clears_references_and_keeps_folders`、`tag_delete_clears_only_associations_and_keeps_folders` 未改仍绿（在未改动的 m05_tests.rs 中）。
- **结论**：H3 **CLOSED**。

### H4（High）— **CLOSED**

- slint 行按钮文案（763）：`root.m-enabled ? UiStrings.context-menu-disable : UiStrings.context-menu-enable`，即当前启用 → “禁用”，当前禁用 → “恢复”，**绝不显示 “添加”**。列状态文本 `enabled-label/disabled-label`（785）。
- main.rs（918-919）：`("action_toggle_enabled", Msg::DisabledLabel)`（由 `ActionAdd` 改为 `DisabledLabel`），注释说明该静态已不挂按钮仅为兜底。
- 新增 Msg `DisabledLabel`/`EnabledLabel`/`PinnedLabel` 均有 zh/en。
- **结论**：H4 **CLOSED**。

### M1（Medium）— **CLOSED**

- `settings.rs`：`one_level_import: bool` `#[serde(default)]`，`Default` 为 `false`（settings.rs:215-218,245），前向兼容测试存在。
- `repository.rs:655-666`：`set_one_level_import` 写入内存工作副本 settings 并 `save_at` 落盘。
- `manager.rs`：`MCommand::SetOneLevelImport(bool)`（282）、`set_one_level_import`（479-483 接口 / 1640-1656 实现：persist→save→更新 view；**关闭时 `dismiss_import()`**）。
- `ui/app-window.slint`：文件夹页 Switch `one-level-import-on`（966-967）→ `command-set-one-level-import`，默认 OFF，带二次说明文案。
- main.rs（2070-2076）：`on_command_set_one_level_import` → `MCommand::SetOneLevelImport`；initial `window.set_one_level_import_on(view.one_level_import_setting)`。
- **无后台扫描**：`maybe_offer_import`（manager.rs:868-880）在 `!one_level_import_setting` 时直接 `open_single_draft` 不列目录；开启后 `list_direct_children_count` 仅一次性一层、有上限。测试 `one_level_import_toggle_persists_and_controls_the_offer` 覆盖默认 OFF→ON→OFF 三态。
- **结论**：M1 **CLOSED**（受控导入现在有真实用户入口）。

### M2（Medium）— **CLOSED**

- slint 文件夹页顶栏 3 个 ComboBox（930-951）：
  - 分类：`filter-category-model` + `current-index <=> filter-category-index` → `command-filter-category`
  - 启用状态：`filter-status-all/enabled/disabled` → `command-filter-enabled`
  - 排序：`sort-name/recent/added` → `command-set-sort`
- main.rs（1806-1854）：三 handler 按索引解析到既有纯逻辑 `FolderFilter{category,enabled}`/`FolderSort`（category index 0=全部/1=未分类/2+k=分类 k；status 1=已启用/2=已禁用；sort 1=最近/2=添加/默认=名称）。
- 适配同步（main.rs:713,775）：`set_filter_category_index`/分类变化重算保证模型/索引一致。
- 纯逻辑测试 `management_rows_filter_and_sort` 在未改动文件中仍绿。
- **结论**：M2 **CLOSED**。

### M3（Medium）— **RECORDED**（接受 response 记录）

- Slint 1.18 winit 不透传 OS `DroppedFile`，`folder_picker.rs` 文档化 seam 保留，`preview_batch` 可被未来 hook 调用。添加链路仍为：原生多选器（IFileOpenDialog 多选）+ 剪贴板粘贴 + 手工路径输入。不新增代码、不伪造入口。**本次不再改动，RETAINED 为已知限制。**
- 处置：reviewer 接受该限制记录，不阻塞 M05 批准。

### M4（Medium）— **CLOSED**

- `src/storage/tests.rs`：`source_under_storage_and_domain_has_no_fs_delete_api_calls` 扫描根由 `{storage, domain}` 扩展为 `{storage, domain, presentation, platform}` 递归；`remove_dir/remove_dir_all/delete_directory` 在实现源（非 `*tests.rs`）零容忍；`remove_file` 仅允许 `src/storage/io.rs`。
- **覆盖到 manager.rs/management.rs/path_semantics.rs**：递归遍历覆盖 presentation 全目录（含 manager.rs、management.rs、view_model.rs）与 domain（含 path_semantics.rs）；`src/platform/` 亦纳入。
- 实测：`git grep remove_dir|delete_directory` 在实现源（排除 tests.rs）**零命中**；`remove_file` 仅 `src/storage/io.rs:89,124`（临时文件清理/lock drop）。repository.rs 无 remove_file。
- **结论**：M4 **CLOSED**（守护从人工核验升级为结构性 CI 约束）。

### M5（Medium，说明）— **RECORDED**

- `remove_file` 仅限 storage/io.rs 的边界与 guard 措辞维持不变（M4 扩展后仍如此）。无新问题。

### M6（Medium）— **DEFERRED（明确延期至 M06，reviewer 接受并记录）**

- 核实：分类/标签**重命名与标签合并**仍然无 UI 入口——`on_command_rename_category` 等在 main.rs **零命中**；`category-rename/tag-rename/tag-merge-to` 仍为 slint 中未绑定给任何按钮/菜单的静态 i18n property（`git grep` 确认只在 UiStrings.ts 声明处出现，无 `Button`/`MenuRow` 消费）。**未出现“展示了但不可用的重命名/合并控件”** —— 无伪功能，延期是诚实的。
- 纯逻辑（`RenameCategory/RenameTag/MergeTag`、`merge_tag` 原子性与测试）在 r01 即已存在且未改动，风险集中在 UI 接线而非数据语义。
- **验收衔接**：M05 所声明的 UI（添加/编辑/删除/筛选/排序/上下文菜单）在本轮已真实完整；重命名/合并属二级编辑面，明确为 **M06 范围**。reviewer 明确接受该范围调整：**M6 延期不阻塞 M05 里程碑批准**，但必须在 M06 交付并复查；M05 里程碑文档与 task 状态需如实标注“重命名/合并 UI 在 M06”。

### M7（Medium，说明）— **RECORDED**

- `import-max-hint`：UI hover 直接用 `ONE_LEVEL_IMPORT_MAX_LITERAL`（“max 100”），不走该键；无消费者，属性保留。
- `category-rename/tag-rename`：与 M6 一致，待 M06 重命名 UI 接入后消费。
- 分类状态筛选已在 M2 接入独立译文（FilterStatusAll/Enabled/Disabled）；SortName/Recent/Added 亦独立。
- 非数据/非安全问题，低优先清理。

### L1（Low）— **CLOSED**

- add/edit 对话框新增 “已启用” CheckBox（slint:1170-1176）绑定 `draft-enabled`，`command-set-enabled`（1991）→ `SetDraftEnabled`。r01 记录的“声明未接控件”回调现已有关联真实控件。

### L2（Low）— **RECORDED**

- undo 横幅 “Cancel” 语义为丢弃撤销（`manager.rs:cancel_remove` 注释明确）。文案区分属低优先文案打磨，不阻塞本轮。

### L3（Low）— **RECORDED**

- `open_manual`/`command-add` 空路径手动输入：空路径在对话框被路径校验拦截，不保存空记录。低优先打磨项。

## 3. r01 核心 PASS 项回归安全复核（点 9）

以下 r01 已 PASS 且**被 bb8d0c7 改动触及的仅剩文件外**的核心不变项，逐一复核：

| 不变项 | r01 结论 | 本轮核实 |
|---|---|---|
| 无真实目录删除可达 | ✅ | `git diff 49dbb77 bb8d0c7` 中 `m05_tests.rs`/`path_semantics.rs`/`management.rs`/`view_model.rs` 均为 0 行 diff（字节级未变）；全仓实现源 `remove_dir*`/`delete_directory` 零命中（M4 guard 亦加强） |
| canary 真实临时目录 remove/disable/clear-all 保留目录+内容 | ✅ | `canary_remove_and_disable_keep_real_dir_and_contents`（m05_tests.rs:107）、`canary_clear_all_records_keeps_every_real_dir`（141）未改动，CI 绿 |
| undo 跨保存边界 revision-guard | ✅ | `undo_is_refused_after_an_intervening_save`（manager.rs:2073）、`undo_remove_record` revision guard（repository/manager）未改动，CI 绿 |
| path semantics M01.2 矩阵 | ✅ | `path_semantics.rs` 0 行 diff。新增 fs-deletion guard 扫描包含它，元数据读取类（metadata/read_dir）不受影响 |
| 无新删除面 | ✅ | 本轮新增 H2 `resolve_duplicate`/`commit_draft` 仅记录增改（put_folder/编辑），无任何删除 API；C1 索引→id 解析为纯读取 |
| M4 guard 自身 | ✅ | `source_under_storage_and_domain_has_no_fs_delete_api_calls` 在 CI 绿（329 passed 包含之） |

**结论**：r01 全部核心安全不变项在修复后保持成立，CI 绿证证实。

## 4. 横切检查（r02）

- **Correctness**：C1 索引→id 解析消除了结构性错误映射；H2 duplicate 三 policy 语义读码确认；M1/M2 索引映射边界（category index 0/1/2+k、status 0/1/2、sort 0/1/2）全部正确。
- **Error handling**：越界索引（folder_id_at None、category/tag index 越界 None、draft tag index None）全部 no-op，不作用于错误记录；`set_one_level_import` 保存失败置 `SaveFailed` notice。
- **Data safety（绝对不变式）**：**维持通过**。所有新路径仍为记录级操作；`remove_file` 仅 io.rs 临时清理；`resolve_duplicate::SaveAsDifferentName` 保留同路径不同名独立记录（不触碰真实目录）。
- **Privacy**：本轮新增 eprintln 无（git diff + 行检查确认）；设置窗口文案不记路径；`draft_color_to_slint` 不涉及路径。
- **Security**：无新增命令执行/RCE 面；`%VAR%` 展开仍在受控 `expand_open_path` 内；无 shell/telemetry。
- **Windows 行为**：右键菜单用 Slint PopupWindow（close-on-click-outside），OS 级右击释放事件由 pointer-event 捕获——真实桌面弹出位置/outside/Escape 属人工验收项。
- **Tests**：新增 6 测试全部读码断言成立；CI 332 passed（329 lib + 3 bin）+ 1 ignored。
- **Performance**：无后台扫描；`rows_enabled/pinned` 与 draft tag 勾选均 O(n)；管理页 refresh 规模=用户记录数（与 r01 一致）。
- **A11y**：新增 CheckBox/SpinBox/ComboBox 为标准 widget；ResultRow 保留 accessible-label/selected/index。未做桌面实测。
- **Maintainability**：`folder_id_at` 为纯助手（Option）；`resolve_duplicate`/`commit_draft` 重构复用；slint 单文件合并说明保留；i18n 键类型化。manager.rs 仍大（~2390 行随新增测试增长），但结构清晰。

## 5. 新增发现（r02）

未发现新的 Critical/High/Medium 阻断项。记录两条低优先级观察（非阻断）：

1. **OBS-01（Low）**：H1 右键菜单 `command-context-action` 的 action 码（0-6）依赖 slint 与 main.rs 的顺序一致，两者现为一处 `MenuRow` 顺序与一个 match 顺序，属常量级耦合；建议未来提取为命名常量以免重排菜单项时错位。当前读码确认顺序一致。
2. **OBS-02（Low）**：`action_toggle_enabled` i18n 静态键已由 H4 改为 `DisabledLabel`，但该静态属性已无按钮消费（仅兜底）。与 M7 同类死键清理项，低优先。

## 6. 未自动验证的 GUI/平台项（需人工桌面验收，与 r01/response 一致并新增）

1. 右键上下文菜单在真实 Windows 上的弹出位置、项点击、outside/Escape 关闭后的 overlay 复位（H1 新增面）。
2. H2 对话框各字段真实交互：SpinBox 聚焦、CheckBox 行、颜色循环色块、分类 ComboBox 模型/索引一致性（新增面）。
3. H3 两步删除确认的可见反馈与取消复位（新增面）。
4. M1 one-level-import 开关与受控导入二次确认流程（新增面）。
5. M2 筛选/排序选择器在列表变化后的模型/索引一致性（新增面）。
6. 原生文件夹多选器模态与多选返回。
7. 中文 IME 在名称/路径/备注/标签创建输入框的组成行为。
8. 多显示器 / DPI 缩放下设置窗口布局（min 760×520）。
9. 撤销横幅 8 秒计时真实表现。
10. 从 FileGo 移除后资源管理器对应真实目录未被触碰（canary 之外的日常路径复核）。
11. 托盘添加 → 设置窗口焦点顺序。

## 7. 需求/验收标准映射（M05.1–5.5 + M01.2 + M03）

| 要求 | r01 | r02 |
|---|---|---|
| M05.1 导航/CRUD/名称筛选/单活动检查/不扫描 | ✅ | ✅ + 分类/状态筛选与排序选择器（M2）已绑定 |
| M05.2 添加/编辑对话框全字段 | ⚠️ | ✅（note/weight/color/category/tags/enabled/duplicate/unsaved 全绑定） |
| M05.2 重复 取消/编辑既有/另存他名 | ❌ | ✅（ResolveDuplicate 三路接线+测试） |
| M05.2 受控 ONE-LEVEL 导入（默认 OFF、max 100、二次确认、不递归） | ⚠️ 无入口 | ✅ 开关已绑定（M1） |
| M05.2 OS 拖拽 | ❌ seam | RECORDED（Slint winit 限制，不伪造） |
| M05.3 移除/撤销/undo 跨保存边界 | ✅ | ✅（未改） |
| M05.3 canary 真实目录保留 | ✅ | ✅（未改） |
| M05.4 分类/标签 创建/删除/颜色/用量 | ⚠️ | ✅ + 两步删除确认（H3） |
| M05.4 重命名/合并 | ❌ | DEFERRED（M06，逻辑已存在，UI 诚实未伪造） |
| M05.5 右键上下文菜单 7 项 | ❌ 死代码 | ✅（H1 全部可达） |
| M01.2 path semantics | ✅ | ✅（未改） |
| M03 右键补全（M04 open/copy-path） | ✅ | ✅（并入 7 项菜单） |

## 8. 结论

**Verdict: APPROVED_FOR_MILESTONE**

（注意：这是**里程碑级批准**，不是 release 批准。CLAUDE.md §4.5 要求 release 必须由最终 release code-review agent 对实际候选 commit 给出 `APPROVED_FOR_RELEASE`，此处不构成该批准。）

理由：

1. **C1（Critical）已关闭**：FolderId 已彻底不再经 Slint `int` 往返；`folder_id_at` 索引→完整 128 位 id、越界 no-op；`rows-ids` 已删；两个回归测试（含“低 32 位=0 碰撞”直接回归）存在且与 CI 绿证一致。
2. **H1-H4 全部关闭**：右键 7 项菜单真实可达且 overlay 复位健全；add/edit 对话框全字段有 widget+handler（color 为真调色板）；分类/标签删除两步确认且语义保持；启用/禁用按钮显示 禁用/恢复。
3. **M1/M2/M4 关闭**：one-level 开关真实持久化且默认 OFF 无扫描；三筛选/排序选择器接纯逻辑；fs 守护扩展到 presentation/platform 并覆盖 manager/management/path_semantics。
4. **核心安全不变式维持**：canary/undo/path-semantics 文件零改动、CI 332 passed 绿证；实现源无任何真实删除 API。
5. **M6 延期被接受并记录**：重命名/合并 UI 明确归 M06，未出现伪功能；不阻塞 M05。
6. CI：Windows CI `35921184606` + Search benchmark `35921184624` 均在 `bb8d0c7` MSVC 全绿；doc-only `c1d5f1d` 正确零触发。
7. 需人工验收的桌面项（§6，尤其右键菜单、对话框新字段、两步删除、one-level 开关）仍须由用户在真实 Windows 上验收后方可进入发布流程。

**未解决事项（非 M05 阻断）**：M3 OS 拖拽（记录）；M6 重命名/合并（延期 M06）；M7/L2/L3 低优先清理；OBS-01/OBS-02 低优先记录。

**复审请求**：若主 agent 选择将 M05 作为里程碑冻结，本 r02 结论为 `APPROVED_FOR_MILESTONE`；发布仍需：(a) 用户桌面手工验收记录到 `review/0-0-1/manual-acceptance.md`；(b) 独立 release code-review 对最终候选 commit 给出 `APPROVED_FOR_RELEASE`。

## 9. 附录：r02 评审关键证据定位（head bb8d0c7）

- `src/presentation/manager.rs:598-604`（folder_id_at）、`2250-2268` / `2360-2395`（C1 两个回归测试）、`1031-1052`（cycle_draft_color 调色板）、`1077-1115`（resolve_duplicate）、`1640-1656`（set_one_level_import + dismiss）、`282`（SetOneLevelImport）
- `src/main.rs:1728-1790`（5 个文件夹行 handler）、`1628-1647`（on_command_context_action）、`280-292`（request_context_action）、`563-594`（drain_context）、`1990-2077`（H2 各 handler）、`1900-1942`（confirm-delete）、`1806-1854`（M2 三 handler）、`814-821`（draft_color_to_slint）、`398-418`（rows_pinned/enabled 推送）
- `src/storage/repository.rs:655-666`（set_one_level_import）
- `src/storage/tests.rs:690-764`（M4 守护扩展）
- `src/domain/settings.rs:215-218,245`（one_level_import serde default off）
- `ui/app-window.slint:199-216`（row-context-requested）、`317-380`（ContextMenuPopup 7 项）、`602-613`（row-context handler + overlay）、`695-712`（context-menu 组件 + close）、`930-951`（M2 三 ComboBox）、`955-967`（one-level Switch）、`1090-1195`（H2 对话框全字段）、`1039-1078`（H3 两步确认）、`763-767`（H4 按钮文案）
- `src/presentation/i18n.rs:182-200,309-322,452-470,612-636,800-830`（H2/H3 新增 zh/en）
