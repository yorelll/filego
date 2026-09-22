# Review: M01-A Versioned Folder Data Model + Pure JSON Codec (Round 02)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m01-storage`
- **Round:** `02`
- **Date:** 2026-09-21
- **Reviewer agent:** Independent M01-A code-review agent（r02 复审）
- **Role declaration:** 本次 r02 复审以 **全新独立 reviewer 身份** 执行，未参与 M01-A 的实现、response 撰写或补测代码；复审过程中未修改任何源代码、未运行 cargo/build/test、未提交或推送。本次评审输出仅为本文档（及给主 agent 的文本报告）。
- **Independence statement:** 评审人为独立代码评审 agent，未参与所评代码（domain 模型、codec、schema、Cargo 变更）与新增测试的实现。本 r02 在评审前完整阅读了 `m01-storage-review-r01.md` 与 `m01-storage-response-r01.md`，并对其中所有结论逐条独立复核（见下文「逐条 r01 finding 的 r02 disposition」）。
- **Base SHA（r01 评审 head）:** `b6ba57ec3efd5083fdf3ac01978a29ec69518dfc`
- **Head SHA（本次复审）:** `3a59378bf61170404b5f778c349c34f0e0881561`
- **Comparison range:** `b6ba57e..3a59378`
- **被评审的修复/补测提交:** `9adc92b4847789c21bd773c10022ce57f0a916b5`（test: close M01-A review boundary and branch gaps）
- **被评审文件范围（代码）:** 仅 `src/storage/tests.rs`（+241 / −2，含 import 重排）；后续 3 个提交为纯文档修正（详见下）。
- **Requirements:** M01-A scope from task docs / product decisions（沿用 r01 的验收标准映射，本次仅需确认补测后闭环）。
- **Review method:** 读取 `git diff b6ba57e..9adc92b -- src/storage/tests.rs` 全量 diff、当前工作树全部 `src/storage/tests.rs`、`src/domain/settings.rs`、`src/domain/folder.rs`、`src/domain/document.rs` 关键区域、常量定义与 `validate()` 逻辑，并以 `git grep`/`git diff` 核对补测提交的文件范围与测试计数；通过 `gh.exe` 核查 3 次关键 CI run。为只读检查，未运行任何构建/测试命令。

## 修复提交（9adc92b）后的提交序列

`git log --oneline b6ba57e..HEAD`：

```
3a59378 docs: align M01-A response verification table with real count
24aa111 docs: record M01-A storage review r01
1f215ad docs: correct M01-A response test count
3d88acf docs: pin M01-A response to actual fix commit SHA
9adc92b test: close M01-A review boundary and branch gaps
```

- `9adc92b`：唯一代码提交（新增 8 个测试），并在同一提交内含 r01 response 文档初稿（146 行 doc）。
- `3d88acf` / `1f215ad` / `3a59378`：均为 `review/0-0-1/m01-storage-response-r01.md` 的纯文档修正（回填 commit SHA、修正测试计数占位符），无任何源代码改动。
- `24aa111`：记录 r01 review 文档本身（纯文档）。
- **代码文件范围核实**：`git diff b6ba57e..HEAD -- src/domain src/storage/codec.rs src/storage/schema.rs Cargo.toml Cargo.lock` 输出为空——**domain/storage 代码、codec/schema 与依赖清单在 b6ba57e..HEAD 范围内零改动**。这与 response 声称「仅修改 `src/storage/tests.rs`」一致（唯一附带变化是同提交内的 response 文档本身，属正常流程产物）。

## CI Evidence Recorded

| Run | Head SHA | Workflow | Result | 说明 |
|---|---|---|---|---|
| [Windows CI 35683419670](https://github.com/yorelll/filego/actions/runs/35683419670) | `3d88acff9a3467a125757a43b8f6644259a5d153` | `Windows CI` | `success` | **唯一真正执行代码变更（9adc92b 补测）的 MSVC 门禁 run**。单 job `fmt, clippy, test, release, package` 全部步骤 success：Check formatting、Clippy with warnings denied（`-D warnings`）、Run tests、Build release executable、Verify release outputs、许可证审计与 portable 打包等。9adc92b 已验证为 3d88acf 的祖先。 |
| [Windows CI 35684120045](https://github.com/yorelll/filego/actions/runs/35684120045) | `1f215ad2c8a6cf684630131d02f5abcb44af311f` | `Windows CI` | `success` | 纯文档修正（修正 response 测试计数占位）。同单 job 全绿；无代码变化。 |
| [Windows CI 35684632059](https://github.com/yorelll/filego/actions/runs/35684632059) | `3a59378bf61170404b5f778c349c34f0e0881561`（HEAD） | `Windows CI` | `success` | **当前 HEAD 的门禁 run**。纯文档修正（对齐验证表计数）。同单 job 全绿；无代码变化。 |

- 3 次 run 均为 MSVC target 门禁；工作流步骤显示 fmt/clippy/test/release/package 显式一致。
- **说明**：`35683419670`（@3d88acf）是唯一执行新代码（8 个补测）的权威 MSVC 证据，其历史包含修复提交 9adc92b。后两次 run 为文档-only 连续性验证，代码面与它相同。
- **透明性备注**：分支 run 列表中另有一条 **cancelled** run `35684475705`（@24aa111，「docs: record M01-A storage review r01」）。该 run 因紧随其后的新推送被取消，非失败；24aa111 为纯文档提交，不影响代码证据链。已记录以备审计。
- r01 已记录的 base/head 两次 run（35675583131 @244b68d、35678609388 @b6ba57e）仍有效，作为 r01 基线的 MSVC 证据。

## 逐条 r01 finding 的 r02 disposition

### F007 — Settings/Folder 范围边界测试缺口 — **CLOSED**

r02 独立核查当前 `src/storage/tests.rs`：

- `settings_range_boundaries_are_enforced_inclusively`（tests.rs:118-199）实际存在，断言与 response 声明一致：
  - `max_results`：`MIN_MAX_RESULTS − 1`=0 拒绝（同时经 `validate()` 与 `encode()`→`InvalidDocument` 双径断言）、`MIN_MAX_RESULTS`=1 接受、`MAX_MAX_RESULTS`=100 接受、`MAX_MAX_RESULTS + 1`=101 拒绝；
  - `window_width`：479 拒绝 / 480 接受 / 760 接受 / 761 拒绝；
  - `max_edit_distance`：`0..=MAX_EDIT_DISTANCE`（0/1/2）循环全部接受、`MAX_EDIT_DISTANCE + 1`=3 拒绝。**max_edit_distance=0 被接受与 `settings.rs::validate()`（settings.rs:75-77 仅检查 `> MAX_EDIT_DISTANCE`，不含下界）完全一致**，测试正确反映了实现。
- `folder_range_boundaries_are_enforced_inclusively`（tests.rs:201-248）：`manual_weight` −101 拒绝 / −100 接受 / 100 接受 / 101 拒绝；恰好 `MAX_ALIASES_PER_FOLDER`=20 个不同别名接受、推送第 21 个（`"alias-20"`）拒绝。与 `folder.rs:100-108` 的 `nums.contains` 包含性范围一致。
- **常量核实**：测试全部使用导出常量，无魔法数字。常量存在且值如 response 所述：
  - `settings.rs:6-12`：`MIN_MAX_RESULTS=1`、`MAX_MAX_RESULTS=100`、`MIN_WINDOW_WIDTH=480`、`MAX_WINDOW_WIDTH=760`、`MAX_EDIT_DISTANCE=2`（均 `pub`）；
  - `folder.rs:11-18`：`MIN_MANUAL_WEIGHT=−100`、`MAX_MANUAL_WEIGHT=100`、`MAX_ALIASES_PER_FOLDER=20`、`MAX_ALIAS_LEN=255`、`MAX_NOTE_LEN=4096`、`MAX_CATEGORY_NAME_LEN=128`、`MAX_TAG_NAME_LEN=128`（均 `pub`）。
- 结论：**F007 关闭**。response 的覆盖点、数值与实现逐项吻合。

### F008 — 校验分支无直接单测 — **CLOSED**

r02 独立核查 6 个新增测试，均通过 `encode(...).expect_err(...).kind() == StorageErrorKind::InvalidDocument` 断言：

| 测试（tests.rs 行） | 构造 | 触发分支算法 |
|---|---|---|
| `empty_alias_after_trim_is_rejected`（251-261） | 别名 `vec!["   "]` trim 后为空 | `EmptyAlias`（folder.rs:135-137） |
| `too_long_alias_is_rejected`（264-274） | `MAX_ALIAS_LEN + 1`=256 字符 | `AliasTooLong`（folder.rs:138-140） |
| `too_long_note_is_rejected`（277-287） | `MAX_NOTE_LEN + 1`=4097 字符 | `NoteTooLong`（folder.rs:110-112） |
| `too_long_category_and_tag_names_are_rejected`（290-308） | name 各 129 字符 | `CategoryNameTooLong` / `TagNameTooLong`（folder.rs:32-55 via `validate_named_value`） |
| `duplicate_category_and_tag_ids_are_rejected`（311-335） | 同 id category / 同 id tag push 副本 | `DuplicateCategoryId` / `DuplicateTagId`（document.rs:30-45） |
| `unknown_category_reference_is_rejected`（338-348） | `category_id = Some(id)` 且 category 不存在 | `UnknownCategoryReference`（document.rs:55-62） |

- 各测试复用 `fixture_document()` 单字段变异，失败断言精确指向 `InvalidDocument`，不依赖错误文案（与泄漏防护原则一致）。
- 对照 document.rs:47-73 确认：duplicate folder ID / unknown tag reference 已由既有 `unknown_or_duplicate_references_are_rejected` 覆盖；`TooManyAliases`/`ManualWeightOutOfRange` 已由既有测试与新增 F007 测试覆盖；`EmptyAlias`/`NoteTooLong`/`UnknownCategoryReference`/`DuplicateCategoryId` 等 r01 点名的关键分支现全部有直接单测。r01 列出的全部 8 个分支缺口均已关闭。
- 结论：**F008 关闭**。每个响应命名的测试都存在、断言与实现吻合、错误类型精确。

### F001–F006、F009–F010 — 其余 finding 处置确认

| Finding | r01 描述 | r02 disposition |
|---|---|---|
| F001（活跃断言依赖固定文案） | Low informational | `ACCEPTED` 理由成立：`StorageError::Display`/`ValidationError::Display` 仅固定英文，无内容字段；补测延续 SENSITIVE 断言路径。无修改，合理。 |
| F002（encode clone 深拷贝） | Low informational | `ACCEPTED` 理由成立：个人规模开销可忽略，M01-B 热点再引入 `encode_mut`。非阻断，合理。 |
| F003（重复检测 O(n²)） | Low | `ACCEPTED`（with-note）理由成立：仅手工严重重复输入最坏 O(n²)，M02 10k 基准时评估 `HashSet`。合理。 |
| F004（`Default` 生成随机 v4） | Low design note | `ACCEPTED` 理由成立：已核实 `src` 下无依赖默认 ID 的构造路径、无 `#[serde(default)]` 用法，无静默引用错乱路径。合理。 |
| F005（chrono `clock` 依赖面） | Low bloat note | `ACCEPTED`（with-note）理由成立：当前模块不用时钟函数；Cargo.toml/Cargo.lock 未改动，M02 时间戳需要 `Utc::now()` 时保留合理。合理。 |
| F006（未来 schema 检测位置） | informational | `ACCEPTED` 确认性结论与 r01 一致。合理。 |
| F009（deny_unknown_fields 覆盖） | Info confirmatory | `ACCEPTED` 确认 5 处覆盖全部 wire 入口，与 r01 grep 结论一致。合理。 |
| F010（encode/decode 两侧验证） | Info confirmatory | `ACCEPTED` 确认两侧均跑 `validate_and_normalize`。合理。 |

- **无 `REJECTED`/`PARTIALLY_ACCEPTED`**；response 对每条 r01 描述的转述未发现曲解或夸大，逐条均给出可复核的证据/理由。全部接受成立。

## r02 Check-list 独立验证结论

1. **无领域/逻辑改动混入**：`git diff b6ba57e..9adc92b --stat` 仅触及 `src/storage/tests.rs` 与同提交的 response 文档；`b6ba57e..HEAD` 范围内 `src/domain/*`、`src/storage/codec.rs`、`src/storage/schema.rs`、`Cargo.toml`、`Cargo.lock` 均零改动。补测提交无任何 `pub` 常量导出新增（所用常量均为 r01 已存在）。
2. **测试计数**：`git grep -c '#[test]' b6ba57e -- src/storage/tests.rs` = 17；HEAD = 25。**17 → 25（+8）与 response 一致**。
3. **Response 文档内部一致性（HEAD 版本）**：验证表已修正为「storage 25 + main 1 = 26」；汇总行为「17 → 25（新增 8）」；3d88acf/1f215ad/3a59378 修正了占位 SHA 与旧的「20 → 28」「28 + main 1 = 29」过期数字。**当前 HEAD 版本无残留过期计数**。
4. **既有不变量未削弱**：`favorite_limit_is_enforced`（MAX_FAVORITES=5，tests.rs:398-415）、`pinned_and_favorite_are_independent_persisted_fields`（4 组合 + 10 pinned + 6 fav 拒绝，tests.rs:417-478）、`unknown_fields_are_rejected_without_silent_data_loss`（deny_unknown_fields，tests.rs:547-557）、`validation_errors_do_not_expose_sensitive_path_or_json`（泄漏防护，tests.rs:591-600）、`domain_does_not_expose_real_directory_delete_api`（无真实目录删除 API，tests.rs:602-608）全部保留且未被修改；新增 8 测试仅追加在既有测试之前，未改动任何既有断言逻辑（diff 仅 import 重排 + 追加）。
5. **补测质量**：边界测试对「接受」路径用 `validate().is_ok()` 或 `encode().expect(...)`，对「拒绝」路径断言 `InvalidDocument`，与错误分类语义匹配；`max_edit_distance=0` 接受行为与实现（无下界检查）一致，未错误编码产品约束。

## Cross-Cutting Checks（r02 复审）

- **正确性**：新增测试与被测实现（settings.rs:66-80、folder.rs:79-147、document.rs:20-80）逐项核对一致；无领域逻辑在本轮变化，r01 的正确性结论仍然成立。
- **错误处理**：补测全部经由 `encode` 抵达 `InvalidDocument`，覆盖在 decode/encode 双径之一上有代表性；错误分类不受影响。
- **数据安全**：无代码改动，纯数据层无 fs/无删除/无网络，泄漏防护既有测试保留。
- **隐私与安全**：无新依赖、无遥测；SENSITIVE fixture 仅测试使用。
- **Windows 行为**：纯 codec，无平台 API 变化；无新增真实桌面验证项。
- **测试覆盖**：F007/F008 缺口补齐后，r01 枚举的正常路径、错误分类、round-trip、Unicode、fav 上限、pinned/favorite、去重、迁移/未来 schema、溢出、权限保护与 8 个新分支均有直接测试。MSVC `Run tests` 全绿。
- **性能**：补测不引入热点；r01 的 O(n) 编解码结论不变。
- **可访问性**：无新增 UI 影响。
- **可维护性**：补测使用导出常量与 `fixture_document()` 单字段变异，命名清晰，无魔法数字；`-D warnings` clippy 门禁通过。

## 新发现问题

未发现新的问题。以下两条为记录性说明，**不构成 finding**：

- N/A-1（信息记录）：补测提交 `9adc92b` 同时包含 `src/storage/tests.rs` 与 146 行 r01 response 文档；response 所述「修改文件仅 tests.rs」严格来说是「代码文件仅 tests.rs」（response 文档为同提交伴生产物）。数字本身（+241/−2）与 stat 完全吻合，无实质影响。
- N/A-2（信息记录）：分支 run 列表含一条 cancelled run `35684475705`（@24aa111 纯文档提交），因后续推送取消，非失败；已在上文 CI 表中记录。head `3a59378` 与最终成功 run 一致。

## 未能自动验证的 GUI/平台项

- M01-A 为纯 codec/数据模型里程碑，无 UI 交互与平台 API 变更；本 r02 无需要真实桌面验证的新增项。
- 沿用 M00 已记录的桌面边界（托盘、IME、DPI、Explorer 重启等）不在本里程碑改动范围，归属后续里程碑/手工验收。

## 最终结论

`APPROVED_FOR_MILESTONE`

- **F007**：已关闭（boundary tests 存在、断言与实现一致、使用导出常量）。
- **F008**：已关闭（8 个 r01 点名的校验分支均有直接单测、错误类型断言精确）。
- **F001–F006、F009–F010**：r02 接受 response 的 `ACCEPTED` 理由，未发现对 r01 文本的曲解或跳过。
- 无任何新的 `Critical`/`High`/`Medium`/`Low` finding；r01 唯一要求（补测）已在本轮 CI 中由 MSVC 门禁验证（35683419670 为执行代码变更的权威 run，后两次 run 保持文档连续性全绿）。
- **本结论仅为里程碑批准，不构成发布授权**：`APPROVED_FOR_RELEASE` 不属于 M01 评审范围，tag/GitHub Release 仍须由后续独立 release 评审按 CLAUDE.md §6 另行批准。
