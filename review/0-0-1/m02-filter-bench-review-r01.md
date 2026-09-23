# Review: M02-B 筛选 / 空查询策略 / 10k 基准（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m02-filter-bench`
- **轮次：** `r01`
- **日期：** 2026-09-23（本地时间线；CI 时间为 2026-09-23T10:30–10:37Z）
- **Reviewer：** 独立 code-review agent（M02-B r01）
- **独立性声明：** 本 reviewer 未参与 M02-B 实现 commit `2d37541`，也未参与 M02-A 实现 commit `18c48f5`、M02-A 修复 `eaba66d`、M02-A/N 的 response 撰写或任何 CI 监控。`src/search/` 的全部历史为 `18c48f5`、`eaba66d`、`2d37541` 三个代码提交，reviewer 未以任何形式（含会话压缩/重命名/换名字）实现过其中代码。本次仅作只读评审：未编辑源代码、未运行任何 `cargo` 命令（严格依照 CLAUDE.md §3.1 未执行本地 Rust 验证）、未提交或推送；除本 review 审计文档外未写入其他文件。
- **Base SHA（M02-A approval head）：** `377186c`（`377186cd0a0a2095f2b143d02f1c32c02db01e7`）
- **Head SHA（评审对象）：** `2d37541`（`2d37541446a0a2095f2b143d02f1c32c02db01e7`，消息 `feat: add search filters, empty-query strategies and 10k benchmark`）
- **比较范围：** `377186c..2d37541`
- **审查文件（代码）：** `src/search/{filter,empty_query,generation,benchmark}.rs`（全新）、`src/search/{mod,scoring,search_entry,tests}.rs`（修改）、`src/domain/settings.rs`（修改）、`src/storage/tests.rs`（修改）、`.github/workflows/benchmark.yml`（全新）。
- **审查文件（审计/上下文）：** `review/0-0-1/m02-search-review-r01.md`、`review/0-0-1/m02-search-review-r02.md`（M02-A 已批准结论与既有约束）、`task/01-里程碑任务清单.md`（M02.3/M02.4/M02.5 验收点）、`src/domain/folder.rs`、`src/domain/document.rs`、`src/domain/settings.rs`、`src/storage/codec.rs`、`.github/workflows/ci.yml`。
- **审查方法：** ① 完整阅读 M02-B 新增 4 文件与 5 个修改文件的 head 全量源码与测试；② 核对 `377186c..2d37541` 完整 diff 与统计；③ 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询并核验两个 CI run 的 head SHA、workflow、单 job 全部步骤与 `test result` 日志行；④ **独立从 run `35849214213` job `107142618963` 的日志提取权威 MSVC BENCH 行**（`gh run view --job <jobid> --log`），与 implementation 报告的本地 GNU 数字对照；⑤ 对收藏区上限、截断时序、筛选语义、Accessibility 四态、NoResultReason 三态做逐项代码推演；⑥ `git grep` 质量检查（`#[allow]`、HashMap/HashSet、std::time/fs/net/env、unsafe、测试计数）。未以 implementation 的 response/summary 代替代码核验。

## CI 证据

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35849214017](https://github.com/yorelll/quickfolder/actions/runs/35849214017) | `2d37541446a0a2095f2b143d02f1c32c02db01e7` | `Windows CI` / `fmt, clippy, test, release, package` | `success`（completed，push event） | `gh run view`：conclusion=success，head SHA 与 `git rev-parse HEAD` 一致；单 job 全部步骤 success：`Check formatting`（`cargo fmt --all -- --check`）、`Run Clippy with warnings denied`（`cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-msvc -- -D warnings`）、`Run tests`、`Build release executable`（MSVC）、`Verify release outputs and version helper`、`cargo-deny`、`cargo-about`、`Build portable development artifact`、`Upload portable development artifact`。`Run tests` 日志实测：`test result: ok. 155 passed; 0 failed; 1 ignored`（lib，其中 1 ignored = 10k 基准 `ten_k_release_benchmark_stays_under_lenient_bound`，由 `--ignored` 之外正常测试静默跳过）；`test result: ok. 1 passed`（main bin `filego-version`）；另有 2 个 doc-test 二进制 `0 passed`。**lib 156 + main 1 = 157 项测试全部通过/忽略符合预期，0 失败。** |
| [35849214213](https://github.com/yorelll/quickfolder/actions/runs/35849214213) | `2d37541446a0a2095f2b143d02f1c32c02db01e7` | `Search benchmark` / `release 10k benchmark` | `success`（completed，push event） | `gh run view`：head SHA 一致；单 job 步骤全部 success：`Install pinned Rust toolchain`（1.92.0 + MSVC target）、`Require checked-in Cargo.lock`、`Run release-mode 10k search benchmark`、`Upload benchmark summary log`。**测试日志实测**：`cargo test --release --workspace --all-features --locked --target x86_64-pc-windows-msvc search::benchmark::tests::ten_k_release_benchmark_stays_under_lenient_bound -- --ignored --exact --nocapture`；`test result: ok. 1 passed; 0 failed; 155 filtered out`；**且实际输出 5 行 BENCH**（见下，「否则 throw」守卫未触发）。 |

### 权威 MSVC BENCH 数字（reviewer 独立从 run 日志提取）

以下为 `gh run view --job 107142618963 --log` 实测的 5 行 BENCH，两者为同一行（先由 `Tee-Object` 写入 bench.log 的着色回显，再在 `$summary` 处由 GitHub Actions 白底记录）：

| 查询类 | MSVC median | MSVC p95 | MSVC max | 本地 GNU（implementation 报告）median/p95 |
|---|---|---|---|---|
| empty-query-default | 0.81 ms | 0.85 ms | 0.87 ms | 0.58 / 0.66 |
| pinyin-heavy | 71.11 ms | 75.83 ms | 82.63 ms | 43.46 / 48.21 |
| english-initials | 58.62 ms | 59.75 ms | 62.44 ms | 36.90 / 38.42 |
| edit-distance | 65.34 ms | 67.21 ms | 68.83 ms | 43.57 / 124.59 |
| multi-token | 54.25 ms | 54.77 ms | 54.99 ms | 100.07 / 114.52 |

- **已证实：** workflow 确实从 `cargo test --release` 的 `--nocapture` 输出中取出 BENCH 行（`$summary` 非空 → 未触发 `throw`），即 `LENIENT_MEDIAN_BOUND_MS = 500ms` 判定对所有 5 类成立；这 5 个数字是 **MSVC ABI + hosted runner** 的权威证据，直接产出并记录于日志与 artifacts。
- **数字差异（本地 GNU vs MSVC）：** 两者为不同机器/ABI/CPU 的墙钟数，不可直接等同。MSVC hosted runner 的多 token（54.25）与 edit-distance p95（67.21）低于本地 GNU 报告值（100.07 / 124.59）；MSVC 的 pinyin/english-initials/edit-distance median 高于本地 GNU。**implementation 报告仅以本地 GNU 数字作为基准证据，未透出 workflow 自有 MSVC BENCH 行** —— 已由本 reviewer 独立补齐（见 F001）。
- **`<50ms` 产品目标：** 5 类中 4 类（pinyin 71.11 / english-initials 58.62 / edit-distance 65.34 / multi-token 54.25）在 **MSVC hosted runner** 上超过 50ms，仅 empty-query 大幅低于 50ms。按 `task/01` M02.5 的显式界定（「目标单次 <50 ms」由真实机器 M07.3 复核；「CI 阈值留合理噪声空间；硬产品目标由真实机器验收」），这不阻断 M02-B，但必须如实记录为 M07 基线与优化输入（见 F005）。
- **MSVC 门禁：** `benchmark.yml` 顶层 `TARGET: x86_64-pc-windows-msvc`（`benchmark.yml:26`）、toolchain `targets: x86_64-pc-windows-msvc`（`42`）、命令 `--target x86_64-pc-windows-msvc`（`59`），与 `ci.yml` 口径一致（ci.yml 未变）。`ci.yml` 的 MSVC 门禁（fmt/clippy -D warnings/157 项测试/release build/EXE 与版本核对）

  由 run `35849214017` 全绿继续成立。**benchmark.yml 是纯增量 workflow，不替换、不弱化任何既有 MSVC 门禁。**

## 变更范围核验（无 scope creep）

`git diff 377186c..2d37541 --name-status` 精确为 11 个文件：

| 文件 | 类型 | 变更 |
|---|---|---|
| `src/search/filter.rs` | A | +451 筛选矩阵 |
| `src/search/empty_query.rs` | A | +438 空查询策略 + NoResultReason |
| `src/search/generation.rs` | A | +60 查询世代令牌 |
| `src/search/benchmark.rs` | A | +314 确定性 10k 基准 |
| `src/search/mod.rs` | M | +12 模块导出与文档 |
| `src/search/scoring.rs` | M | +7 测试 fixture 补新字段 |
| `src/search/search_entry.rs` | M | +12 新增 `accessibility`/`origin` 字段 + `PartialEq`/`Eq` 派生 |
| `src/search/tests.rs` | M | +7 测试 fixture 补新字段 |
| `src/domain/settings.rs` | M | +26 新增 `EmptyQueryStrategy` 枚举 + 2 个持久字段 |
| `src/storage/tests.rs` | M | +81 设置前后兼容测试 |
| `.github/workflows/benchmark.yml` | A | +72 增量基准 workflow |

`git diff 377186c..2d37541 -- src/domain/folder.rs src/domain/document.rs src/domain/ids.rs src/storage/mod.rs src/storage/codec.rs src/storage/schema.rs Cargo.toml Cargo.lock src/lib.rs src/app.rs .github/workflows/ci.yml`：**零输出**。domain/folder、domain/document、storage codec/schema、Cargo.toml/lock、ci.yml 均未变（**无依赖新增**：基准用 `std::time::Instant`，无 criterion 依赖）。**无 scope creep。**

## 需求/验收标准映射（M02.3 / M02.4 / M02.5）

### M02.3 筛选（`filter.rs`）

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 全部/置顶/最近/分类/标签/可访问/不可访问/本地/网络/移动 | `FilterSet`（`filter.rs:53-62`）：`pinned_only`、`recent`、`category: Option<String>`、`tags: Vec<String>`、`accessibility: Option<Accessibility>`、`origin: Option<Origin>`；`Accessibility {Unknown,Checking,Accessible,Inaccessible}`（`35-40`）、`Origin {Local,Network,Removable,Unknown}`（`45-50`）。`apply` 为单次 O(n) 扫描返回源序索引（`76-84`） | **PASS** |
| 组合条件 AND；同一多选维度 OR/AND 明确 | 维度间 AND（`matches` 任一维失败即排除，`86-132`；测试 `dimensions_are_anded_across_all_axes`）。**tags AND-within**（`98-102` 每 tag 都必须存在，缺一排除；测试 `tags_are_and_within_and_exclude_partial`）——符合 `task/01`「0.0.1 标签组合采用 AND 语义」。**category 单值**：`category: Option<String>` 单值，模块文档明确「OR-within collapses to equality in this shape」（`12-15`）。分类多选非 M02.3 的「多选维度」要求项（任务的多选 OR/AND 决策点单指标签，见 `task/01` M02.3 第 2 条）；未来多分类选择器须用 OR-within，已记录于文档。**此取舍可接受**（见对抗项 3） | **PASS（含文档化取舍）** |
| 清除单项/全部 | `FilterSet::is_empty`（`66-73`）空筛选=全部；单项清除机制（`pinned_only` 置 false、tags 清空等）由 UI 层构造新的 FilterSet 实现。任务明确该项属 M03/M05 presenter 范围，M02-B 提供语义原语 | **PASS（范围界定）** |
| 记住上次筛选按设置控制 | `remember_last_filter: bool` 持久字段，`#[serde(default)]`，默认 false，`AppSettings::default`（`settings.rs:63-64,86`）；未接入 UI（文档明示「not wired to any UI yet」），属设置层 hook | **PASS（hook 已备，UI 待 M03）** |
| Unknown 不错误归为不可访问；四态明确 | `Accessibility::Inaccessible` 仅收 confirmed-inaccessible（`107-109`）；`Accessible` 收 `Accessible | Unknown`（`117-123`）——未检查路径不被永久排除；`Unknown`/`Checking` 为精确态过滤（`111-115`）。测试 `accessibility_inaccessible_keeps_only_inaccessible` / `accessibility_accessible_keeps_accessible_and_unknown` / `accessibility_unknown_and_checking_are_exact_filters` | **PASS** |

### M02.4 空查询和限制（`empty_query.rs`）

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 默认先 ≤5 条收藏快速访问区，再置顶+最近 | `EmptyQueryStrategy::{FavoritesFirst(默认),All,PinnedOnly,Blank}`（`settings.rs:25-36`）。`favorites_first_indices`（`empty_query.rs:106-147`）：先收藏区（`kept.len() == MAX_FAVORITES` 上限，`112`），再置顶非收藏（`121-127`）、再最近非收藏（`128-134`）、再其余（`138-144`）；`selected` 位图保证每项至多出现一次，O(n)。`MAX_FAVORITES` 常量来自 `domain::folder.rs:15` 且为 5 | **PASS** |
| 支持全部/仅置顶/保持空白；收藏区不超 5，超额置顶仍可搜索 | 四种策略分支（`86-94`）：`Blank`→空、`PinnedOnly`→仅置顶、`All`→全部、`FavoritesFirst`→收藏优先。置顶非收藏项在收藏区后被列出（从不丢失，测试 `favorites_first_lists_pinned_not_favorite_after_favorites` + `favorites_are_capped_at_five_and_remaining_entries_stay_listed`） | **PASS** |
| 最大结果默认 8，先全量评分排序再截断 | `DEFAULT_MAX_RESULTS = 8`（`settings.rs:5`）。**空查询路径**：`empty_query_candidate_indices` 先对全量候选排序（`84`）→ 构建完整收藏/置顶/最近列表 → `empty_query` 再 `take(max_results)`（`198-199`）。**非空路径**：`search_with_filter` 落入 `super::search`，后者对全部匹配 `sort_by`（`mod.rs:76-82`）后再 `.take(settings.max_results)`（`86`）。均先全量排序后截断，非 take-first-N。测试 `empty_query_applies_filter_and_truncates_after_full_sort` | **PASS** |
| 无结果可本地化原因，不把筛选无结果误当无数据 | `NoResultReason::{NoData,FilteredOut,NoMatch}`（`30-39`）。`NoData`：`entries.is_empty()`（`empty_query.rs:174-179`、`search_with_filter:226-231`）。`FilteredOut`：filter 排空（`181-187`、`233-239`）或策略（PinnedOnly 无置顶）排空（`201`）。`NoMatch`：非空查询无命中（`246`）。三态互斥且均可达；测试 `no_result_reason_distinguishes_no_data_filtered_out_and_no_match`；`Blank` 策略有意为空、不设原因（`189-194,428-437` 测试断言 `no_result_reason=None`） | **PASS** |

### M02.5 性能与取消（`generation.rs` + `benchmark.rs`）

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 确定性 10,000 条 fixture，含中英文/UNC/标签 | `deterministic_fixture(FIXTURE_SIZE=10_000)`（`benchmark.rs:43,101-174`）：纯常量+索引算术构造（无 RNG/fs/time），含中文显示名+拼音别名、UNC 长路径、标签、分类、可访问性/来源分布、恰 `MAX_FAVORITES` 条收藏。`fixture_is_deterministic_and_representative` 用 `SearchEntry` 的 `PartialEq` 断言两次构建**位级相同**（`249-251`）并抽查代表性（中文/UNC/置顶/收藏） | **PASS** |
| release-mode benchmark 测量核心；目标单次 <50ms | `#[ignore]` 单测（`271`）+ CI 以 `cargo test --release ... -- --ignored --exact --nocapture` 显式运行。5 个查询类（`198-204`）：empty-query（经 `empty_query`）、pinyin-heavy/english-initials/edit-distance/multi-token（经 `search`）——**pinyin 与 edit-distance 成本单独记录**（任务要求）。MSVC 实测见 CI 证据表 | **PASS（测量与成本拆分到位；目标 50ms 由 M07.3 真实机器复核）** |
| 避免首跑/I-O/日志干扰；报告 median/p95 | `WARMUP_RUNS=4`（`44`）排除首次初始化/懒分配；`SAMPLE_RUNS=21`（奇数，median 为真实样本，`45`）；samples `sort_by(total_cmp)` 后取 median（`232`、`219`）、p95（`176-180` 百分位）；`eprintln!("BENCH {label}: ...")`（`276`）逐类输出。fixture 纯内存，无 I/O；`Instant` 仅用于 harness（模块文档 `29-30`） | **PASS** |
| CI 阈值留噪声空间；硬目标真实机器验收 | `LENIENT_MEDIAN_BOUND_MS = 500.0`（`47`），断言每类 median ≤ 500ms（`277-287`）。`benchmark.yml` 为纯增量 job；`<50ms` 真实目标显式移交 M07.3（模块文档 `21-27` + `task/01`） | **PASS（宽松 bound 设计无误）** |
| 查询取消/世代号 | `QueryGeneration`（`generation.rs:11-39`）：单调 u64 计数，`next_generation` 用 `wrapping_add`（release/debug 行为一致，`28-33`），`is_stale`（`36-38`）判定陈旧；零值初始、wrap 语义在文档中说明。测试 `generation.rs:45-59` | **PASS** |

### Settings 前后兼容

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 新字段 `#[serde(default)]` 与默认值 | `empty_query_strategy`（`settings.rs:59-60`）与 `remember_last_filter`（`63-64`）均 `#[serde(default)]`；默认 `FavoritesFirst`/`false`（`86`）。`EmptyQueryStrategy` 为 `#[serde(rename_all="snake_case")]` + `#[default]`（`23-36`） | **PASS** |
| 旧 schema-v1 JSON 无新字段仍可解码 | `settings_forward_compat_new_fields_default_when_absent`（`storage/tests.rs`）：完整 schema-v1 JSON（无两个新字段）经 storage `decode` 成功，断言 `FavoritesFirst`/`false`。`AppSettings` 域结构带 `#[serde(deny_unknown_fields)]`（`settings.rs:39`，base 已存在）——若无 `#[serde(default)]`，新字段会使旧 JSON 解码失败；两字段均有 default，故前向兼容成立 | **PASS** |
| 新值可解码 | `settings_forward_compat_new_fields_decode_when_present`：`"empty_query_strategy": "pinned_only"` / `"remember_last_filter": true` 可解码并断言。**wire 层**：`WireDocumentV1` 直接复用 `AppSettings` 为 `settings` 类型（`codec.rs:151`），inline 两个新字段的序列化/反序列化，兼容测试经真实 `decode()` 路径验证 | **PASS** |

## 强制对抗性清单逐项结论（decisive verdicts）

### 对抗项 1：收藏区上限 — **PASS**

`favorites_first_indices` 以 `if kept.len() == MAX_FAVORITES { break; }`（`empty_query.rs:112`）硬性钳制收藏区 ≤ 5，使用域常量 `MAX_FAVORITES`（=`folder.rs:15`=5）。测试 `favorites_are_capped_at_five_and_remaining_entries_stay_listed` 构造 **7 条收藏 + 1 条置顶非收藏**（非法数据，防御场景），断言收藏区恰 5 条与总顺序 `[1,2,3,4,5,8]`（`313`），`favorites_before_first_nonfavorite == MAX_FAVORITES`（`312`）。置顶非收藏项（id8）在收藏区之后仍列出（`314`），未丢失。**顺序确定性**：`empty_rank`（`97-104`）= manual_weight 降 → pinned 先 → open_count 降 → id 升，纯函数无哈希/时间依赖。超过上限的收藏（id6,7）在非法输入场景下从常规区一并排除（`121-144` 的 `!entry.favorite` 谓词）——见 F004；合法数据（`document.rs:75` 校验 ≤5）下永不出现。

### 对抗项 2：全量排序后截断 — **PASS**

非空路径：`super::search`（`mod.rs:76-88`）对全部命中 `sort_by(total_score desc → tiebreak)` 后 `.take(max_results)`。空查询路径：`empty_query_candidate_indices`（`84`）先全量 `sort_by(empty_rank)`，`FavoritesFirst` 在完整有序列表上分节构建（不提前截断），最终 `empty_query` 在 `199` 处 `take(max_results)`。两路径皆「全量排序 → 截断」，测试 `empty_query_applies_filter_and_truncates_after_full_sort` 用 max_results=2 验证「先滤全集→排全集→再截」，断言序为 `[2,3]`（open_count 300/200 前两位）而非源序。**PASS。**

### 对抗项 3：筛选语义（tags AND / category 单值）— **PASS（含文档化取舍）**

**tags AND-within**：`filter.rs:98-102` 对每个请求 tag 要求 `entry.tag_names.iter().any(|name| name == tag)`，缺任一即排除——与 `task/01`「0.0.1 标签组合采用 AND 语义」一致；测试 `tags_are_and_within_and_exclude_partial` 断言「缺一 excluded（AND-within）」。**维度 AND**：`matches` 各维独立短路排除，测试 `dimensions_are_anded_across_all_axes`（六维全中才保留）。**category 单值 vs OR-within**：`category` 设计为 `Option<String>` 单值，一个文件夹至多一个分类（域模型 `category_id: Option<Uuid>`），故「OR-within（任一选中分类准入）」在此形状下退化为单值相等；模块文档（`filter.rs:12-15`）明确记录此塌缩与「未来多分类选择器必须用 OR-within」的对比。M02.3 的多选 OR/AND 决策点任务文本仅要求「标签」选定首版规则（已选 AND），未要求分类多选；**分类单值设计与本 slice 需求一致，非缺口**，仅需在 response 确认该取舍已记录（已记录于模块文档）。

### 对抗项 4：Accessibility Unknown 双向语义 — **PASS**

- 滤 `Inaccessible` → **只收** confirmed-inaccessible（`107-109`），Unknown/Checking/Accessible 全排除；测试断言 `vec![4]`（恰为 Inaccessible）。
- 滤 `Accessible` → 收 `Accessible | Unknown`（`117-123`），**Unknown 不因未检查而被永久排除**；测试断言 `[1,3]`（Unknown + Accessible），Checking/Inaccessible 排除。
- `Unknown` 与 `Checking` 是**精确/独立态**（`111-115`），彼此不混入；测试分别断言只回各自条目。
- 全程**无任何位置把 Unknown 自动归类为 Inaccessible**（`matches` 无 fallthrough 归并）。

**PASS（两方向 + 精确态均验证）。**

### 对抗项 5：`recent` 语义 — **PASS（诚实实现，附观察）**

`recent` 为纯谓词 `entry.last_opened_at.is_some()`（`filter.rs:90-92`），无时钟读取；模块文档（`21-23`）明确「Recent is the later slice of already-opened entries, not a wall-clock cutoff」，与实现完全一致，**无「recent = 全部」的虚假声明**（有 `last_opened_at=None` 的条目被排除，测试 `recent_keeps_entries_with_a_last_opened_at` 断言排除从未打开项）。**意义评估**：作为核心层的纯谓词，「最近 = 曾经打开过」是忠实且确定性的实现；但严格说它表达的是「已打开/从未打开」二分，而非时间窗「近 N 天」。更细的时间窗语义（如近 30 天）必然引入墙钟依赖，破坏核心的纯/确定性约束，应在 presenter（M03）层实现与排序。**对该 slice 不构成缺陷**（任务只要求「最近」维度存在，未规定时间窗），记为 F003 观察供 M03 注意。**空查询的「最近使用排序」由 `open_count` 降序表达**（`empty_rank`），非 `last_opened_at`——这是 M02-A 既定 tie-break 的复用，与 README「最近使用」意图一致。

### 对抗项 6：基准完整性 — **PARTIAL（结构完整，一处覆盖缺口 = F002；50ms 目标需 M07，F005）**

- **fixture 确定性**：`deterministic_fixture` 全常量 + 索引算术，无 RNG/fs/time；`fixture_is_deterministic_and_representative` 用 `SearchEntry` 新增的 `PartialEq` 断言两次构建 **bit-identical**（`251` `assert_eq!(first, second, "fixture must be bit-identical across runs")`），并抽查代表性（收藏恰 5、置顶 >1000、UNC 存在、中文存在）。`PartialEq` 派生（`search_entry.rs:52`，含 doc 声明「not part of ranking」）不改变排序行为。
- **gating**：`#[ignore = "release-mode wall-time benchmark; ..."]`（`271`）+ `#[cfg(test)] mod benchmark`（`mod.rs:14-15`）+ CI 显式 `--release ... -- --ignored --exact --nocapture`。debug `cargo test` 仅编译不执行（CI 日志确认 1 ignored 正常跳过）。
- **warmup / 样本 / 统计**：WARMUP_RUNS=4、SAMPLE_RUNS=21（奇数，真实中位样本）、median=`samples[len/2]`（`232`）、p95=分位数内插（`176-180`，测试 `percentile_is_stable_at_endpoints`）、BENCH 逐类 `eprintln!`（`276`）。
- **宽松 bound**：`LENIENT_MEDIAN_BOUND_MS=500.0` 断言 median ≤500（`277-287`），`benchmark.yml:63` 无 BENCH 行即 `throw`（**本次 5 行 BENCH 均被取出，守卫未触发，证明判定真实执行**）。
- **无 criterion 依赖**（Cargo 未变），用 `std::time::Instant`。
- **成本拆分**：pinyin-heavy / english-initials / edit-distance / multi-token / empty-query 五类分别记录（`QUERIES`），满足「单独记录拼音与编辑距离性能成本」。
- **edit-distance p95 尾部（M02-B 数据规模）**：MSVC p95=67.21ms（本地 GNU 报告 p95=124.59ms）。M02-A 已加 `MAX_EDIT_DISTANCE_LEN=64` 护栏（`matching.rs`），防御 32k 路径整键 DP；当前 10k + 6 字符 token（`driber`→`driver` dist 1）的实测在 500ms 宽松界内，且在真实桌面机器（高于 hosted runner 性能）下预计低于此基线。
- **50ms 真实目标裁定（见 F005）**：以**权威 MSVC 数字**为准，4/5 类（pinyin 71.11 / edit-distance 65.34 / english-initials 58.62 / multi-token 54.25）在 hosted runner 上超过 50ms。按 `task/01` 明确范围（50ms 由真实机器 M07.3 复核、CI 宽松界防 hosted 噪声），**这是「documented-defer-to-M07」，不是 M02-B 阻断**；不要求下调 CI bound（hosted 噪声会使紧 bound 假失败）。**对「multi-token 100ms 本地 GNU median vs 50ms 目标」的明确裁定：本地 GNU 100ms 不是权威数字——权威 MSVC median=54.25ms；两者均 >50ms，但按任务分阶段，50ms 达标验证归 M07.3 真实机器，M02-B 提供的基准证据与成本拆分已满足本 slice 验收。** M07 必须把 MSVC 基线（本表）作为优化起点（pinyin 键派生、multi-token 逐 token 匹配、edit-distance 均是候选项）。

### 对抗项 7：设置前后兼容 — **PASS**

见需求映射「Settings 前后兼容」节：两新字段 `#[serde(default)]` + `AppSettings::default` 默认值 + 真实 `decode()` 路径的「absent→默认」与「present→新值」双向测试；`deny_unknown_fields` 在域结构上无碍（新字段均带 default，旧 JSON 缺字段走默认）。wire `WireDocumentV1.settings: AppSettings` 直连域类型，无独立 wire 副本，故无「域加了新字段但 wire 未跟」的学徒风险。

### 对抗项 8：无 scope creep / 无回归 — **PASS**

11 文件精确切分（见「变更范围核验」）；domain/folder、domain/document、storage codec/schema、Cargo.toml/lock、ci.yml 零变更。**SearchEntry 新增 `accessibility`/`origin` 两字段后，所有既有构造点均更新**：`scoring.rs` 测试 fixture（`scoring.rs:268-269` `accessibility: Unknown, origin: Unknown`）、`tests.rs` `make_entry`（`tests.rs:41-42`）、新增文件自己的 fixture；`SearchEntry` 无 `#[derive(Default)]`/Default impl、字段无默认值 → **任何漏更新的构造点是编译错误，编译即证明全量更新（CI 全绿）**；不存在静默默认化改变行为。`PartialEq/Eq` 仅新增派生，不影响排序（排序走 `total_score` + `tiebreak`）。M02-A 的 `search()`/评分/高亮/拼音/toggles 本体**未动**（`git diff` 确认 `search_entry.rs` 仅加字段；`scoring.rs` 仅测例 fixture 加字段），tie-break 方向修复（M02-A r02 关闭项）保持。M02-A 全部 133 项测试在 M02-B head 继续通过（CI 157 项 0 失败，其中 M02-A 测试未删除、未弱化）。

### 对抗项 9：workflow — **PASS（纯增量）**

`benchmark.yml`：MSVC target 三处一致（`26/42/59`）、`--locked`（`59`）、pinned toolchain `1.92.0`（`41`）、release + `--ignored --exact --nocapture`（`59-61`）、无 BENCH 行即 `throw`（`63`）、只读权限（`contents: read`，`21-23`）、上传汇总 artifacts 到 Actions（`66-72`）。**只新增 job，不触碰 ci.yml**（diff 确认 ci.yml 零变更），MSVC 门禁不被弱化。运行时入参（toolchain、--locked、filter 命令）与 ci.yml 一致。

### 对抗项 10：确定性 / 无 allow / 隐私 — **PASS**

`git grep` 于 `src/search/`：零 `HashMap/HashSet/BTreeMap/BTreeSet`（filter/empty-query/generation 均用 `Vec` + 位图 `selected`），零 `#[allow]` 与 `unsafe`；`std::time::Instant` 仅出现于 `benchmark.rs:32`（`#[cfg(test)]` harness，模块文档明示仅供基准墙钟）；`filter.rs`/`empty_query.rs`/`generation.rs` 零 `std::fs/net/process/env/thread`。无用户路径/搜索词写入日志（BENCH 只输出计时与 label）；核心无 fs/网络/时间（`recent` 明确不读时钟）。**确定性**：排序键均纯函数，`empty_rank` 与 `tiebreak` 无哈希序依赖，fixture bit-identical 测试佐证。

### 对抗项 11：NoResultReason 正确性 — **PASS**

三态互斥且真实可达（详见 M02.4 映射）：`NoData`（无任何条目，空查询与非空查询两条路径均覆盖）、`FilteredOut`（filter 排空 **或** PinnedOnly 策略无置顶）、`NoMatch`（条目存在但非空查询无命中）。`Blank` 策略有意空、原因 `None`（不含糊为错误）。filter 先于匹配应用（`search_with_filter:233-239`）保证「筛选把全部候选排掉」报 `FilteredOut` 而非 `NoMatch`——语义正确。

## 按严重级排序的 Findings

| ID | 严重级 | 文件:行 | 问题 | 影响 | 证据/复现 | 建议 |
|---|---|---|---|---|---|---|
| **F001** | **Low（证据卫生 / 记录）** | `benchmark.yml`（run `35849214213` log）vs implementation 报告 | **implementation 报告仅以本地 GNU release 数字作为 10k 基准证据，未从 workflow 日志提取并记录其自有 MSVC BENCH 行。** 本 reviewer 独立从 `gh run view --job 107142618963 --log` 取出权威 MSVC 数字：empty 0.81/0.85、pinyin 71.11/75.83、english-initials 58.62/59.75、edit-distance 65.34/67.21、multi-token 54.25/54.77（median/p95）。其中 multi-token 与 edit-distance p95 与 GNU 报告值（100.07/114.52、43.57/124.59）**方向相反**——GNU 不能代表 MSVC 宿主。 | 报告口径可能误导后续里程碑对真实性能基线的判断；CLAUDE.md §3.1 明确 GNU 只是快速反馈、「绝不替代远程 MSVC CI」。不改变代码正确性与 CI 结论（两 run 均 success），但证据记录必须修正。 | `gh run view --job 107142618963 --log` 的 5 行 BENCH（见 CI 证据表）；implementation 报告含本地 GNU 数字而未含这些 MSVC 行。 | response/记录以本 review 表中的 **MSVC 数字为权威基准**，本地 GNU 仅作本地快速反馈留存；把 MSVC BENCH 行（含 artifacts `search-benchmark-log-<sha>`）链接进 response。 |
| **F002** | **Medium（基准覆盖缺口）** | `benchmark.rs:190-194`（`run_query`）vs `empty_query.rs:233-245`（`search_with_filter` 的克隆） | **5 个基准类中非空查询全走 `super::search(entries, ...)` 直连（`benchmark.rs:193`），没有一个类走 M02-B 新增的 `search_with_filter`（filter 维度非空时会把过滤后的条目整体 `clone()` 成新 `Vec<SearchEntry>` 再排名，`empty_query.rs:241-244`）。** 空查询类虽走 `empty_query`（无克隆，`apply` 只回索引），但「筛选 + 克隆 + 排名」的组合路径的克隆成本（10k 个含 String 的 `SearchEntry`，路径最长 32,767 chars）**未被任何基准类度量**。 | M03 presenter 将以 `search_with_filter` 为入口；真实打字查询延迟将包含该 O(n) 克隆（n=过滤后规模）。当前 CI 500ms 宽松界对这条未被测路径**无任何回归防护**；克隆本身非缺陷（API 需物化子集），但「基准未度量其成本」是覆盖缺口。非正确性缺陷。 | `benchmark.rs:193` 直连 `search`，测试 `run_query` 从未调用 `search_with_filter`；`empty_query.rs:241-244` 无条件克隆。 | 增加 1 个基准类以 `search_with_filter` + 真实 filter（如 `origin: Local` 或 `tags`）度量组合路径；或至少在模块文档注明「筛选克隆成本未入基准、由 M07.3 真实机器覆盖」，并在 response 记录该已知限制。 |
| **F003** | **Low（观察）** | `filter.rs:21-23,90-92` | `recent` 为纯「曾经打开过」二分谓词，非时间窗「近 N 天」。 | 与「最近」的直觉语义有差距；无时钟是核心纯/确定性的正确取舍，但 presenter 若不补时间窗排序，「最近」筛选体验偏弱。不影响正确性。 | 模块文档 `21-23` 自述「not a wall-clock cutoff」；空查询的最近排序由 `open_count` 表达而非 `last_opened_at`。 | 记录为 M03 presenter 的已知供给：如需时间窗「最近 X 天」，在 presenter/UI 层实现墙钟排序，核心保持纯函数。 |
| **F004** | **Low（防御路径观察）** | `empty_query.rs:121-144`（`!entry.favorite` 谓词） | 当输入非法地含 >5 条收藏（如测试构造的 7 条）时，超出上限的收藏不仅不进收藏区，还被常规区各节的 `!entry.favorite` 谓词**整体排除出展示序列**（`favorites_are_capped_at_five...` 断言结果恰不含 id6,7）。 | 合法数据（`document.rs:75` 校验 ≤5）不可能触发；仅对 hand-built/非法输入「防御地把超额收藏丢弃」而非「降级到常规区」。已有注释说明（`135-138`），非真实用户可达。 | `empty_query.rs:111-117` 收藏节 `break` 于上限，`121-144` 谓词均 `!entry.favorite` → 超额收藏不落入任何节。 | 可选改进（不阻断）：把超出上限的收藏在常规区末尾列出（与置顶非收藏一致）；或维持现状并在注释中说明「超额收藏在非法输入下被丢弃」的行为边界。 |
| **F005** | **Medium（性能基线 / 移交项）** | `benchmark.rs`（MSVC 实测，见 CI 证据表） | **权威 MSVC hosted-runner 上 4/5 基准类 median 超过 50ms 产品目标**：pinyin 71.11、edit-distance 65.34、english-initials 58.62、multi-token 54.25（仅 empty-query 0.81ms 达标）。 | 按 `task/01` 分阶段界定（50ms 由真实机器 M07.3 复核、CI 宽松 500ms 防噪声），**不是 M02-B 阻断**；但这是 M07 优化清单的直接输入，且在第 10k 数据规模下「单次查询 54–71ms」意味着真实桌面机器上也接近或略超 50ms。若不跟踪，M07 可能无基线可比对。 | run `35849214213` BENCH 行（本 review 表）；本地 GNU 亦呈 100ms（multi-token）。 | 由 response 明确「documented-defer-to-M07」，并把 MSVC median/p95 作为 M07.3 真实机器验收与优化（pinyin 键派生缓存、multi-token 剪枝、edit-distance 护栏复核）的基线记录在案；**不要求下调 CI bound**。 |
| **F006** | **Low（一致性观察）** | `filter.rs:20,103-131` | `origin` 与 `accessibility` 的 Unknown 语义不对称：`Accessible` 显式**包含** Unknown（未检查不排除），而 `Origin::Local/Network/Removable` 与 `Origin::Unknown` **互不匹配**（`entry.origin != origin` 精确相等）。 | 两种语义分别合理（accessibility 未检查是暂时态需保守保留；origin 是静止事实，未知即非本地）。但调用方（M03 UI）需注意「未知来源文件夹在『本地』筛选下被排除」这一点与 accessibility 的宽容不同。 | 模块文档 `20` 已明示该对比；`127-131` 精确相等。 | 无代码修改必要；在模块文档（已含 `20`）基础上确认该不对称被 M03 presenter 理解（可用作 response 中的说明，非修复）。 |

**汇总：** **0 High、2 Medium（F002 基准覆盖缺口、F005 性能基线移交项）、4 Low（F001 证据卫生、F003 recent 观察、F004 防御路径观察、F006 一致性观察）。** 无正确性/数据安全/隐私/回归缺陷。

## Cross-cutting 检查

- **正确性：** 筛选矩阵（AND / tags AND-within / accessibility 四态 / origin）与空查询四策略均经逐项单元测试；收藏区上限、全量排序后截断、NoResultReason 三态可达性均经代码推演 + 端到端测试。`empty_rank` 与前序 M02-A tie-break 方向一致（manual_weight 降 → pinned 先 → open_count 降 → id 升）。无逻辑反转或边界错误。
- **错误处理：** 核心仍无 I/O/错误类型（纯函数设计延续）；`run_query` 释放结果不 panic；`percentile` 对非空 samples 有 `debug_assert`，21 样本下安全；`deterministic_fixture` 的 `from_timestamp` `expect` 为固定常量时间戳（硬件安全）。无新增 unwrap/expect 于生产路径（仅测试/harness）。
- **数据安全：** 筛选/空查询只读 `SearchEntry` 元数据，不探测磁盘、不修改任何用户数据；`document.rs:75` 的 MAX_FAVORITES 校验与 `empty_query` 防御上限一致。无真实目录删除面。
- **隐私：** 无路径/搜索词写入日志（BENCH 仅计时与 label）；无网络/遥测；filter 不过查询真实路径状态（accessibility/origin 由 presenter 提供）。
- **安全性：** 无新依赖（Cargo/lock 未变）、无 unsafe、无攻击面增量；generation 令牌用 wrapping 算术无 UB。base 上 cargo-deny/cargo-about 门禁在 head 继续全绿（run 35849214017）。
- **Windows 行为：** 无平台 API；路径仅字符串比较（正确边界）；MSVC CI（含 cargo-deny/cargo-about/portable artifact）全绿；benchmark.yml 新增 MSVC release job 全绿。
- **测试覆盖：** lib 156（155 passed + 1 ignored）+ main 1 与 CI 日志吻合；新增 filter 11 + empty_query 6 + generation 1 + benchmark 4 + storage 2 = 24 项（base lib 132 → head 156），静态计数与 CI 一致。关键必测全在：收藏上限、置顶非收藏、四策略、筛选组合、四态 accessibility、origin、NoResultReason 三态、fixture 确定性、percentile 端点、generation 陈旧判定、设置前后兼容。**缺口：** F002（`search_with_filter` 组合路径未入基准）。
- **性能：** 筛选 O(n) 单遍；空查询排序 O(n log n) 一次；克隆（F002）未被基准确认量级；MSVC 4/5 类 >50ms（F005）移交 M07。
- **可访问性 / 可维护性：** 无 UI 变更；模块文档（`filter.rs`、`empty_query.rs`、`benchmark.rs`、`generation.rs`）语义先行、对取舍（Unknown 宽容、OR-within 塌缩、无时钟 recent、500ms 界）均有明确记录；测试按「筛选/策略/原因/世代/基准」分节命名清晰。`MAX_FAVORITES`/`LENIENT_MEDIAN_BOUND_MS`/`FIXTURE_SIZE` 等常量集中声明。
- **依赖与许可证：** 零新依赖；benchmark.yml 不引入 crate；cargo-deny/cargo-about 门禁继续通过。

## 未能自动验证的项

1. **真实机器 <50ms（M07.3）**：MSVC hosted-runner 4/5 类 >50ms（F005），但真实桌面机器性能必须手工验收；基准提供了可复现基线与宽松 500ms 回归界。
2. **真实 ABI/硬件差异**：本地 GNU 与 MSVC hosted 数字差异显著（尤其 multi-token 100.07 vs 54.25、edit-distance p95 124.59 vs 67.21），需以真实桌面为最终判据。
3. **`remember_last_filter` / `empty_query_strategy` 的 UI 接线**：本 slice 仅持久化 hook + 核心语义，未接入任何 presenter（M03/M05 范围）。
4. **真实筛选交互**（checkbox 组合、清除按钮、筛选数量徽标）为 UI 层验收；M03 需正确消费 `FilterSet::is_empty` 与 `SearchResponse`。
5. **pinyin 首读音语义**（M02-A 已记录的 Deterministic 限制）延续，仍待真实桌面/用户验收。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M02-B 筛选 / 空查询 / 10k 基准；非发布批准）

- **范围与独立性：** 11 文件精确切分；domain/folder、domain/document、storage codec/schema、Cargo.toml/lock、ci.yml 零变更；无新依赖。`src/search/` 全部历史 = 3 个代码提交（M02-A 实现 + M02-A 修复 + M02-B），reviewer 对三者均未参与。
- **CI：** 两个 run 均以 head `2d37541` 全绿：Windows CI `35849214017`（fmt / clippy `-D warnings` / **157 项测试通过 0 失败（lib 155+1 ignored，main 1）** / MSVC release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact）+ Search benchmark `35849214213`（MSVC release `--release --ignored` 1 passed 0 failed，**并实测输出 5 行 BENCH，`throw` 守卫未触发**）。
- **权威基准数字（reviewer 从 run 日志独立提取，MSVC）：** empty 0.81/0.85、pinyin 71.11/75.83、english-initials 58.62/59.75、edit-distance 65.34/67.21、multi-token 54.25/54.77 ms（median/p95）；均 ≤500ms 宽松界。**明确裁定：multi-token 的本地 GNU 100ms 非权威值，权威 MSVC median=54.25ms；4/5 类超 50ms 产品目标属 task 分阶段界定的「documented-defer-to-M07」，非 M02-B 阻断，也不要求下调 CI bound。**
- **Findings：** **0 High、2 Medium（F002 基准未覆盖 `search_with_filter` 的筛选+克隆组合路径；F005 MSVC 4/5 类 >50ms 需作为 M07 基线跟踪）、4 Low（F001 报告只引 GNU 未透 MSVC BENCH；F003 recent=曾经打开二分观察；F004 超额收藏在非法输入下被整体排除的防御路径观察；F006 origin/accessibility 的 Unknown 语义不对称一致性观察）。**
- **需求映射：** M02.3 筛选矩阵 PASS（含 category 单值 vs OR-within 的文档化取舍）、M02.4 空查询 PASS（收藏上限/截断时序/NoResultReason 三态）、M02.5 基准与世代 PASS（确定性 fixture、release bench、宽松 bound、成本拆分、取消令牌）。Settings 前后兼容 PASS。
- **处置要求（不阻断批准，随 response 一并关闭）：**
  1. **F001**：response 必须以本 review 的 **MSVC 数字**为准记录基准证据（链接 run 与 BENCH artifacts），本地 GNU 仅作本地快速反馈。
  2. **F002**：在 benchmark 增加 `search_with_filter` 组合路径类，或至少在文档/response 记录「筛选克隆成本未入基准，由 M07.3 覆盖」，二选一。
  3. **F003/F004/F006**：作为已知限制/观察记录于 response（无代码修复要求）。
  4. **F005**：显式记录「documented-defer-to-M07」，附 MSVC 基线供 M07.3 比对与优化。
  - 按 CLAUDE.md §4.5，本结论为 `APPROVED_FOR_MILESTONE`；**不构成 `APPROVED_FOR_RELEASE`**。发布需后续独立发布评审、真实桌面手工验收与对应响应闭环（含「未能自动验证的项」）。
