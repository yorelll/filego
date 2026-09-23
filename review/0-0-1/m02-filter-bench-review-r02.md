# Review: M02-B 筛选 / 空查询策略 / 10k 基准（Round 02 复审）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m02-filter-bench`
- **轮次：** `r02`（对 r01 review + response + 代码修复的复审）
- **日期：** 2026-09-21（本地时间线；CI 时间为 2026-09-23T11:35–11:45Z）
- **Reviewer：** 独立 code-review agent（M02-B r02 复审）
- **独立性声明：** 本 reviewer 未参与 M02-B 实现 commit `2d37541`、未参与 r01 之后修复 commit `ba8bd8d` 的编码、未参与 `m02-filter-bench-response-r01.md` 撰写、未参与任何 CI 监控与 run 触发。`src/search/` 的全部历史仍为 M02-A 两个代码提交（`18c48f5`、`eaba66d`）与 M02-B 两个代码提交（`2d37541`、`ba8bd8d`），reviewer 对四者均未以任何形式（含会话压缩/重命名/换名字）参与实现。本次仅作只读评审：未编辑源代码、未运行任何 `cargo` 命令（严格依照 CLAUDE.md §3.1 未执行本地 Rust 验证）、未提交或推送；除本 review 审计文档外未写入其他文件。
- **Base SHA（r01 head）：** `2d37541`（`2d37541446a0a2095f2b143d02f1c32c02db01e7`，`feat: add search filters, empty-query strategies and 10k benchmark`）
- **修复 commit SHA：** `ba8bd8d`（`ba8bd8da5561278eef9e2811308ba61e06116c57`，`feat(bench): cover the filter-and-clone search path`）— F002 代码修复
- **Head SHA（复审对象 / 里程碑 head）：** `3829525`（`382952563e20e5eaa19d0691138a2514a92df519`，`docs: finalize M02-B response with MSVC evidence`）— 文档回填
- **比较范围：** `2d37541..3829525`（git 图：`2d37541 → 607659c（docs: record r01）→ ba8bd8d（代码修复）→ 3829525（docs: finalize）`）
- **审查文件（代码）：** `src/search/benchmark.rs`（+106 / −11，F002 新增第 6 基准类 + 非退化测试 + label 透传）
- **审查文件（审计/上下文）：** `review/0-0-1/m02-filter-bench-review-r01.md`（r01 结论与全文）、`review/0-0-1/m02-filter-bench-response-r01.md`（r01 response，逐条 F001–F006）、`src/search/empty_query.rs`（`search_with_filter` 实现校验）、`src/search/filter.rs`（`FilterSet`/`Origin`）、`.github/workflows/benchmark.yml`、`.github/workflows/ci.yml`、`task/01-里程碑任务清单.md`（M02.3/M02.4/M02.5）为上下文。
- **审查方法：** ① 完整阅读 head `3829525` 的 `src/search/benchmark.rs` 全量源码与测试，以及 base `2d37541` 版本对照；② 核对 `2d37541..3829525` 完整 diff 与统计（3 文件）；③ 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询并核验 4 个 CI run 的 head SHA、workflow、结论与关键日志行；④ **独立从 run `35855426323` job `107162638889` 日志提取权威 MSVC BENCH 行**（含新增 `filtered-with-clone`），与 response 承诺「供 r02 reviewer 独立提取」的方式一致（与 r01 相同：`gh run view --job <jobid> --log`）；⑤ 对 F002 新增类做逐项代码推演（是否真实走 `search_with_filter` 组合路径、子集非退化、输出截断、bound 覆盖）；⑥ `git grep` 质量检查（`#[allow]`、HashMap/HashSet、Instant 仅 harness、io/thread 不出现、fixture 确定性）。未以 implementation 的 response/summary 代替代码核验。

## CI 证据（reviewer 独立核验）

`gh run list --branch feature/m00-foundation` 实测，以下 4 个 run 均为 push event、`completed`、`success`，head SHA 与 `git rev-parse` 逐字一致：

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35855426114](https://github.com/yorelll/filego/actions/runs/35855426114) | `ba8bd8da…6157c` | `Windows CI` / job `107162640090` | `success` | `gh run view`：head SHA 一致；`Run tests` 日志实测 `test result: ok. 156 passed; 0 failed; 1 ignored`（lib，其中 1 ignored = 10k 基准，正常 debug 跳过）+ `test result: ok. 1 passed`（main `filego-version`）+ 2 个 doc-test `0 passed`；**新增测试 `filtered_with_clone_keeps_a_meaningful_subset ... ok` 与 `query_classes_cover_pinyin_initial_edit_multi_token_and_filtered_clone ... ok` 均出现在日志**。157 passing + 1 ignored，0 失败。 |
| [35855426323](https://github.com/yorelll/filego/actions/runs/35855426323) | `ba8bd8da…6157c` | `Search benchmark` / job `107162638889` | `success` | `gh run view`：head SHA 一致；`Run released-mode 10k search benchmark` 日志实测：`test result: ok. 1 passed; 0 failed; 156 filtered out`；**实测 6 行 BENCH（含新增 `filtered-with-clone`），`$summary` 守卫（`if ($null -eq $summary) { throw ... }`）未触发**（重复两遍为 `Tee-Object` 写入 bench.log 的着色回显 + 最终 `$summary` 白底回显，与 r01 观察一致）。 |
| [35856136080](https://github.com/yorelll/filego/actions/runs/35856136080) | `382952563e…519` | `Windows CI` / job `107164931932` | `success` | `gh run view`：head SHA 一致；`Run tests` 日志实测 `156 passed; 0 failed; 1 ignored`（lib）+ `1 passed`（main），与 ba8bd8d 相同。文档回填提交未改代码，测试计数一致。 |
| [35856136092](https://github.com/yorelll/filego/actions/runs/35856136092) | `382952563e…519` | `Search benchmark` / job `107164932863` | `success` | `gh run view`：head SHA 一致；日志实测 6 行 BENCH（代码与 ba8bd8d 相同，数字有 run-to-run 噪声，见下），守卫未触发。 |

- **权威性说明：** 两个代码一致的 head（`ba8bd8d` 与 `3829525`）各自触发并成功运行了 Windows CI + Search benchmark，共 4 个成功 run；`3829525` 即里程碑 head 且被 CI 完整验证（两 workflow 均 success）——旧 run 无冒充（每个 run 的 head SHA 均与对应 git commit 逐字一致）。
- **MSVC 门禁：** `benchmark.yml`（head 未变，diff 零输出）顶层 `TARGET: x86_64-pc-windows-msvc`（`benchmark.yml:26`）、toolchain `targets: x86_64-pc-windows-msvc`（`42`）、命令 `--target x86_64-pc-windows-msvc`（`59`）三处一致；ci.yml 未变。MSVC 门禁（fmt / clippy `-D warnings` / 158 项测试 / release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact）由 ba8bd8d 与 3829525 两个 Windows CI run 全绿成立。**benchmark.yml 为纯增量，不弱化既有门禁。**

### 权威 MSVC BENCH 数字（reviewer 独立从 run 日志提取）

`gh run view --job 107162638889 --log`（run `35855426323`，head `ba8bd8d`）实测的 6 行 BENCH（median / p95 / max）：

| 查询类 | MSVC median | MSVC p95 | MSVC max |
|---|---|---|---|
| empty-query-default | 0.81 ms | 1.06 ms | 1.08 ms |
| **filtered-with-clone（新增，M02-B 组合路径）** | **59.97 ms** | **62.59 ms** | **64.55 ms** |
| pinyin-heavy | 71.21 ms | 81.83 ms | 89.37 ms |
| english-initials | 59.81 ms | 60.37 ms | 68.42 ms |
| edit-distance | 66.60 ms | 67.22 ms | 70.54 ms |
| multi-token | 54.68 ms | 58.69 ms | 62.92 ms |

- **`filtered-with-clone` 的权威 MSVC median/p95 = 59.97 / 62.59 ms**（r01 response 承诺「供 r02 reviewer 独立提取，与 r01 提取方式一致」，本 reviewer 已按同一方式独立取得并记录于此）。`LENIENT_MEDIAN_BOUND_MS = 500ms` 对该类成立（59.97 ≤ 500），`throw` 守卫未触发。
- **对照 run `35856136092`（head `3829525`，代码相同）的数字**（实测，与 ba8bd8d 的数字并排验证 run-to-run 噪声）：empty-query-default 0.60、filtered-with-clone **42.40 / 50.08**、pinyin-heavy 47.29、english-initials 38.38、edit-distance 44.24、multi-token 39.74。**同一份代码的两个 run 之间同类 median 可相差 1.4–1.5×（pinyin 71.21→47.29，filtered-with-clone 59.97→42.40）**，这直接佐证 r01 F005 的「hosted runner 噪声大、CI 宽松界必要、紧 bound 会假失败」裁定，也佐证「权威数字以具体 run 日志为准、不以 GNU 本地值代 MSVC」的 F001 立场。
- **修复后各类的 bound 情况：** 6 类全部 ≤500ms。**5/6 类在 MSVC hosted-runner 上 median 超 50ms 产品目标**（filtered-with-clone 59.97 / pinyin 71.21 / english-initials 59.81 / edit-distance 66.60 / multi-token 54.68；仅 empty-query-default 0.81 达标）——与 response 在 F005 中修订的「4/5 类 >50ms → 5/6 类 >50ms」陈述一致，r02 已用独立提取的权威数字完成该陈述的最终回填（见 F005 处置）。

## 变更范围核验（无 scope creep / 无回归）

`git diff 2d37541..3829525 --name-status` 精确为 3 个文件：

| 文件 | 类型 | 变更 |
|---|---|---|
| `src/search/benchmark.rs` | M | +106 / −11（F002：第 6 基准类 + 非退化测试 + `run_query`/`time_query_class`/`measure_all` label 透传 + 模块文档 `# Classes`） |
| `review/0-0-1/m02-filter-bench-review-r01.md` | A | r01 评审文档（前一轮审计记录） |
| `review/0-0-1/m02-filter-bench-response-r01.md` | A | r01 response + `3829525` 的 10 行文档回填（5 增 5 删，全部为占位符→真实 run ID/SHA/结论） |

`git diff 2d37541..3829525 -- src/domain/folder.rs src/domain/document.rs src/domain/ids.rs src/storage/mod.rs src/storage/codec.rs src/storage/schema.rs Cargo.toml Cargo.lock src/lib.rs src/app.rs src/search/filter.rs src/search/empty_query.rs src/search/generation.rs src/search/mod.rs src/search/scoring.rs src/search/search_entry.rs src/search/tests.rs .github/workflows/ci.yml .github/workflows/benchmark.yml src/domain/settings.rs src/storage/tests.rs`：**零输出**。domain/storage/工作流/除 benchmark.rs 外的全部 src 均未变。**无 scope creep、无依赖新增、无 ci.yml 变更。**

`git diff ba8bd8d..3829525` 仅修改 `review/0-0-1/m02-filter-bench-response-r01.md`（5 行增 5 行删）：被替换内容为 `<NEW_SHA>`、`<WIN_CI_RUN_ID>`、`<BENCH_RUN_ID>`、`...`、runs/... 等**占位符 → 真实值**（head SHA `ba8bd8d…`、run 35855426114/35855426323、success 结论），并用 `git grep` 实测 head response 文档已**无任何 `<...>`/`<NEW...>`/`TBD`/`待运行` 残留占位符**（见 F001 处置）。

## r01 Finding 逐条复审处置

### F001（Low，证据卫生/记录）— **CLOSED**

- **r01 要求：** response 以 reviewer 提取的 **MSVC 数字为权威基线**记录，链接 run 与 BENCH artifacts；本地 GNU 仅作快速反馈。
- **head 证据（实测）：** response「权威 MSVC 基准数字」节（`m02-filter-bench-response-r01.md` 第 36-49 行）完整录入 r01 5 行权威 MSVC median/p95/max 表，逐行与 r01 review CI 证据节一致；明确声明「**权威基准 = 该表（MSVC ABI + hosted runner）**；本地 GNU 数字仅作开发期快速反馈（CLAUDE.md §3.1）」；链接 run `35849214213`、job `107142618963`、artifact `search-benchmark-log-<sha>`。本地 GNU 数字（multi-token 100.07 / edit-distance p95 124.59 vs MSVC 54.25 / 67.21 方向相反）不再作为基线证据。
- **新增类的 MSVC 数字：** response 未把 `filtered-with-clone` 的 MSVC 数字硬写进表（撰写时该 run 尚未产生），而是显式承诺「该类的权威 MSVC median/p95 记录于 run 日志与 artifact `search-benchmark-log-ba8bd8d…`，**供 r02 reviewer 独立提取**（与 r01 提取方式一致）」；本 r02 review 已按同一方式提取并记录（**median 59.97 / p95 62.59 / max 64.55 ms**，见 CI 证据节）。「待回填」表述在 r02 由 reviewer 独立证据关闭。
- **内部一致性（实测）：** head SHA `ba8bd8da…6157c` 与 `git rev-parse` 一致；run ID 35855426114 / 35855426323 均真实存在且 success；`3829525` 提交把全部 `<...>`/`runs/...` 占位符替换为真实值；response 文档 `grep -E "<[A-Z_]+>|<[a-z-]+/>|<NEW|<RUN|placeholder"` 零命中。head 指向 `3829525` 且 CI 已对该 head 全绿（见 CI 证据）。
- **结论：** CLOSED。

### F002（Medium，基准覆盖缺口）— **CLOSED**（实现 agent 选择「增加基准类」的强关闭路径）

r02 逐项代码核验（不依赖 response 表述）：

1. **第 6 类存在且经 `search_with_filter`：** `QUERIES: [(&str, &str); 6]`（`benchmark.rs:229-236`）含 `("filtered-with-clone", "project alpha")`；`run_query`（`benchmark.rs:212-226`）`if label == FILTERED_WITH_CLONE_LABEL` 分支调用 `crate::search::search_with_filter(entries, &query, &filter, settings, &options)`。`"project alpha"` 解析为 2 个 token（非空），进入 `search_with_filter`（`empty_query.rs:211-251`）的 `filter.apply`（`233`）→ 克隆 `Vec<SearchEntry>`（`241-244`）→ `super::search` 全量打分排序 + 高亮（`245`）的真实组合路径——**不是** `search` 直连、也不是空查询分支。若 label 透传缺失导致落入 `search`，则 M03 presenter 的克隆成本仍未度量；实测 head 分支正确。
2. **filter 保留有意义的子集：** `filtered_clone_filter()`（`200-205`）取 `origin: Some(Origin::Local)`；fixture 中 `index % 10 <= 6` → Local（`157-161`）= 约 70%（约 7000 条）。`filtered_with_clone_keeps_a_meaningful_subset`（`350-400`）断言 `kept_fraction > 0.5` 且 `kept.len() > 5_000`，并断言 `kept.iter().all(|&index| entries[index].origin == Origin::Local)`（filter 非哑）——排除了「全过滤排除的退化快速路径」（`empty_query.rs:234-239` 短路不克隆不排序）与「filter 未生效」两种伪度量。
3. **搜索作用于克隆切片（输出 ⊆ kept subset）：** 同一测试镜像调用 `search_with_filter(..., "project alpha")`，断言 `SearchDisplay::Ranked` 非空、且每个 ranked 条目 ID 都 `∈ kept`（`388-394`）——过滤排除项不可能出现在结果中，证明排名运行在克隆后的切片上而非原始全集。
4. **输出被截断（≠ kept）：** `assert_ne!(ranked.len(), kept.len())`（`395-399`）——ranked 为 `max_results` 截断输出（`DEFAULT_MAX_RESULTS=8`），绝非整个 kept 子集，路径语义正确。
5. **与原 5 类一致的度量与 bound：** 第 6 类与其它类共用同一 `time_query_class`（WARMUP_RUNS=4 + SAMPLE_RUNS=21 + median/p95）、同一 `LENIENT_MEDIAN_BOUND_MS=500ms` 断言（`measure_all` / `ten_k_release_benchmark_stays_under_lenient_bound`）。MSVC 实测 median **59.97ms** ≤ 500ms，bound 覆盖该新类。
6. **原 5 类未变：** `git diff 2d37541..ba8bd8d` 对 `pinyin-heavy`/`english-initials`/`edit-distance`/`multi-token`/`empty-query-default` 五条 label 与 `WARMUP_RUNS`/`SAMPLE_RUNS`/`LENIENT_MEDIAN_BOUND_MS` 常量**零 +/- 改动**（`grep -E "^[-+]...(label|常量)"` 仅命中 import 行）；测试 `query_classes_cover_...` 更新锚定 6 个 label 均 present。5 类 MSVC 数字在 ba8bd8d run 中与 r01 同量级（pinyin 71.21 vs r01 71.11、multi-token 54.68 vs 54.25 等），无回归。
7. **确定性：** 新增路径仍全常量 + 索引算术，无 RNG/Hash/非确定性依赖；label 为 `&'static str` 常量比较，不影响查询结果。

- **结论：** CLOSED（新基准类 + 非退化测试 + CI 实测 6 行 BENCH 全部 ≤500ms 且守卫未触发）。

### F003（Low，观察）— **RECORDED**

response F003 节（`response-r01.md` 第 77-81 行）如实记录：`recent` 为纯谓词 `last_opened_at.is_some()`（`filter.rs:90-92`）、核心不读时钟（模块文档 `filter.rs:21-23`）、时间窗语义移交 M03 presenter、空查询「最近」排序由 `open_count` 降序表达（`empty_rank`，`empty_query.rs:97-104`）。**无代码变更**（`git grep` 核实 `recent`/`empty_rank` 行为未动）。结论：RECORDED。

### F004（Low，防御路径观察）— **RECORDED**

response F004 节（`response-r01.md` 第 83-87 行）如实记录：收藏区 `break` 于 `MAX_FAVORITES`（`empty_query.rs:111-117`）、常规区各节 `!entry.favorite` 谓词（`121-144`）、注释已说明（`135-138`）、合法数据（`document.rs:75` 校验 ≤5）不可达、维持防御性丢弃不降级。**无代码变更**。结论：RECORDED。

### F005（Medium，性能基线 / documented-defer-to-M07）— **CLOSED-as-recorded**

- response F005 节（`response-r01.md` 第 89-93 行）把「4/5 类 >50ms」更新为「**新增第 6 类后『4/5 类 >50ms』变为『5/6 类 >50ms（权威数字待 MSVC CI 回填）』**」——与 `QUERIES` 增至 6 一致，且 r02 已独立回填：**MSVC hosted 上确为 6 类中 5 类 median >50ms**（仅 empty-query-default 0.81ms 达标）。
- response 维持 r01 的「documented-defer-to-M07」裁定：按 `task/01` M02.5 分阶段界定（50ms 由真实机器 M07.3 复核、CI 阈值留噪声空间），**不要求下调 CI bound**；MSVC 权威表作为 M07.3 比对基线；M07 优化输入（pinyin 键派生缓存、multi-token 剪枝、edit-distance 护栏复核、筛选克隆物化成本）已列。
- **r02 边界核验（无 CI-bound 变更）：** `LENIENT_MEDIAN_BOUND_MS=500.0`（`benchmark.rs:59`）与 base 一致；`benchmark.yml` 未变。**无把 CI 门禁盲目调紧/调松的越界行为。**
- 结论：CLOSED（as documented-defer-to-M07，权威数字已由 r02 回填）。

### F006（Low，一致性观察）— **RECORDED**

response F006 节（`response-r01.md` 第 95-99 行）如实记录：`Accessible` 收 `Accessible|Unknown`（`filter.rs:117-123`）与 `Origin` 精确相等（`filter.rs:127-131`）的语义不对称、模块文档 `filter.rs:20` 已明示、供 M03 presenter 理解。**无代码变更**。结论：RECORDED。

### 汇总

| ID | 严重级 | r01 处置要求 | r02 结论 |
|---|---|---|---|
| F001 | Low | 以 MSVC 为权威基准记录 | **CLOSED**（head 无占位符；`filtered-with-clone` MSVC 数字由 r02 独立提取 59.97/62.59） |
| F002 | Medium | 加基准类 或 记录限制 | **CLOSED**（新增第 6 类 + 非退化测试，代码核验全部通过） |
| F003 | Low | 记录 | **RECORDED** |
| F004 | Low | 记录 | **RECORDED** |
| F005 | Medium | documented-defer-to-M07 | **CLOSED-as-recorded**（5/6 类 >50ms 已确认，bound 未变，M07.3 基线就绪） |
| F006 | Low | 记录 | **RECORDED** |

无 `REJECTED` / `PARTIALLY_ACCEPTED`；无未关闭的 Critical/High/Medium。

## 需求/验收标准映射（M02.3 / M02.4 / M02.5，r02 增量复核）

| 验收点 | r01 结论 | r02 增量证据 | 结论 |
|---|---|---|---|
| M02.3 筛选矩阵 | PASS | `filter.rs`/`empty_query.rs` 未变；`Origin`/`Accessibility` 语义未动 | **维持 PASS** |
| M02.4 空查询四策略 / NoResultReason 三态 | PASS | `empty_query.rs` 未变；`search_with_filter` 组合路径语义经新测试镜像调用再次确认 | **维持 PASS** |
| M02.5 确定性 10k 基准 / 成本拆分 | PASS | 原 5 类未变且数字同量级；新增第 6 类补上「筛选+克隆+排名」组合路径的度量缺口（F002 关闭） | **维持 PASS** |
| Settings 前后兼容 | PASS | `settings.rs`/`codec.rs`/`storage/tests.rs` 未变 | **维持 PASS** |

## 强制对抗性清单逐项结论（r02）

### 对抗项 1：F002 新类是否真实度量组合路径 — **PASS**

见 F002 核验第 1-4 点：label 分支命中 `search_with_filter`；非空 token 绕过空查询分支；filter 保 ~70% 子集绕过全排除快速路径；ranked 输出非空且 ⊆ kept；输出被截断（≠ kept）。fixture 的 Local 分布（`index % 10 <= 6`）与 filter 谓词（`Origin::Local`）吻合，`kept.len() > 5000` 断言保障克隆物化数千条目。

### 对抗项 2：确定性 / 无 hash 依赖 — **PASS**

`git grep` 于 `src/search/`：零 `HashMap/HashSet/BTreeMap/BTreeSet`；`std::time::Instant` 仅 3 处且全在 `benchmark.rs`（模块文档 + `use` + `time_query_class:251` harness 内）；`filter.rs`/`empty_query.rs`/`generation.rs` 零 `std::fs/net/env/process/thread`；`empty_rank` 排序键纯函数无时钟/哈希依赖；fixture bit-identical 测试（`benchmark.rs:286-304`）在 head 实测继续通过（CI 156 passed 含之）。零 `#[allow]`、零 `unsafe`（head 复核）。

### 对抗项 3：无 scope creep / 无回归 — **PASS**

`2d37541..3829525` 恰 3 文件（benchmark.rs + 2 份审计文档）；domain/storage/workflow/其余全部 src 零变更（diff 实测空）；无依赖新增（Cargo.toml/lock 未变）；无 CI-bound 变更。**测试计数：base lib 156（155 passed + 1 ignored）+ main 1 → head lib 157（156 passed + 1 ignored，多出的 1 个 passed 即 `filtered_with_clone_keeps_a_meaningful_subset`）+ main 1 = 158 项全部通过/忽略符合预期，0 失败**（ba8bd8d 与 3829525 两个 Windows CI 日志均实测）。M02-A 全部既有测试未删除、未弱化。

### 对抗项 4：workflow 守卫真实执行 — **PASS**

`benchmark.yml:63` `if ($null -eq $summary) { throw ... }` 在两个 run 日志中均未触发且均有 BENCH 行取出（ba8bd8d 6 行、3829525 6 行），证明「判定真实执行」而非跳过。artifact `search-benchmark-log-<sha>` 上传步骤 `if-no-files-found: error`。

### 对抗项 5：证据溯源（无旧 run 冒充） — **PASS**

4 个 run 的 head SHA 与 `git rev-parse 2d37541/ba8bd8d/3829525` 逐字一致；`ba8bd8d`（代码修复）与其随后的 `3829525`（纯文档回填）各有独立的两 workflow 成功 run；里程碑批准所依据的 head `3829525` 即为 CI 验证对象，非旧 run。`git diff ba8bd8d..3829525` 确认无代码改动——r01 的代码结论在文档回填后依然成立。

## Cross-cutting 检查（r02）

- **正确性：** F002 新类经 7 点逐项代码核验（见 F002）；`search_with_filter` 组合语义（filter→clone→rank）与原 5 类度量口径一致。无逻辑反转。
- **数据安全 / 隐私：** `ba8bd8d` 仅改引擎测试 harness；不触碰任何 DOM/storage/数据路径；BENCH 只输出计时与 label，无路径/搜索词/用户数据入日志。
- **安全性：** 无新依赖、无 unsafe、无攻击面增量；基准类纯内存无 I/O。
- **Windows 行为 / MSVC 门禁：** benchmark.yml 三处 MSVC target 一致；ci.yml 未变；两条 code-identical head 的 Windows CI 全绿（含 cargo-deny/cargo-about/portable artifact）。
- **测试覆盖：** 新增 1 个纯逻辑测试（非退化子集）在 debug CI 实测通过；`#[ignore]` 基准在 release CI 实测 1 passed。缺口已由 F002 修复。
- **性能：** 权威 MSVC 6 类全部 ≤500ms；5/6 类 >50ms 已确认并移交 M07.3（F005）。
- **可访问性 / 可维护性：** 模块文档 `# Classes`（`benchmark.rs:21-31`）准确描述第 6 类与 F002 关闭关系；label 常量集中声明；`filtered_clone_filter` 与测试命名自解释。
- **依赖与许可证：** 零新依赖，维持 r01 结论。

## 未能自动验证的项（延续）

1. **真实机器 <50ms（M07.3）**：MSVC hosted 上 5/6 类 >50ms，但真实桌面机器性能须手工验收；基准提供可复现基线与 500ms 宽松回归界。
2. **真实 ABI/硬件差异与 run-to-run 噪声**：同一代码两个 hosted run 之间同类 median 可差 1.4–1.5×（r02 实测），最终判据为真实桌面机器。
3. **`remember_last_filter` / `empty_query_strategy` 的 UI 接线**：M03/M05 presenter 范围，本 slice 仅核心语义 + 持久化 hook。
4. **真实筛选交互**（checkbox 组合、清除、徽标）为 UI 层验收（M03）。
5. **第 6 类的克隆成本在真实 presenter 行为下的可感知性**：M07 应结合活跃筛选下的实际查询频率评估（M07 备注已列入）。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M02-B 筛选 / 空查询 / 10k 基准 r02 复审；**非发布批准**）

- **范围与独立性：** `2d37541..3829525` 恰 3 文件（`src/search/benchmark.rs` + 两份审计文档）；domain/folder、domain/document、storage codec/schema、Cargo.toml/lock、ci.yml 零变更；无新依赖。reviewer 对 `src/search/` 全部 4 个代码提交（含 M02-B 修复 `ba8bd8d`）均未参与。
- **CI：** 4 个 run 全部 head-SHA 匹配、success：ba8bd8d 的 Windows CI `35855426114`（158 项测试 0 失败，含新增非退化测试）+ Search benchmark `35855426323`（1 passed，**6 行 BENCH，守卫未触发**）；3829525 的 Windows CI `35856136080` + Search benchmark `35856136092`（代码相同，独立再验证）。
- **权威基准数字（reviewer 从 run 日志独立提取，MSVC）：** empty 0.81/1.06、**filtered-with-clone 59.97/62.59**、pinyin 71.21/81.83、english-initials 59.81/60.37、edit-distance 66.60/67.22、multi-token 54.68/58.69 ms（median/p95）；全部 ≤500ms 宽松界；5/6 类 >50ms 属 documented-defer-to-M07（F005 已有定论，bound 不调整）。
- **Findings 关闭：** F001 CLOSED（head 无占位符、r02 独立回填新类 MSVC 数字）、F002 CLOSED（第 6 类真实度量 filter→clone→rank、子集非退化、截断、bound 覆盖、原 5 类未变）、F003/F004/F006 RECORDED、F005 CLOSED-as-recorded。**0 Critical / 0 High / 0 Medium 未关闭。**
- **需求映射：** M02.3/M02.4/M02.5/Settings 兼容均维持 r01 PASS；F002 修复补上流程缺口。
- **特别记录（r02 新增观察，非阻断）：** 同一份代码的两次 hosted-run run（ba8bd8d vs 3829525）同类 median 相差最高 1.5×（pinyin 71.21→47.29、filtered-with-clone 59.97→42.40）——**这是 F005「CI 宽松界、硬目标归真实机器」裁定的直接实证**，也是 run-to-run 噪声的首次双样本量化；对 M07.3 的提示是「以多 run 分布而非单点 median 判断达标」。该项不影响 M02-B 批准。

**按 CLAUDE.md §4.5，本结论为 `APPROVED_FOR_MILESTONE`；不构成 `APPROVED_FOR_RELEASE`。** 发布需后续 Release Candidate 独立发布评审、真实桌面手工验收与对应响应闭环（含「未能自动验证的项」）。M02-B slice 在本轮后可标记为 milestone 批准；M07.3 真实机器性能验收与 M03 presenter 接线为后续里程碑内容。
