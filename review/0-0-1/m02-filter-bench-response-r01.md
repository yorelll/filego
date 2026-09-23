# Response: M02-B 筛选 / 空查询策略 / 10k 基准（Round 01）

## Metadata

- **版本:** `0.0.1`
- **里程碑/topic:** `m02-filter-bench`
- **轮次:** `r01`
- **Implementation agent:** M02-B 响应 implementation agent（仅实现 F002 补基准、补测试、触发本地 GNU 验证与远程 CI；未参与 `m02-filter-bench-review-r01` 评审）
- **对应 review 文档:** [`m02-filter-bench-review-r01.md`](m02-filter-bench-review-r01.md)
- **评审前（base）commit SHA:** `2d37541446a0a2095f2b143d02f1c32c02db01e7`（`feat: add search filters, empty-query strategies and 10k benchmark`）
- **响应后（head）commit SHA:** `<NEW_SHA>`（消息 `feat(bench): cover the filter-and-clone search path`，创建后回填）
- **Response 日期:** 2026-09-21（本地时间线）

## Summary

| Finding | 严重级 | Assessment | Status |
|---|---|---|---|
| F001 | Low | `ACCEPTED` | response 以 reviewer 从 run `35849214213` 日志独立提取的 **MSVC BENCH 数字为权威基准**记录（见下文「权威 MSVC 基准数字」表）；本地 GNU 数字仅作本地快速反馈留存（CLAUDE.md §3.1）。附 BENCH artifacts 链接。无代码变更 |
| F002 | Medium | `ACCEPTED` | **新增第 6 个基准类 `filtered-with-clone`**，经 `search_with_filter` 度量「筛选 → 克隆 → 排名」组合路径（含 O(n) 克隆成本），并补一个测试保证 fixture 保留有意义的子集、非退化全排除快速路径。详见下文 |
| F003 | Low | `RECORDED`（已知限制） | `recent` = 曾经打开过（`last_opened_at.is_some()`）纯谓词，无墙钟截止；时间窗语义移交 M03 presenter。无代码变更 |
| F004 | Low | `RECORDED`（防御路径观察） | `FavoritesFirst` 对 >5 条收藏（非法输入）整体排除超额收藏；合法数据（`document.rs:75` 校验 ≤5）不可达。维持现状，已有注释说明。无代码变更 |
| F005 | Medium | `RECORDED`（documented-defer-to-M07） | 权威 MSVC hosted-runner 4/5 类 median 超 50ms；按 `task/01` 分阶段界定移交 M07.3 真实机器复核与优化基线。**不阻断 M02-B，不要求下调 CI bound** |
| F006 | Low | `RECORDED`（一致性观察） | `origin` 的 Unknown 精确态 vs `accessibility` 的 Accessible 收 Unknown 的语义不对称已文档化（`filter.rs:20,127-131`）；供 M03 presenter 理解。无代码变更 |

无 `REJECTED` / `PARTIALLY_ACCEPTED` 条目。1 个代码项（F002）落地为基准类 + 测试，其余以记录/文档形式关闭。

## 变更范围

- `src/search/benchmark.rs` — F002：新增第 6 个查询类 `filtered-with-clone`（经 `search_with_filter` 度量 filter→clone→rank 组合路径），`run_query`/`time_query_class`/`measure_all` 增加 label 透传，模块文档新增 `# Classes` 说明；新增测试 `filtered_with_clone_keeps_a_meaningful_subset`（保证 fixture 子集非退化），更新 `query_classes_cover_…` 锚定 `FILTERED_WITH_CLONE_LABEL`。+106 / -11 行。
- `review/0-0-1/m02-filter-bench-response-r01.md` — 本 response 文档。

未改动 `src/search/` 其它文件、`src/domain/*`、`src/storage/*`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/*`。合法 diff 精确等于上述 1 个代码文件 + 1 份 response 文档（外加随后的 task/ 本地状态更新，task/ 不被 Git 追踪）。

## 权威 MSVC 基准数字（F001 证据卫生 — 本响应以 MSVC 为准）

以下为 reviewer 在 `m02-filter-bench-review-r01.md` 中从 run `35849214213`（workflow `Search benchmark`，head `2d37541446a0a2095f2b143d02f1c32c02db01e7`）job `107142618963` 日志独立提取的 **5 行 BENCH**：`gh run view --job 107142618963 --log`。**权威基准 = 该表（MSVC ABI + hosted runner）**；本地 GNU 数字仅作开发期快速反馈（CLAUDE.md §3.1），不代表发布机性能。

| 查询类 | 权威 MSVC median | 权威 MSVC p95 | 权威 MSVC max |
|---|---|---|---|
| empty-query-default | 0.81 ms | 0.85 ms | 0.87 ms |
| pinyin-heavy | 71.11 ms | 75.83 ms | 82.63 ms |
| english-initials | 58.62 ms | 59.75 ms | 62.44 ms |
| edit-distance | 65.34 ms | 67.21 ms | 68.83 ms |
| multi-token | 54.25 ms | 54.77 ms | 54.99 ms |

- workflow 从 `cargo test --release -- --nocapture` 输出取出 5 行 BENCH（`$summary` 非空 → `throw` 守卫未触发），对全部 5 类成立 `LENIENT_MEDIAN_BOUND_MS = 500ms` 判定。
- 本地 GNU（`1.92.0-x86_64-pc-windows-gnu`）报告值 multi-token 100.07 / edit-distance p95 124.59 与 MSVC 54.25 / 67.21 **方向相反** —— 两种 ABI/机器不能互代，本响应不再以 GNU 数字作为基线证据。
- Artifacts：run `35849214213` 上传 `search-benchmark-log-2d37541446a0a2095f2b143d02f1c32c02db01e7`（含完整 `bench.log`，`benchmark.yml:69` step）。
- 本响应自身提交的 MSVC 数字将在对应 CI run 后回填（含新增的 `filtered-with-clone` 类）。

## Finding Responses

### `F001` — `ACCEPTED`（Low，证据卫生/记录）

- **评估:** 属实。原 implementation 报告仅引本地 GNU release 数字，未从 workflow 日志透出 workflow 自有 MSVC BENCH 行，证据口径不完整。
- **处置（记录，无代码变更）:** 本响应「权威 MSVC 基准数字」节以 reviewer 独立提取的 **MSVC 数字为权威基线**（见上表，含 median/p95/max），链接 run `35849214213`、job `107142618963`、artifact `search-benchmark-log-<sha>`；本地 GNU 数字仅作为本地快速反馈留存（CLAUDE.md §3.1「GNU 与 MSVC ABI、linker、Windows SDK 和依赖行为不同；本地通过绝不替代远程 MSVC CI」）。
- **验证:** 本 response 的 5 数字与该 review 表逐行一致（回读 review 文档 CI 证据节 `34-35` 行）；无代码绕过。

### `F002` — `ACCEPTED`（Medium，基准覆盖缺口 — 补最强关闭：新增基准类）

按 review 处置要求二选一，本实现选择 **「增加基准类」**（reviewer 强化的关闭路径）。

- **新基准类:** `filtered-with-clone`（`benchmark.rs`）
  - **度量内容:** 完整组合路径 **filter → clone → rank**：经 `search_with_filter`（`empty_query.rs:211-251`），`filter.apply` 取本地来源索引（`empty_query.rs:233-239`），把过滤后的 `Vec<SearchEntry>` 整体 `clone()` 物化（`empty_query.rs:241-244`），再对克隆做全量打分排序与高亮（`empty_query.rs:245`）——即 M03 presenter 每次按键在活跃筛选下真实付出的成本，包括 10k 含 String 条目的 O(n) 克隆。
  - **filter 选择:** `origin: Local`（`filtered_clone_filter()`），fixture 中 `index % 10 <= 6` 的条目为 Local（约 70% ≈ 7000 条），克隆真实物化数千条目，**不会落入全过滤排除的退化快速路径**（该路径在 `empty_query.rs:234-239` 直接短路，不克隆不排序）。
  - **测量方式:** 与其它类完全一致——同一 `time_query_class`（WARMUP_RUNS=4 + SAMPLE_RUNS=21 + median/p95）、同一 `LENIENT_MEDIAN_BOUND_MS=500ms` 断言、同一 `BENCH` 行输出；`QUERIES` 增至 6 类，原 5 类保持不变。
  - **查询文本:** `"project alpha"`（fixture 英文名池中大量 Local 条目共享的展示名，保证 rank 输出非空、克隆切片真正被消费）。注意：类标签决定走 `search_with_filter` 分支，与文本 token 是否空无关——它是**非空查询 + 筛选**的组合路径。
- **Subset 非退化测试:** `filtered_with_clone_keeps_a_meaningful_subset`（`benchmark.rs`）
  - 断言 `filter.apply` 保留 >50% 且 >5000 条，且每条都是 `origin == Local`（filter 非哑）；
  - 镜像调用 `search_with_filter` 断言走 `SearchDisplay::Ranked` 非空，且每个 ranked 条目 ID 都属于 kept 子集（证明搜索作用于**克隆后**切片，不是原始全集——过滤排除项不可能出现在结果）。
- **本地 GNU release 记录（快速反馈，非权威; MSVC 数字待本响应 commit 的 CI run 回填）:**
  - `filtered-with-clone`: **median=39.03ms / p95=57.70ms**（max=68.24ms）
  - 5 个既有类仍在宽松界内：empty 0.57/0.73、pinyin 45.20/55.91、english-initials 37.84/119.06、edit-distance 135.07/149.65、multi-token 112.24/122.61（全部 ≤500ms，`ten_k_release_benchmark_stays_under_lenient_bound` PASS）。
  - 说明：GNU median/p95 为开发期快速反馈，方向可能与 MSVC hosted 不同（如上 review 已证 multi-token/edit-distance p95 两 ABI 方向相反）；最终以本响应 commit 的 MSVC CI run 为准。
- **与 F005 的关系:** 新增 `filtered-with-clone` 的本地 GNU median 39.03ms 亦超过 50ms（本地非权威），权威数字待 MSVC CI 回填；同按「documented-defer-to-M07」处理，不阻断 M02-B。

### `F003` — `RECORDED`（Low，观察）

- **评估:** 属实。`recent` 为纯谓词 `last_opened_at.is_some()`（`filter.rs:90-92`），核心不读时钟（模块文档 `21-23` 自述「not a wall-clock cutoff」）。「近 N 天」时间窗必然引入墙钟依赖、破坏核心纯/确定性约束。
- **处置（记录，无代码变更）:** 记为 M03 presenter 的已知供给：时间窗语义（如近 30 天）在 presenter/UI 层以墙钟实现与排序，核心保持纯函数。空查询的「最近使用」排序由 `open_count` 降序表达（`empty_rank`，`empty_query.rs:97-104`），非 `last_opened_at`——与 README 意图一致。
- **验证:** `filter.rs:21-23,90-92`（谓词实现与文档）；`empty_query.rs:97-104`（`empty_rank`）。

### `F004` — `RECORDED`（Low，防御路径观察）

- **评估:** 属实。`favorites_first_indices` 在收藏区 `break` 于 `MAX_FAVORITES` 后，常规区各节谓词均为 `!entry.favorite`（`empty_query.rs:121-144`），非法输入下超额收藏（如测试构造的 id6,7）被整体排除而非降级常规区。
- **处置（记录，无代码变更）:** 合法数据（`document.rs:75` 校验收藏 ≤5）下不可能触发；已有注释说明该边界（`empty_query.rs:135-138`）。维持现状——防御性丢弃而非降级，避免在非法输入下制造重复/顺序歧义。可选改进（常规区末尾列出超额收藏）不作为本 slice 改动，若 M03 需要可另行评估。
- **验证:** `empty_query.rs:111-117`（收藏节上限）、`121-144`（各节 `!entry.favorite` 谓词）、`135-138`（注释）；测试 `favorites_are_capped_at_five_and_remaining_entries_stay_listed` 断言结果恰不含 id6,7。

### `F005` — `RECORDED`（Medium，性能基线 / documented-defer-to-M07）

- **评估:** 属实。权威 MSVC hosted-runner 上 4/5 类 median 超 50ms 产品目标（pinyin 71.11 / edit-distance 65.34 / english-initials 58.62 / multi-token 54.25）。按 `task/01` M02.5 显式界定（「目标单次 <50 ms」由真实机器 M07.3 复核；「CI 阈值留合理噪声空间；硬产品目标由真实机器验收」），**不是 M02-B 阻断**。
- **处置（记录为 documented-defer-to-M07）:** 本 response「权威 MSVC 基准数字」表即 M07.3 比对基线；**不要求下调 CI bound**（hosted 噪声会使紧 bound 假失败）。M07 优化输入：pinyin 键派生缓存、multi-token 逐 token 匹配剪枝、edit-distance 护栏（`MAX_EDIT_DISTANCE_LEN=64`，M02-A 已加）复核、以及新增 `filtered-with-clone` 的克隆成本（M07 若能避免逐键克隆物化则为收益点）。新增第 6 类后「4/5 类 >50ms」变为「5/6 类 >50ms（权威数字待 MSVC CI 回填）」。
- **验证:** run `35849214213` BENCH 行（review CI 证据节）；本 response 权威表。

### `F006` — `RECORDED`（Low，一致性观察）

- **评估:** 属实。`Accessible` 显式包含 `Unknown`（未检查不永久排除，`filter.rs:117-123`），而 `Origin::Local/Network/Removable` 与 `Origin::Unknown` 精确相等互不匹配（`filter.rs:127-131`）。两种语义分别合理：accessibility 的 Unknown 是暂时态需保守保留，origin 的 Unknown 是静止事实、未知即非本地。
- **处置（记录，无代码变更）:** 模块文档 `filter.rs:20` 已明示该对比；确认 M03 presenter 理解「未知来源文件夹在『本地』筛选下会被排除、与该宽容性不同」的语义不对称。
- **验证:** `filter.rs:20,103-131`；测试 `origin_unknown_never_matches_specific_origins_and_vice_versa`（`filter.rs:320-357`）。

## 本地 GNU 验证证据（快速反馈，非 MSVC 权威）

- **Toolchain:** `1.92.0-x86_64-pc-windows-gnu`（rustc 1.92.0, cargo 1.92.0）；预检确认已安装 `x86_64-pc-windows-gnu`、`rustfmt`、`clippy`；`RUSTUP_AUTO_INSTALL=0`、`RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-gnu`，未经 rustup 下载任何组件。
- **shlwapi linker 漂移:** **是**。`cargo test` 链接阶段出现 GNU linker 漂移（`cannot find -lshlwapi`），按已有会话设置解决：`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"`、`PATH` 前置 `D:\mingw64\bin`（并显式 `WINDRES`/`AR` 指向 `D:\mingw64\bin` 供 `winresource` 的 `windres` 使用，与 M02-A 经验一致）。会话级环境变量，非提交内容。
- **结果（本会话均 PASS）:**
  - `cargo fmt --all` 且 `cargo fmt --all -- --check` — PASS；
  - `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` — PASS（0 warnings）；
  - `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` — PASS，**lib 156 + main 1 = 157 项通过 / 1 ignored（10k 基准），0 失败**；新增 `filtered_with_clone_keeps_a_meaningful_subset` 在 debug 下运行并通过；
  - `cargo test --release --workspace --all-features --locked --target x86_64-pc-windows-gnu`（含基准）— PASS，**6 行 BENCH 全部 ≤500ms 宽松界**（见 F002 记录）；
  - `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` — PASS。
- **权威性声明:** 本机 GNU 通过不替代远程 MSVC CI；本地产物仅做开发检查（CLAUDE.md §3.1）。权威基准 = MSVC run（待本响应 commit 的 CI 回填）。
- **新基准类 GNU median/p95（快速反馈）:** `filtered-with-clone` **39.03 / 57.70 ms**。

## CI 证据（远程 MSVC）

| commit SHA | Workflow | run ID/URL | 结果 | 说明 |
|---|---|---|---|---|
| 响应代码（head SHA，见 Metadata） | [Windows CI](https://github.com/yorelll/filego/actions/runs/...) | `<WIN_CI_RUN_ID>`（创建后回填） | 待运行 | `fmt / clippy -D warnings / tests(157) / release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact`（ci.yml 未变） |
| 同上 | [Search benchmark](https://github.com/yorelll/filego/actions/runs/...) | `<BENCH_RUN_ID>`（创建后回填） | 待运行 | MSVC release `--release --ignored --nocapture`，应输出 **6 行 BENCH**（含新增 `filtered-with-clone`），`$summary` 守卫应不触发；artifact `search-benchmark-log-<sha>` 应含该类 median/p95 |

推送后监控到结束并回填两 run 的 ID/结论与新类的 **MSVC median/p95**（权威值）。CI 必须由已推送到 GitHub 的提交触发；本响应在推送完成前状态为「待验证」。

## 未解决事项与待人工验证项

1. **本 response commit 的 MSVC CI run（Windows CI + Search benchmark）** 为权威验证证据；新类 `filtered-with-clone` 的权威 MSVC median/p95 与 `throw` 守卫行为待回填。若 run 失败将修复根因并重跑，不以旧 run 冒充证据（CLAUDE.md §3.4）。
2. **F005 的 50ms 产品目标** 由 M07.3 真实桌面机器验收；本表（含新增第 6 类）为比对基线。
3. **`remember_last_filter` / `empty_query_strategy` 的 UI 接线** 属 M03/M05 presenter 范围（本 slice 仅持久化 hook + 核心语义）。
4. **M03 presenter 需注意：** F003（`recent` 时间窗在 presenter 层实现）、F006（未知来源在「本地」筛选下被排除）、以及 `search_with_filter` 的克隆成本在活跃筛选下的延迟表现（M07 优化输入）。
5. **GNU linker 漂移（shlwapi）** 为本机会话现象，非项目缺陷；MSVC CI 无此问题。

## M07 备注（F002/F005 移交输入）

- **M07.3 基线与优化:** 本 response 权威 MSVC 表（5 类 + 新增 `filtered-with-clone` 待回填）作为真实机器验收与优化起点。候选优化：pinyin 键派生缓存、multi-token 逐 token 匹配剪枝、edit-distance 护栏复核、以及 `search_with_filter` 的过滤克隆物化成本（M07 若能在筛选后复用原 `Vec`/索引而非逐条 `clone`，则 presenter 路径收益明显——但需先以权威数字确认克隆占比）。
- **新增 `filtered-with-clone`（本地 GNU 39.03ms）** 本身即 M07 的优化输入：M03 presenter 开启筛选后每按键将付出 filter + clone + rank 三段成本，M07 真实机器验收须覆盖该交互路径。

## 请求复审

本 response 已按 CLAUDE.md §4.4 逐条回应 F001–F006，完成 F002 的代码关闭（新增基准类 + 非退化测试）与 F001/F005 的权威记录。请原 reviewer（或另一名同样独立、未参与实现的 code-review agent）对以下内容进行 **r02 复审**：

1. `src/search/benchmark.rs` 的新增 `filtered-with-clone` 类（`run_query` 分支、`QUERIES` 第 6 项、label 透传）是否真实走 `search_with_filter` 组合路径且与原 5 类一致度量；
2. `filtered_with_clone_keeps_a_meaningful_subset` 测试是否证明子集非退化、且 search 作用于克隆切片；
3. 本 response 的权威 MSVC 数字表是否与 `m02-filter-bench-review-r01.md` 一致、处置是否满足各 finding 要求（F001 证据修正、F002 补类、F003/F004/F006 记录、F005 documented-defer-to-M07）；
4. 本 commit 的 Windows CI + Search benchmark run 结果（回填后）。

本里程碑批准与后续发布均不构成 `APPROVED_FOR_RELEASE`；`APPROVED_FOR_RELEASE` 需后续 Release Candidate 独立发布评审、真实桌面手工验收与对应响应闭环。
