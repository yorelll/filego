# Review: M03 Slint 主搜索窗口 + ViewModel（Round 02 — 复审）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m03-window`
- **轮次：** `r02`（对 `review/0-0-1/m03-window-response-r01.md` 及 F001/F002 修复代码的复审）
- **日期：** 2026-09-21（本地时间线；CI 时间 2026-09-23T…Z，见 CI 证据表）
- **Reviewer：** 独立 code-review agent（M03 r02）
- **独立性声明：** 本 reviewer **未参与** M03 实现 commit `7de3dd3` 的任何编码，**未参与** F001/F002 修复 commit `097dc40` 的任何编码，未撰写 M03 的 r01 review 或 r01 response 文档，未参与 CI 触发与监控；本评审严格只读：**未编辑任何源代码、未运行任何 `cargo` 命令（依照 CLAUDE.md §3.1 与任务限制未执行本地 Rust 验证）、未提交、未推送**；除本 review 审计文档外未写入任何其他文件（本文件创建后会由主 agent/用户决定 Git 跟踪提交，reviewer 不自行 commit）。
- **Base SHA（r01 review 对象）：** `7de3dd39f63d9d43d6fe1d20622a9250b365fc79`（`feat: add Main search window with ViewModel, IME gate, i18n, theme`）
- **Head SHA（本评审对象）：** `1228d6d4e319672768a575393a69aa2b603ea21b`（`docs: respond to M03 window review r01`）——`git rev-parse HEAD` 实测与之一致。该 head 的线性历史（`7de3dd3..1228d6d`）包含：
  - `6aac7df` `docs: record M03 window review r01`
  - `097dc40` `fix: render real result count and full-path tooltip`（F001/F002 代码修复）
  - `1228d6d` `docs: respond to M03 window review r01`（本复审所审 response 文档）
- **比较范围：** `7de3dd3..1228d6d`（代码 + 文档；`git diff --stat` 4 文件，见下）
- **审查文件（代码）：** `src/main.rs`（+66/−…，F001 计数派生 + F002 full-path 推送 + 1 新单测）、`ui/app-window.slint`（+8/−…，ResultRow `row-full-path` + Tooltip 绑定 + root `rows-full-path`）
- **审查文件（审计/上下文）：** `review/0-0-1/m03-window-review-r01.md`（r01，verdict `APPROVED_FOR_MILESTONE`）、`review/0-0-1/m03-window-response-r01.md`（本复审对象）、`src/presentation/{i18n,state,view_model}.rs`（head 逐字核验消费契约）
- **审查方法：** ① `git diff 7de3dd3..1228d6d` 逐字审查 4 文件；② 直接用 `git show <sha>:<file>` 读取 head 代码（不依赖 response 转述）核验 F001/F002 的实际实现；③ 对抗性 grep（shell 执行/`#[allow]`/真实目录删除/`unwrap/expect/panic` 新增/`std::fs`）；④ 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询并核验 2 个 CI run 的 head SHA、workflow、job/steps 结论、关键日志行（test result 计数、BENCH 行、MSVC release EXE 验证步骤守卫）；⑤ 统计口径核对（静态 `#[test]` 计数 vs CI 日志）；⑥ response 文档内部一致性与台头 commit 有效性核验。

## CI 证据（reviewer 独立核验）

任务给定的 CI 范围在 head `1228d6d`（含修复 commit `097dc40`，见上线性历史）；两 run 均由本 reviewer 用指定 gh.exe 独立查询：

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35869424552](https://github.com/yorelll/filego/actions/runs/35869424552) | `1228d6d4e319672768a575393a69aa2b603ea21b` | `Windows CI` / job `107209287288`（`fmt, clippy, test, release, package`） | `success`（completed，push event，`feature/m00-foundation`） | `gh run view`：head SHA 与 `git rev-parse HEAD` 一致；displayTitle `docs: respond to M03 window review r01`（即 head 就是 response 提交，含其父 `097dc40` 修复）；job 同名单 success；20 个 step 全绿（fmt→clippy `-D warnings`→tests→release build→`Verify release outputs and version helper`→deny→about→portable artifact→upload）。`--log` 实测：**lib `running 189 tests` → `test result: ok. 188 passed; 0 failed; 1 ignored`**（ignored = M02-B 基准）+ **main bin `running 3 tests` → `test result: ok. 3 passed`**（含新增 F001 测试 `results_count_label_is_derived_from_the_row_count`）+ 其余 bin `0 passed`；`Verify release outputs and version helper` 步骤含 `if (-not (Test-Path ...filego.exe...)) { throw ... }` 与 version 一致性守卫，步骤结论 success（守卫未触发）。 |
| [35869424387](https://github.com/yorelll/filego/actions/runs/35869424387) | `1228d6d4e319672768a575393a69aa2b603ea21b` | `Search benchmark` / job `107209285588`（`release 10k benchmark`） | `success`（completed，push event，同 head/同时间戳） | `gh run view`：head SHA 一致；job 内 `Run release-mode 10k search benchmark` 结论 success；`--log` 实测 `running 1 test` → `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 188 filtered out`；**实测 6 行 BENCH**（BENCH 行 + 着色副本共 12 条）：`empty-query-default median=0.79ms` / `filtered-with-clone median=58.36ms` / `pinyin-heavy 70.03ms` / `english-initials 57.38ms` / `edit-distance 65.08ms` / `multi-token 53.45ms`；`if ($null -eq $summary) { throw }` 守卫未触发（BENCH 行存在且 job success）。 |
| 补 | `git diff 7de3dd3..1228d6d --name-only` 不含任何 workflow | — | — | 两 workflow 在本 range 内零变更（`git diff --name-only 7de3dd3..1228d6d -- .github/workflows` 零输出，见下） |

- **统计口径核验：** head 静态 `git grep '#[test]'` 计数 **192** = app 8 + domain/mod 1 + main 3 + version 1 + presentation（i18n 5 + state 5 + theme 4 + view_model 18）+ search（benchmark 5 + empty_query 6 + filter 11 + generation 1 + keys 7 + matching 6 + query 5 + scoring 7 + tests 38）+ storage（repository_tests 33 + tests 28）。192 − main 3（独立 bin）= 189 lib `#[test]`（其中 1 个 `#[ignore]` 基准）→ 与 CI `running 189 tests / 188 passed + 1 ignored` 一致；main bin 3 = base 2 + 新 F001 测试 1 → CI `running 3 / 3 passed` 一致。新测试在 MSVC CI 中真实执行并通过。
- **MSVC 门禁核验：** Windows CI head `1228d6d` 全绿覆盖 fmt / clippy `-D warnings` / 192 项测试（189 执行 + 1 ignored，main 3）0 失败 / MSVC release build / `Verify release outputs and version helper`（MSVC release EXE 与 version helper 存在性检查）/ cargo-deny / cargo-about / portable artifact 上传。MSVC 门禁成立（§3.2 的 1–5 项均有实体步骤且 success）。
- **response 文档 CI 台头可满足性：** response 中 CI 表为「见下方 `gh run list` 输出」占位（未写死 run ID），并声明「main agent 在推送本 commit 后定位 run ID 并监控到结束」。实际主 agent 在 head `1228d6d` 推送触发的前述两 run（35869424552 / 35869424387）即可满足该占位，且两 run head SHA 与 response 修复 commit `097dc40` 的 head 一致（`1228d6d` 是其子提交，代码与 fix 相同）。台头有效，无缺口。

## 变更范围核验（无 scope creep）

`git diff 7de3dd3..1228d6d --name-status` 恰 4 文件：

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/main.rs` | M | F001：删除 `apply_localization_and_theme` 中常量 `ResultsCount(0)` 写入（改为注释性 NOTE 块）；新增纯函数 `results_count_label`（`Msg::ResultsCount(u16::try_from(rows.len()).unwrap_or(u16::MAX)).tr(locale)`）；`sync_ui` 每次推送 `set_count(...)`；F002：`sync_ui` 新增 `set_rows_full_path(...)`；新增单测 `results_count_label_is_derived_from_the_row_count` |
| `ui/app-window.slint` | M | F001：显示结构不变（`text: UiStrings.count` 在 `root.rows-count > 0` 下）；F002：`ResultRow` 新增 `in property <string> row-full-path`；root 新增 `in-out property <[string]> rows-full-path: []`；for 循环绑定 `row-full-path: root.rows-full-path[i]`；行 2 `Tooltip` Text 改为 `row-full-path.is-empty ? row-path : row-full-path`；可见行 2 文本仍绑定 `row-path`（label） |
| `review/0-0-1/m03-window-response-r01.md` | A | r01 response（本复审对象） |
| `review/0-0-1/m03-window-review-r01.md` | A | r01 review（已在上一轮存在，位于比较范围内因 base 先于其创建） |

**越界零检出：** `git diff --name-only 7de3dd3..1228d6d -- src/search src/domain src/storage src/lib.rs Cargo.toml Cargo.lock .github build.rs` → **零输出**。`src/presentation/{i18n,state,view_model}.rs` 在 head 与本 range 未变（`git diff --stat` 仅列 2 个代码文件 + 2 个 review doc）。**无 scope creep、无依赖新增、无 workflow 变更、无 CMake/build 变更、无 M00/M02 回归面。**

## Finding 逐条复审

### F001（Medium）— 结果计数显示恒为 0 → **CLOSED**（本次修复关闭）

**复审证据（head 代码，非 response 转述）：**

1. `apply_localization_and_theme`（`src/main.rs` head）：原 `strings.set_count(Msg::ResultsCount(0)...)` **已删除**，替换为注释 NOTE(F001) 说明「不在初始化设置 count，改为每次 sync_ui 从 rows 派生」。`git grep -n 'ResultsCount(0)' 1228d6d -- src/main.rs` → 仅命中注释文本行（`// Setting it once with a constant (the old ResultsCount(0)) made the label stay "0 个结果" forever.`），**无任何实际常量写入**。
2. 纯函数 `results_count_label(rows, locale)`（head `src/main.rs`）：返回 `Msg::ResultsCount(u16::try_from(rows.len()).unwrap_or(u16::MAX)).tr(locale).into()`——数量即 `rows.len()`，`u16::try_from` 溢出饱和到 `u16::MAX`，无 panic。
3. `sync_ui`（head `src/main.rs`）在每次推送统一更新：`window.set_rows_count(row_count as i32)` 后紧跟 `window.global::<UiStrings>().set_count(results_count_label(&rows, state.locale))`——与 `rows-count` 同源（`state.rows`）同步；`rows` 是 `let rows = state.rows.clone()` 的同一快照。
4. 空查询默认策略行：`SearchViewModel::new` 末尾调用 `self.refresh()`（空查询触发 M02.4 `empty_query_default_strategy`，测试 `initial_state_shows_empty_query_strategy_rows` 断言初始 rows 非空且 query 为空），故默认窗口行进 rows → 计数显示真实默认行数。
5. 新单测 `results_count_label_is_derived_from_the_row_count`（base 无、head main.rs tests 3 项含此）断言：空 rows → 含 `0`；2 行 rows → zh `2 个结果`、en `2 results`（目录：zh `"{count} 个结果"`、en `"{count} results"`，`i18n.rs` 逐字核验一致）。
6. 初始帧边界：base 与 head 的 `sync_ui` 均仅在 `handle()` 内被调用（`main.rs:94-95`），启动首帧不显式推送；此时 `.slint:426` 的计数 Text 仅在 `root.rows-count > 0` 时渲染，而 rows-count 默认 `0` → 首帧不显示计数文本，首个用户命令到达后 `handle()→sync_ui()` 即推送真实 count。**该初始推送缺口在 base 与 head 完全一致（非本次修复引入）且不会显示陈旧「0 个结果」**（无 rows 则不显示计数）。故修复对全部可达状态闭合。

**结论：** F001 对应 r01 建议（「将 count 加入 ViewState（或复用 rows.len()）并在 sync_ui 每次 set_count」）自洽实现，常量路径彻底移除，新测试在 MSVC CI 通过。**CLOSED。**

### F002（Low）— 行 2 路径 tooltip 显示 path_label 而非 full_path → **CLOSED**（本次修复关闭）

**复审证据（head 代码）：**

1. `ResultRow` 新增 `in property <string> row-full-path;`（head `ui/app-window.slint`，与 `row-path` 分离）。
2. root `AppWindow` 新增 `in-out property <[string]> rows-full-path: [];` 并行数组。
3. for 循环（结果行）新增 `row-full-path: root.rows-full-path[i];` 绑定（head slint）。
4. 行 2 `Tooltip` 的 `Text` 改为 `text: row-full-path.is-empty ? row-path : row-full-path;`——tooltip 优先显示完整路径；回退分支（full 为空）为防御性，adapter 总推送 `full_path`。
5. **可见行 2 文本不变**：仍为 `text: row-path + ...`（path_label + category + tags），长路径只在 tooltip 完整展示。显示/工具提示解耦达成，符合 `state.rs` 注释声明的 full_path 用途（tooltip / Copy-Path）。
6. `sync_ui` 推送 `window.set_rows_full_path(string_model(rows.iter().map(|row| row.full_path.clone())))` 与 `rows-path` 同源（`state.rows`），并附注释「full path is carried on each row for the tooltip / Copy-Path only; visible row line-2 keeps the short label」。
7. **隐私：** full_path 仅在 UI 属性/tooltip 呈现；修复未新增任何日志/打印（`diff` 中无 `eprintln`/`log`/`dbg!`）；`state.rs`「never serialized, never logged」注释未变。

**结论：** F002 建议（「把 full_path 作为独立 row-full-path 属性推入并绑定 tooltip」）完整实现，显示文本与 tooltip 解耦，隐私语义保持。**CLOSED。**

### F003（Low）— 交互路径无自动化覆盖 → **RECORDED**（维持现状）

**复审确认：** 本 range 对 `.slint` 的 key-pressed/touch 映射无任何自动化测试新增（r01 建议的「轻量 ISA 向量化覆盖」response 明确记录为可选、不在本轮范围）。UI 层 key 分发（`ime-composing` reject 分支、Ctrl+C input/results 分歧）仍依赖结构审查 + CI 编译 + 桌面手工验收。response 已将之列入桌面验收清单（主窗键盘交互、IME 组合期间 Enter/方向键、单双击、focus ring、Tab 顺序）。**维持 r01 裁定：接受现状 + 记录为已知限制，不阻断里程碑；M04 评审时需确认桌面验收项被实际执行。RECORDED（无代码变更，符合记录）。**

### F004（Low）— `SearchFailure` 死变体 / `Search Err` 分支未覆盖 → **RECORDED**（M06 前置）

**复审确认：** 本 range 未触碰 `view_model.rs`/`state.rs`（`git diff --name-only` 不含 presentation 文件）；`SearchFailure::Load/Save` 在 M06 存储接线前不可达、`Search` 分支无注入 runner 的 Err 测试的现状**未变也不应在本轮改**。response 明确记录为 M06 前置接线项并要求「M06 评审必含 error 端到端」。**RECORDED（维持，M06 强制项）。**

## 需求/验收标准映射（M03.1–M03.5，r02 增量视角）

r01 已对 M03.1–M03.5 全部验收点给出 **PASS**（含显式后置项）。r02 仅复核与本题修复/回归相关的增量：

| 验收点 | r02 证据 | 结论 |
|---|---|---|
| M03.1「筛选按钮/数量」中**数量**有真实语义 | `sync_ui` 每次从 `rows.len()` 派生并写入 `UiStrings.count`；`.slint:426-427` 显示；M02.4 默认空查询策略行计入 → 数量不再恒 0 | **PASS**（修复后） |
| M03.1「长路径 ellipsis + tooltip」语义完整 | 可见行 2 仍 elide（`overflow: elide` 未动）；tooltip 现显示完整 `full_path` | **PASS**（修复后） |
| M03.2 单一状态来源 | count 与 rows 同一 `state.rows` 快照派生，无第二来源 | **PASS** |
| M03.4 i18n parity | 本 range 未改 `i18n.rs` 目录；`ResultsCount` 两语言 key/格式逐字核对一致；parity 测试 head 全绿 | **PASS**（无回归） |
| M03.5 UI 测试 | 新 F001 单测 + 既有 191 项（CI 192 静态口径）全部 head 通过 | **PASS** |

无新增 GAP。

## Cross-cutting 检查（head `1228d6d`，增量）

- **正确性：** F001 计数与 rows 同快照派生（无时序分歧）；`u16` 溢出饱和不 panic；空查询默认行计入。F002 tooltip 回退分支 (`is-empty ? row-path : row-full-path`) 在与 adapter 总推送 `full_path` 的组合下等价于恒 full_path，防御分支不产生错误显示。
- **错误处理：** 修复新增零 `unwrap()`/`expect`/`panic!`（`diff` grep 仅 `unwrap_or`/`unwrap_or_default`）；`try_from(...).unwrap_or(u16::MAX)` 无 panic 路径。既有 `expect("fixed date")`/`expect("theme token ...")`（编译期常量解析）未动。
- **数据安全：** 修复范围零 `std::fs`、零真实目录删除引用；`rows-full-path` 仅 UI 属性。grep `remove_dir/remove_file` 于 `7de3dd3..1228d6d` 代码 diff 零命中。
- **隐私：** 无新增日志/路径写入；full_path 仅 tooltip/Copy-Path 语义。
- **安全性：** 修复范围零 shell 执行（grep `Command::new/std::process/powershell/explorer/ShellExecute` 于 diff 零命中）；无新依赖、无新攻击面。
- **Windows 行为 / MSVC：** 两 workflow 零变更；Windows CI head 全绿（含 fmt/clippy/tests 192/release/verify/deny/about/artifact）；benchmark CAS（`filtered-with-clone median=58.36ms`）与 r01（61.58ms）同数量级、宽松上界内，无回归信号。
- **测试覆盖：** 192 静态 = 189 lib（188 pass + 1 ignored 基准）+ 3 main，CI 日志逐项吻合；M03 presentation 33 项（view_model 18 + i18n 5 + state 5 + theme 4 + main 新增 1）在 head 全绿；既有 M00/M02/M01 测试未被删改。
- **确定性：** 计数派生与 full-path 推送均为确定性顺序映射，无集合迭代序依赖。
- **可维护性：** F001/F002 注释（NOTE(F001)、F002 注释块）清晰说明为什么不再常量设置 count、full_path 仅 tooltip 用；纯函数独立可测。

## 对抗性检查结论（r02 增量）

1. **Shell/路径执行 → 零命中**（diff 内无 `Command::new`、`std::process`、`explorer`、`ShellExecute`）。
2. **Search/domain/storage/lib/Cargo/workflows → 零变更**（`git diff --name-only` 验证）。
3. **日志/隐私 → 通过**（无新 `eprintln`/log；full_path 不进日志）。
4. **`#[allow]`/unsafe/真实目录删除 → 零命中**（head `main.rs` + `.slint` grep）。
5. **初始化惰性推送缺口 → 非回归**（base/head 一致；无 rows 则不显示计数文本，不出现「0 个结果」）。

## 未能自动验证的项（与 r01 一致，追加 r02 相关）

1. r01 项 1–6 全部维持（native IME preedit、system widget palette、OpenEntry/CopyPath stub、高对比/文本缩放、Ctrl+A/Ctrl+Backspace、早期视觉/IME smoke）。
2. **r02 新增：** 计数 Text 的「首个用户命令前不渲染」边界为纯渲染时序，无法自动验证，需在 M04 桌面 smoke 中目视确认（窗口打开后结果行出现时计数即显示真实值）。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M03 主搜索窗口 + ViewModel；非发布批准）

- **范围与独立性：** `7de3dd3..1228d6d` 恰 4 文件（2 代码 + 2 review doc）；search/domain/storage/lib/Cargo/workflows 零变更；修复 commit `097dc40` 仅含 F001/F002 代码，独立于本 reviewer（未参与任何 M03 编码）。F001（Medium）与 F002 修复均经**直接读取 head 代码**复核，非依赖 response 转述。
- **CI：** head `1228d6d` 的 Windows CI `35869424552`（job `107209287288`，**192 项测试 0 失败**，含新 F001 单测 `3 passed`，MSVC release EXE 验证守卫通过，全 20 step 绿）+ Search benchmark `35869424387`（job `107209285588`，`1 passed` + **6 行 BENCH** 含 `filtered-with-clone median=58.36ms`，守卫未触发）两 run 独立核验，head SHA 均与 `git rev-parse HEAD` 一致。
- **Finding 处置：** F001 **CLOSED**、F002 **CLOSED**（有代码证据 + 新单测 + CI）；F003/F004 **RECORDED**（无代码变更，M04 桌面验收 / M06 前置接线项维持）。r01 的 5 项后置（focus+全选、Tab filter、右键 menu、reduce-motion、高对比）按任务清单继续沿用，无新增阻断项。
- **非发布批准：** 依 CLAUDE.md §4.5，本结论仅 `APPROVED_FOR_MILESTONE`，不构成 `APPROVED_FOR_RELEASE`；发布需后续独立 release review、真实桌面手工验收（含上节未自动验证项）、以及 M03 之前里程碑要求全部满足后的最终 `APPROVED_FOR_RELEASE`。
- **后续义务：** F003 桌面验收项需在 M04 评审时确认已执行；F004 的 error 端到端为 M06 评审强项；r01 结论中「F001 作为 Medium 在本次已关闭」替代原 m05/发布前关闭义务。
