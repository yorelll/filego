# Review: M01-A Versioned Folder Data Model + Pure JSON Codec (Round 01)

## Metadata

- **Version:** `0.0.1`
- **Milestone/topic:** `m01-storage`
- **Round:** `01`
- **Date:** 2026-09-21
- **Reviewer agent:** Independent M01-A code-review agent
- **Role declaration:** Code-review only; no repository files were modified during this review.
- **Independence statement:** The reviewer did not implement the M01-A domain models, codec, schema, Cargo.toml/Cargo.lock changes, tests, or any commit in the reviewed range. Reviewer output is limited to this document (plus the text report to the main agent).
- **Base SHA (pre-M01):** `d8c3a7bfafb67c049afb1c895e2994a0f1f3b922`
- **Head SHA (reviewed):** `b6ba57ec3efd5083fdf3ac01978a29ec69518dfc`
- **Comparison range:** `d8c3a7b..b6ba57e` (12 files changed, +1148 / −15)
- **Reviewed file scope:**
  - `src/domain/mod.rs`, `src/domain/ids.rs`, `src/domain/folder.rs`, `src/domain/document.rs`, `src/domain/error.rs`, `src/domain/settings.rs`
  - `src/storage/mod.rs`, `src/storage/schema.rs`, `src/storage/codec.rs`, `src/storage/tests.rs`
  - `Cargo.toml`, `Cargo.lock`
- **Requirements:** M01-A scope from task docs / product decisions (mapped in detail below).
- **Review method:** read the actual current file contents, the full `d8c3a7b..b6ba57e` diff, the pinned-restore commit `b6ba57e` in isolation, and the relevant region of `Cargo.lock`. Did not run `cargo`/`build`/`test` (reviewer policy). This is pure codec/shape work with no GUI in scope.

## CI Evidence Recorded

| Run | Head SHA | Workflow | Result | Jobs / gates |
|---|---|---|---|---|
| [Windows CI 35675583131](https://github.com/yorelll/filego/actions/runs/35675583131) | `244b68d93da44325f82bcc823059ebdbf532a11c` | `Windows CI` | `success` | single job `fmt, clippy, test, release, package`: formatting, Clippy (`-D warnings`), tests, MSVC release build, release-output/version smoke, cargo-deny audit, license inventory, portable package + artifact upload — all `success` |
| [Windows CI 35678609388](https://github.com/yorelll/filego/actions/runs/35678609388) | `b6ba57ec3efd5083fdf3ac01978a29ec69518dfc` (head) | `Windows CI` | `success` | same single job and gates, all `success` |

- Both are MSVC-target runs; `ci.yml:22` sets `TARGET: x86_64-pc-windows-msvc`, and fmt/clippy/test/release/package steps all use that target explicitly.
- A separate local GNU validation (MinGW64 `x86_64-pc-windows-gnu`, Rust `1.92.0`) also passed prior to the CI runs; it is quick-feedback evidence only and does not substitute for the MSVC CI above.

## Requirement / Acceptance Mapping

| # | M01-A requirement | Status | Evidence |
|---|---|---|---|
| 1 | Typed UUID IDs for Folder/Category/Tag | PASS | `src/domain/ids.rs` `typed_id!` macro generates `FolderId`/`CategoryId`/`TagId` wrapping `Uuid` with `new()` (v4), `const from_uuid`/`as_uuid`, `Default`, and `Serialize`/`Deserialize` via `#[serde(transparent)]`. |
| 2 | FolderEntry fields incl. validation bounds | PASS | `src/domain/folder.rs:58-76` defines all 18 fields. Validation in `validate_and_normalize` (lines 79-116): display_name non-blank, ≤255; path non-blank, ≤32,767; aliases normalized (trim), dedup, ≤20, each ≤255, non-empty; manual_weight in −100..=100; note ≤4096; tag_ids dedup. Path is not canonicalized or access-checked (documented M01-A decision). |
| 3 | Remove category/tag clears refs only | PASS | `AppData::remove_category` / `remove_tag` in `src/domain/document.rs:88-113`; `retain` on the list and clear references on `FolderEntry.category_id` / `.tag_ids`. Regression test `removing_category_and_tag_only_clears_references` (tests.rs:268) asserts the folder survives with `None`/reduced ids. |
| 4 | AppData = settings + folders + categories + tags + revision; revision ≥1; overflow-safe next_revision | PASS | `src/domain/document.rs:10-17`, `revision: u64`; zero rejected (`InvalidRevision`, lines 21-23); `next_revision` uses `checked_add` (lines 82-86, `RevisionOverflow`). Tests at tests.rs:337-350 (zero revision; `u64::MAX` overflow). |
| 5 | Settings incl. theme/max_results/window_width/search toggles (search_pinyin / search_english_initials / max_edit_distance = M02 inputs) + hide/clear-on-open + hide_on_focus_loss + launch_at_login; range validation | PASS | `src/domain/settings.rs:23-41` all 17 fields; doc comment on `search_pinyin` (line 33-34) explicitly scopes it to M02 key derivation with no scan index; `validate()` (lines 66-80) validates max_results (1..=100), window_width (480..=760), max_edit_distance (≤2). |
| 6 | Schema: CURRENT_SCHEMA_VERSION=1; root schema_version+revision; wire deny_unknown_fields; future→UnsupportedFutureSchema; legacy→MigrationRequired; malformed→InvalidJson; invalid domain→InvalidDocument | PASS | `src/storage/schema.rs:5` `CURRENT_SCHEMA_VERSION: u32 = 1`. `codec.rs`: `SchemaProbe` (87-90) reads only `schema_version`; `migrate_to_current` branches (107-127); `deny_unknown_fields` on all five wire structs (details below); deserialize abort → `InvalidJson`, validation abort → `InvalidDocument`. Tests: future (321), legacy (329), malformed (299), unknown-fields (309). |
| 7 | Codec: pure encode/decode, no fs side effects, UTF-8 JSON, stable round-trip | PASS | `encode`/`decode` are pure (codec.rs:92-105); `to_vec_pretty` (UTF-8 bytes); round-trip stability test `json_round_trip_is_stable_and_preserves_unicode` (tests.rs:286). `storage/mod.rs` doc comments state M01-A keeps the module free of fs side effects (M01-B adds the repository boundary). |
| 8 | Error Display must not leak stored paths / JSON content / note text | PASS | `StorageError` carries only `kind` + optional `schema_version` (codec.rs:26-54); `Display` has no content interpolation (56-83). `ValidationError::Display` emits only fixed English messages. Tests: leak checks at tests.rs:299-306, 353-361. |
| 9 | No API that deletes real directories; no fs in scope | PASS | No `std::fs` / `PathBuf` / `File` imports in `src/domain`/`src/storage`. `remove_dir`/`remove_file`/`delete_directory` appear only as string literals in the guard test `domain_does_not_expose_real_directory_delete_api` (tests.rs:363-369). |
| 10 | Dependencies: chrono=0.4.45 (clock+serde), serde=1.0.229 (derive), serde_json=1.0.151, uuid=1.26.1 (serde+v4); lockfile matches manifest | PASS | `Cargo.toml:17-29` pins exactly those versions/features. `Cargo.lock` resolves `chrono 0.4.45`, `serde 1.0.229`, `serde_json 1.0.151`, `uuid 1.26.1` (with `getrandom 0.4.3` added under uuid's `v4`). All four crates are MIT/Apache-2.0 dual-licensed, compatible with MIT distribution; the CI license-inventory gate (`cargo about --locked --fail ...`) passed on both runs. |

## Findings (by severity)

### F001 — 泄漏防护断言依赖固定错误文案 — Level: Low (informational)
**问题**: `tests.rs:304-305` 与 `tests.rs:359-360` 断言错误文本不包含 `SENSITIVE_PATH`/`SENSITIVE_JSON_MARKER`,前提是 `Display` 只输出固定文案而不带字段值。经核查,`StorageError::Display`(codec.rs:56-83)与 `ValidationError::Display`(error.rs:42-75)均只输出固定英文消息,`StorageError` 不携带内容字段——断言是有效的,恰好验证了需求 8。
**影响**: 无。
**建议**: 无需修改。若未来错误类型开始携带字段,这些测试会失败并提醒回归。

### F002 — `encode` 的 `clone()` 深拷贝 — Level: Low (informational)
**问题**: `codec.rs:92-94` — `encode` 先 `document.clone()`(复制整个文档,含所有字符串/向量/UUID),在副本上做就地规范化与校验,以保留调用者持有的原 `document` 不被 `validate_and_normalize` 改写。
**影响**: 个人规模(<2000 条记录)可忽略;启动时编码/解码次数极少。
**建议**: 无需修改。若 M01-B 写路径成为热点,可提供 `encode_mut`/in-place 变体。非阻断。

### F003 — 重复检测 O(n²) (Vec::contains) — Level: Low
**问题**: `document.rs:27-73`(folder/category/tag ID 重复检测)与 `folder.rs:141`(别名去重)、`149-157`(tag_id 去重)均使用 `contains`。O(n²)最坏情形只出现在手工构造的严重重复输入。
**影响**: 个人规模(<2000)开销可忽略;分片无关。没有任何自动扫描产生 10k 规模的输入。
**建议**: 不修改;M02 引入 10k 基准时若仍走此路径可换 `HashSet`。已在「性能」小节记录。

### F004 — `typed_id!` `Default` 生成随机 v4 — Level: Low (design note)
**问题**: `ids.rs:24-28` 中 `Default` 委托 `new()` → `Uuid::new_v4()`(依赖 `getrandom`)。`Default` 约定为“零/空” 值,这里返回随机值。该 ID 仅通过 `derive(Eq)`/`PartialEq`/`Hash` 语义使用，且没有代码路径在「默认未被显式赋值的构造上下文」中依赖 `FolderId::default()`(搜索确认:src 下无 `FolderId`/`CategoryId`/`TagId` 的 `default()` 调用;缺少的字段在 domain/codec 构造中全部显式提供)，因此不存在「未知字段自动补默认 ID 导致静默引用错乱」的路径。
**影响**: 若未来在 `#[serde(default)]` 字段上依赖 `Default`，会产生随机 ID,但当前无此用法。可接受。
**建议**: 不修改;可在文档中注明 `Default` 语义为“生成一个新 ID”。

### F005 — `chrono` 的 `clock` feature 拉入 timezone/iana — Level: Low (bloat note)
**问题**: `Cargo.toml:17` 启用 `chrono["clock"]`。经核实,当前两个新增模块(schema/codec/tests/settings/folder/document)在解构/编码/测试中**从不调用** `Utc::now()`/`offset`/`Local`/`.tz` 等时钟函数,只使用 `DateTime<Utc>` 值。`clock` feature 会拉入 `iana-time-zone`、`windows-link`、`js-sys`、`wasm-bindgen` 等依赖树(见 Cargo.lock:677 附近的 chrono 依赖)。
**影响**: 二进制体积/依赖面轻微增加,但与 Data Safety 无关联;Windows 目标上 iana-time-zone 沿用现有 windows 依赖。MIT 分发无阻。
**建议**: 可选优化——若后续 M01-B 不要在存储层生成时间戳,可去掉 `clock` feature 缩小依赖;若 M02 需要 `Utc::now()`(创建/更新时间戳),则必须保留。当前保留是安全的,不作为缺陷。

### F006 — 未来 schema 检测位置 (informational)
**问题**: `migrate_to_current`(codec.rs:107-113)在反序列化前仅对 `schema_version` 前向/后向判定,不做完整字段校验;后续 `deny_unknown_fields` 的 `WireDocumentV1` 反序列化,再进入 `validate_current_document`。未来 schema(eg. 向 v2 添加新字段)将先命中 `UnsupportedFutureSchema`,不会静默忽略字段——符合要求 6。
**影响**: 无。
**建议**: 无需修改。

### F007 — Settings 范围校验的边界测试缺口 — Level: Low (test coverage gap)
**问题**: 测试只覆盖“上界之外”(`tests.rs:95-109`)和默认值(74-93)。未覆盖:max_results 的下界 0(等于 MIN−1)、window_width 下界 479、max_edit_distance 恰好 2(边界接受)以及 min 恰好 1/480(边界接受)。`folder.rs` 边界(min −100、高 100、别名数恰好 20)也未测。
**影响**: 校验逻辑本身简单且正确(`contains` 使用包含性范围);缺口仅为回归保险。
**建议**: 追加边界测试(每个范围的 min−1、min、max、max+1)。非阻断;建议在 response 中补充。

### F008 — 若干校验分支无直接单测 — Level: Low (test coverage gap)
**问题**: `EmptyAlias`、`AliasTooLong`、`NoteTooLong`、`CategoryNameTooLong`、`TagNameTooLong`、`DuplicateCategoryId`、`DuplicateTagId`、`UnknownCategoryReference`(folder 的 `category_id` 为 `Some(id)` 但该 category 不存在)均无直接测试用例。`TooManyAliases` 已由 `aliases_are_normalized_and_invalid_manual_weight_is_rejected` 覆盖(`0..=20` 生成 21 个不同别名,超过上限 20)。`DuplicateFolderId`、`UnknownTagReference` 已测(`unknown_or_duplicate_references_are_rejected`)。`ManualWeightOutOfRange` 已测(101)。空 `tag_ids`(`[]`)未显式测,但被 `encode` 正常路径隐式覆盖。
**影响**: 非缺陷;这些校验逻辑均 3-6 行且无复杂分支。个人规模下风险低。
**建议**: 在 response 中补齐关键分支(尤其 `EmptyAlias` + `NoteTooLong` + `UnknownCategoryReference` + `DuplicateCategoryId`)。非阻断。

### F009 — `deny_unknown_fields` 覆盖确认(未发现遗漏) — Level: Info (confirmatory)
**问题**: grep 得出 5 处 `deny_unknown_fields`:`AppSettings`(settings.rs:23)、`WireDocumentV1`(codec.rs:148)、`WireFolderEntry`(codec.rs:182)、`WireCategory`(codec.rs:228)、`WireTag`(codec.rs:246)——全部 wire 反序列化入口均覆盖,无遗漏。
**影响**: 无。
**建议**: 无需修改。已确认。

### F010 — 验证在 encode 与 decode 两侧同时运行 — Level: Info (confirmatory)
**问题**: `encode`→`validate_current_document`(92-98),`decode`→`migrate_to_current`→`validate_current_document`(122-127)。两侧都调用 `AppData::validate_and_normalize`。
**影响**: 无。
**建议**: 无需修改。

## Cross-Cutting Checks

### Correctness
- Field list、验证、别名规范化、tag 去重语义与需求一致;`normalize_aliases` 先 trim 再判空/判长,统一斜率无歧义。
- `validate_and_normalize` 先做别名/tag 规范化再返回 Ok;encode 在克隆副本上执行,调用方原文档不被改写(有意的可靠断言:除非显式调用方要求变异,否则输入不被破坏)。
- `remove_category`/`remove_tag` 的返回值语义(`retain` 后长度变化)正确;在批量删除(重复调用)场景行为一致。
- UUID 序列化格式:`ids.rs` 的 `Uuid` 经 serde `transparent` 序列化为标准 36 字符连字符小写形式(依赖 `uuid` 的 serde 实现,稳定)。
- `DateTime<Utc>` 序列化:`chrono::DateTime` 的 serde 输出为 RFC3339 字符串(如 fixture `2026-09-21T00:00:00Z`),解码一致,round-trip 稳定测试通过（`first == second` 等于逐字节稳定）。
- `next_revision` 使用 `checked_add`,`u64::MAX` 时返回 `RevisionOverflow`,不会回绕。

### Error Handling
- 分类清晰:`InvalidJson` / `UnsupportedFutureSchema` / `MigrationRequired` / `InvalidDocument` / `EncodeFailed`;`schema_version` 仅对 schema 类错误填充。
- 序列化失败映射为 `EncodeFailed`;无法通过反序列化进入后续步骤。
- 错误不携带内容(见 F002 路径);`ValidationErrorKind` 的 Display 文案固定且面向用户可操作。

### Data Safety
- 纯数据层:无 fs、无网络、无删除 API、无扫描。存储路径/JSON/便签内容从未进入错误文本。
- `deny_unknown_fields` 防止未知字段静默丢失——无静默数据丢失路径。

### Privacy & Security
- 无遥测、无上传、无路径/查询日志。
- 匿名化测试 fixture 使用 `SENSITIVE_PATH`/`SENSITIVE_JSON_MARKER`(tests.rs:17-18),且仅在测试内使用——无硬编码真实用户数据。
- 依赖树中新增 `getrandom 0.4.3`(uuid v4 需要)与 `iana-time-zone` 等的许可证均已通过 CI 的 license 审计门禁。

### Windows Behavior
- 纯 codec,无平台 API。Windows 路径不在此层 canonicalize(已授权 M01-A 决定)。路径长度上限 `MAX_PATH_LEN = 32_767` 与 Windows 长路径语义兼容,但不作平台校验——留给 M01-B。

### Test Coverage
- 枚举 20 个测试:`settings_defaults_match_product_baseline`、`settings_outside_allowed_range_are_rejected`、`whitespace_only_folder_name_is_rejected`、`duplicate_tags_are_deduplicated_in_first_seen_order`、`aliases_are_normalized_and_invalid_manual_weight_is_rejected`、`favorite_limit_is_enforced`、`pinned_and_favorite_are_independent_persisted_fields`、`unknown_or_duplicate_references_are_rejected`、`removing_category_and_tag_only_clears_references`、`json_round_trip_is_stable_and_preserves_unicode`、`invalid_or_truncated_json_is_distinguished_without_content_leakage`、`unknown_fields_are_rejected_without_silent_data_loss`、`future_schema_is_rejected_without_defaulting`、`legacy_schema_requires_explicit_migration`、`invalid_revision_and_overflow_are_rejected`、`validation_errors_do_not_expose_sensitive_path_or_json`、`domain_does_not_expose_real_directory_delete_api`。
- 覆盖正常路径、各错误分类、round-trip 稳定性、Unicode(中文+emoji)、fav 上限 5、pinned/favorite 独立性(含 10 pinned 不触发 fav 上限、6 fav 仍拒绝)、空 display_name、去重、迁移/未来 schema、溢出、权限保护。
- 缺口见 F007/F008(低严重度,边界与分支补测)。

### Performance
- 编解码为 O(n × field)线性;O(n²)仅出现在严重重复输入的 `contains` 去重(个人规模可忽略,见 F003)。`to_vec_pretty` 比紧凑编码略大,但稳定性受测试保护,且数据量小。
- 无其他可感知热点;M02 10k 基准不属于本里程碑。

### Accessibility
- 本轮无 UI/可访问性影响。

### Maintainability
- 模块边界清晰:domain 为纯业务规则,storage 只有 codec/schema/tests;`SettingsRepository` trait 保留为 M01-B 扩展点。常量集中且有语义命名。缺少 crate 级 `#![deny(...)]` 或 `#![warn(...)]`——按 `-D warnings` CI 门禁,当前无警告。
- 无 `#![allow(...)]` 或 `#[allow(...)]` 出现在新模块(排除 `cfg_attr`);CI clippy `-D warnings` 通过,无抑制警告。

## Additionally Requested Checks
- **Round-trip stability** `encode(decode(encode(d))) == encode(d)`:测试 `json_round_trip_is_stable_and_preserves_unicode` 断言 `first == second`(tests.rs:286-296)。
- **UUID format stability**:`uuid` serde 透明序列化,标准 canonical 字符串,tests fixture 覆盖 `from_uuid` ↔ 解码往返。
- **DateTime RFC3339 stability**:见 Correctness。
- **deny_unknown_fields 全量覆盖**:见 F009。
- **两侧验证**:见 F010。
- **pinned 独立性回归**:见「Pinned Restore Verification」。
- **`libredox 0.1.24 → 0.1.25`**:补丁级升级;`libredox` 由 winit 的 `orbclient 0.3.55` 依赖(Redox 专用),Windows 编译链路不生效,补丁更新为无害。确认。
- **无 `#![allow(...)]`**:确认。
- **SENSITIVE 标记仅测试使用**:`SENSITIVE_PATH`/`SENSITIVE_JSON_MARKER` 只出现于 tests.rs,且在「不泄漏」断言中使用;`src/diagnostics.rs` 的 `SENSITIVE_FIELDS_REDACTED` 为既有 M00 常量,不在 M01-A 改动范围。

## Pinned Restore Verification (b6ba57e)
历史草稿曾删除 `pinned`;本 head 在 `b6ba57e` 恢复。逐一核对:
- **domain**:`folder.rs:65` 新增 `pub pinned: bool`。✔
- **wire**:`codec.rs:190` 新增 `pinned: bool`,`codec.rs:210` `into_domain` 传递 `pinned`。✔
- **fixture**:`tests.rs:42` 设置 `pinned: true`。✔
- **测试**:新增 `pinned_and_favorite_are_independent_persisted_fields`(tests.rs:179-239),覆盖 4 组合分:(fav=true,pin=true)、(pin=true,fav=false)、(pin=false,fav=true)、10 个 pinned+fav=false 仍有效、6 个 fav 仍拒绝。✔
- **其余构造点**:`FolderEntry` 仅 `domain/folder.rs`(定义)、`storage/codec.rs`(构造)、`storage/tests.rs`(fixture)四处;无遗漏导致编译失败(CI 通过)或逻辑不一致。
- 结论:恢复完整,`pinned` 与 `favorite` 独立持久化,容量逻辑互不干扰。不视为当前缺陷。

## 未能自动验证的 GUI/平台项
- M01-A 为纯 codec/数据模型里程碑,无 UI 交互;无需要真实桌面验证的新增项。
- 沿用 M00 已记录的桌面边界(托盘、IME、DPI、Explorer 重启等)在本里程碑不改动,归属后续里程碑/手工验收范围。

## Verdict

`CHANGES_REQUESTED`

### 阻断性判断
未发现 `Critical`/`High` 级缺陷;各项需求映射均 PASS。唯一被记为需在 **response 中处理** 的是测试覆盖缺口:
- **F007** 设置范围边界测试缺失(min−1 / min / max / max+1);
- **F008** 多个验证分支无直接单测(`EmptyAlias`、`AliasTooLong`、`NoteTooLong`、`CategoryNameTooLong`、`TagNameTooLong`、`DuplicateCategoryId`、`DuplicateTagId`、`UnknownCategoryReference`)。

### 理由
F007/F008 为 Low 级覆盖缺口,不影响当前代码正确性(校验逻辑经代码核对与 CI 门禁验证),但按 CLAUDE.md §4.4「不得因为可能不重要而省略」及里程碑闭环惯例,要求 implementation agent 提供 response,明确逐条回应并在可行时补测。F001-F006、F009-F010 为 Low/informational,无需代码修改;若 reviewer 接受其理由,可整体推进至里程碑批准。**本评审不为发布授权**(`APPROVED_FOR_RELEASE` 不属于 M01 评审范围)。
