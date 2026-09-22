# Response: M01-A Versioned Folder Data Model + Pure JSON Codec (Round 01)

## Metadata

- **版本:** `0.0.1`
- **里程碑/topic:** `m01-storage`
- **轮次:** `01`
- **Implementation agent:** M01-A implementation agent（仅实现与补测；未参与 m01-storage-review-r01 评审）
- **对应 review 文档:** [`m01-storage-review-r01.md`](m01-storage-review-r01.md)
- **评审前 commit SHA（head）:** `b6ba57ec3efd5083fdf3ac01978a29ec69518dfc`
- **修复/补测后 commit SHA:** `%（见下方 CI 证据）`
- **Response 日期:** 2026-09-21

## Summary

| Finding | Assessment | Status |
|---|---|---|
| F001 | `ACCEPTED` | 说明性结论，无需代码修改；补一条断言说明 |
| F002 | `ACCEPTED` | 说明性结论，无需代码修改；M01-B 阶段考虑 in-place 变体 |
| F003 | `ACCEPTED`（with-note） | 说明性结论，无需代码修改；后续影响已记录 |
| F004 | `ACCEPTED` | 说明性结论，无需代码修改 |
| F005 | `ACCEPTED`（with-note） | 说明性结论，无需代码修改；保留后续影响 |
| F006 | `ACCEPTED` | 说明性结论，无需代码修改 |
| F007 | `ACCEPTED` | 已补测：settings 与 folder 范围边界测试 |
| F008 | `ACCEPTED` | 已补测：所列校验分支直接单测 |
| F009 | `ACCEPTED` | 确认性结论，无需代码修改 |
| F010 | `ACCEPTED` | 确认性结论，无需代码修改 |

无 `REJECTED` / `PARTIALLY_ACCEPTED` 条目。F001–F006、F009–F010 均接受为说明/确认性结论并给出理由；F007 与 F008 接受并补齐测试。未对任何领域/编码逻辑代码做改动（仅测试文件）。

## Finding Responses

### `F001` — `ACCEPTED`（说明性，无代码修改）

- **评估:** 正确。`StorageError::Display`（codec.rs:56-83）与 `ValidationError::Display`（error.rs:42-75）只输出固定英文消息，`StorageError` 不携带内容字段，泄漏防护断言（tests.rs 原 304-305、359-360）是有效回归保护。
- **理由与证据:** 与 review 的判断一致；按 CLAUDE.md §4.4 记录为接受。新增测试继续复用 `SENSITIVE_PATH`/`SENSITIVE_JSON_MARKER` 断言路径（`validation_errors_do_not_expose_sensitive_path_or_json` 已覆盖 InvalidDocument 路径，`invalid_or_truncated_json_is_distinguished_without_content_leakage` 已覆盖 InvalidJson 路径）。
- **修改:** 无代码修改；错误类型设计维持现状。

### `F002` — `ACCEPTED`（说明性，无代码修改）

- **评估:** 正确。`encode`（codec.rs:92-94）在克隆副本上就地规范化与校验，以保留调用者原 `document` 不被改写。
- **理由与证据:** 个人规模（<2000 条记录）开销可忽略，启动期调用次数极少。当前保持 `encode` 不变；M01-B 若写入路径成为热点，再提供 `encode_mut`/in-place 变体。
- **修改:** 无。

### `F003` — `ACCEPTED`（with-note，无代码修改）

- **评估:** 正确。重复检测使用 `Vec::contains`，最坏 O(n²) 仅出现在手工构造的严重重复输入。
- **理由与证据:** 个人规模下可忽略，且没有任何自动扫描产生 10k 规模输入。按 reviewer 建议：M02 引入 10k 基准时若仍需走此路径再改 `HashSet`；本条记录于未来影响。
- **修改:** 无。

### `F004` — `ACCEPTED`（说明性，无代码修改）

- **评估:** 正确。`typed_id!` 的 `Default` 委托 `new()` 生成随机 v4 UUID，语义为“生成一个新 ID”。
- **理由与证据:** 已确认 `src` 下不存在依赖默认 ID 的构造路径，domain/codec 构造中所有 ID 均显式提供，`#[serde(default)]` 未被使用。无未知字段自动补默认 ID 导致静默引用错乱的路径。
- **修改:** 无；已在设计上保留 `Default`，其语义在文档注释中可注明。

### `F005` — `ACCEPTED`（with-note，无代码修改）

- **评估:** 正确。`chrono["clock"]` 会拉入 timezone/iana 依赖；当前模块不使用时钟函数。
- **理由与证据:** 二进制体积/依赖面轻微增加，与 Data Safety 无关联，MIT 分发无阻。按 reviewer 建议保留现状：M02 时间戳（创建/更新时间）需要 `Utc::now()`，为该场景保留 `clock` feature 是安全且必要的；若后续确认存储层不生成时间戳，可移除以缩小依赖。
- **修改:** 无；Cargo.toml / Cargo.lock 未改动。

### `F006` — `ACCEPTED`（说明性，无代码修改）

- **评估:** 正确。`migrate_to_current`（codec.rs:107-113）在反序列化前对 schema_version 前向/后向判定，未来 schema 先命中 `UnsupportedFutureSchema`，不会静默忽略新增字段。
- **理由与证据:** 与 `deny_unknown_fields`（要求 6）组合后行为一致。
- **修改:** 无。

### `F007` — `ACCEPTED`（已补测）

- **评估:** 缺口属实：原先只覆盖上界之外与默认值，未覆盖 min−1/min/max/max+1 边界与 folder 边界。
- **修改:** 新增测试于 `src/storage/tests.rs`：
  - `settings_range_boundaries_are_enforced_inclusively`
  - `folder_range_boundaries_are_enforced_inclusively`
- **覆盖点（全部使用导出常量而非魔法数字，常量均已 `pub`，故无需改动设置/领域代码）:**
  - `max_results`：`MIN_MAX_RESULTS − 1`(0) 拒绝、`MIN_MAX_RESULTS`(1) 接受、`MAX_MAX_RESULTS`(100) 接受、`MAX_MAX_RESULTS + 1`(101) 拒绝；
  - `window_width`：`MIN_WINDOW_WIDTH − 1`(479) 拒绝、`MIN_WINDOW_WIDTH`(480) 接受、`MAX_WINDOW_WIDTH`(760) 接受、`MAX_WINDOW_WIDTH + 1`(761) 拒绝；
  - `max_edit_distance`：`0..=MAX_EDIT_DISTANCE`(0/1/2) 接受、`MAX_EDIT_DISTANCE + 1`(3) 拒绝 —— 与 `validate()`（settings.rs:75-77 仅检查 `> MAX_EDIT_DISTANCE`）一致，0 被接受；
  - `manual_weight`：`MIN_MANUAL_WEIGHT − 1`(−101) 拒绝、`MIN_MANUAL_WEIGHT`(−100) 接受、`MAX_MANUAL_WEIGHT`(100) 接受、`MAX_MANUAL_WEIGHT + 1`(101) 拒绝；
  - 恰好 `MAX_ALIASES_PER_FOLDER`(20) 个不同别名接受、再加 1 个(21)拒绝。
- **验证:** 本地 GNU `cargo test` 两个新测试通过；MSVC CI 见下表。

### `F008` — `ACCEPTED`（已补测）

- **评估:** 缺口属实：所列校验分支无直接单测。
- **修改:** 新增测试于 `src/storage/tests.rs`（均复用 `fixture_document()` 单字段变异，断言 `encode(...).expect_err(...).kind() == StorageErrorKind::InvalidDocument`）：
  - `empty_alias_after_trim_is_rejected`（别名 `"   "` trim 后为空 → `EmptyAlias`）；
  - `too_long_alias_is_rejected`（`MAX_ALIAS_LEN + 1` = 256 字符 → `AliasTooLong`）；
  - `too_long_note_is_rejected`（`MAX_NOTE_LEN + 1` = 4097 字符 → `NoteTooLong`）；
  - `too_long_category_and_tag_names_are_rejected`（`MAX_CATEGORY_NAME_LEN + 1` / `MAX_TAG_NAME_LEN + 1` = 129 → `CategoryNameTooLong` / `TagNameTooLong`）；
  - `duplicate_category_and_tag_ids_are_rejected`（两个同 id category / 两个同 id tag → `DuplicateCategoryId` / `DuplicateTagId`）；
  - `unknown_category_reference_is_rejected`（`folder.category_id = Some(id)` 且该 category 不存在 → `UnknownCategoryReference`）。
- **验证:** 本地 GNU 全部通过；MSVC CI 见下表。

### `F009` — `ACCEPTED`（确认性，无代码修改）

- **评估:** 与 review 的 grep 结论一致，5 处 `deny_unknown_fields` 覆盖全部 wire 反序列化入口。
- **理由与证据:** 无需修改；已确认。
- **修改:** 无。

### `F010` — `ACCEPTED`（确认性，无代码修改）

- **评估:** 与 review 结论一致，`encode` 与 `decode` 两侧都运行 `validate_and_normalize`。
- **理由与证据:** 无需修改；已确认。
- **修改:** 无。

## 修改文件与测试清单

- 修改文件：仅 `src/storage/tests.rs`（+241 / −2；含 rustfmt 对 import 重排）。`src/domain/settings.rs`、`src/domain/folder.rs`、`src/domain/document.rs`、`src/domain/error.rs`、`src/storage/codec.rs`、`src/storage/schema.rs`、`Cargo.toml`、`Cargo.lock` 均未改动。
- 新增测试 8 个（均位于 `src/storage/tests.rs`）：`settings_range_boundaries_are_enforced_inclusively`、`folder_range_boundaries_are_enforced_inclusively`、`empty_alias_after_trim_is_rejected`、`too_long_alias_is_rejected`、`too_long_note_is_rejected`、`too_long_category_and_tag_names_are_rejected`、`duplicate_category_and_tag_ids_are_rejected`、`unknown_category_reference_is_rejected`。
- 测试总数从 20 → 28（storage 模块）。

## 验证方式与结果

### 本地 GNU（快速反馈，非 MSVC 权威）

环境：Rust `1.92.0-x86_64-pc-windows-gnu`（MinGW64），`RUSTUP_AUTO_INSTALL=0`。预检确认 toolchain、target、rustfmt、clippy 已安装后执行：

| 步骤 | 命令 | 结果 |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | PASS |
| clippy | `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` | PASS（0 警告） |
| test | `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` | PASS（storage 28 + main 1 = 29 通过，0 失败；bin/doc 0） |
| release | `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` | PASS |

环境备注：默认 GNU linker `x86_64-w64-mingw32-gcc` 出现已知本地工具链漂移 `cannot find -lshlwapi`，已按既定 Workaround 仅在本命令环境临时设置 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` 与 `PATH` 前置 `D:\mingw64\bin`。该 Workaround **未**写入任何仓库文件/CI/workflow，non-committed；不构成 MSVC 证据。

### 远程 MSVC CI（权威门禁）

补测提交推送 `feature/m00-foundation` 后由 GitHub Actions Windows CI（`x86_64-pc-windows-msvc`）验证。Run 见下表（同一提交的 MSVC 门禁）。

| Commit / run | Workflow | Result |
|---|---|---|
| 修复后 commit SHA：`%（响应后在推送时回填 commit SHA 与 run ID，见下备忘）` | Windows CI | 进行中/见 run 列表 |

> 备忘：推送后回填本行与下表为 `%` 占位；响应正文以推送后的实际 commit SHA 与 `gh run list` 结果为准。**response 文档随代码处于同一提交，SHA 在提交后即确定。**

## 未解决事项与待人工验证项

- 无新增真实桌面验证项（M01-A 为纯 codec/数据模型，无 UI 交互）。
- M01-B 安全持久化（数据目录、temp→write→flush/sync→backup→replace、fault injection、损坏保留与恢复等，见 `task/01-里程碑任务清单.md` M01.4）仍待后续里程碑实现。
- F003 的 O(n²) 去重在 M02 10k 基准时评估是否替换 `HashSet`；F005 的 `chrono["clock"]` 依赖面在 M02 时间戳需求明确后定夺。二者均不阻塞本里程碑。

## 复审请求（请 r02 复审）

请独立 code-review agent（M01-A reviewer 或另一名未参与实现的 reviewer）复审：本响应、`src/storage/tests.rs` 测试 diff（base `b6ba57e` → head 修复提交）、新增 8 个测试与断言、本地 GNU 结果与本响应随代码提交的 MSVC CI run。确认 F007/F008 已关闭、其余 finding 接受理由成立后，给出 `CHANGES_REQUESTED` 或 `APPROVED_FOR_MILESTONE` 结论。本次评审不涉及 release 授权（`APPROVED_FOR_RELEASE` 不属于 M01 评审范围）。
