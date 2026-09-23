# Review: M02-A 纯搜索核心（Round 02 / 复审）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m02-search`
- **轮次：** `r02`（对 `m02-search-response-r01` 与修复 commit `eaba66d` 的复审）
- **日期：** 2026-09-21（本地时间线；CI 时间为 2026-09-23T05:01:43Z）
- **Reviewer：** 独立 code-review agent（M02-A r02 复审）
- **独立性声明：** 本 reviewer 未参与 M02-A 实现 commit `18c48f5`、未参与 r01 修复 commit `eaba66d`、未参与任何 response 撰写；本仓库 `src/search/` 的全部历史仅为 `18c48f5` 与 `eaba66d` 两个提交（`git log --all -- src/search/` 仅两条），reviewer 未以任何形式（含会话压缩/重命名/换名字）实现过其中代码；本评审未编辑源代码、未运行任何 `cargo` 命令（严格依照 CLAUDE.md §3.1 未执行本地 Rust 验证）、未提交或推送；除本 review 审计文档外未写入其他文件。
- **Base SHA（r01 head）：** `18c48f5040193be3990251ad8a9c7a11f57efa7a`
- **Fix SHA：** `eaba66dcd0871da412796da97aaa9228ea5044f3`（code；消息 `fix: correct tie-break order and add tie-break coverage`）
- **Head SHA（本评审对象）：** `093941aec49e6783317e8e26f056d529063acb4c`（消息 `docs: describe M02 CI evidence without a pinning run id`，为 `eaba66d` 的后代，头部文档链仅改 `review/` 下文档）
- **比较范围（代码）：** `18c48f5..eaba66d`（仅代码 diff）
- **完整范围（代码+文档）：** `18c48f5..093941a`
- **审查文件（代码）：** `src/search/{scoring,tests,matching,mod,keys}.rs`（`git diff --name-status 18c48f5 eaba66d`）
- **审查文件（审计/上下文）：** `review/0-0-1/m02-search-review-r01.md`、`review/0-0-1/m02-search-response-r01.md`、`src/search/scoring.rs` 全量（head）、`src/search/matching.rs` 全量（head）、`src/search/mod.rs` 全量（head）、`src/search/keys.rs` 相关段（head）。
- **审查方法：** ① 完整阅读 r01 review 与 response，对每个 finding 的处置先按 response 声明，再**回到代码逐一独立核验**（未经 response 摘要代替）；② `git diff 18c48f5..eaba66d` 逐字审查 5 个代码文件与统计；③ 复核 head 上 `tiebreak` 实现、文档注释与三个表驱动单元测试的方向正确性（以 `std::cmp::Ordering` 契约人工推演：comparator 返回 `Less` = left 排前）；④ 复核端到端同分用例是否真正同分（total_score 逐对相等断言）且由 tiebreak 决定序，而非 tier；⑤ 复核编辑距离长度护栏的守卫逻辑与回归测试断言；⑥ 核验 `mod.rs`/`keys.rs` 文档文本；⑦ 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询 CI run `35820715554` 的 head SHA、workflow、job、`test result` 日志行，并记录链上被取消的 doc 中间 run；⑧ `git grep` 质量检查（`#[allow]`、HashMap/HashSet/BTreeMap、`#[test]` 计数）。

## CI 证据

### 权威 run（head `093941a`）

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35820715554](https://github.com/yorelll/filego/actions/runs/35820715554) | `093941aec49e6783317e8e26f056d529063acb4c` | `Windows CI` / `fmt, clippy, test, release, package` | `success`（completed，push event，5m11s） | `gh run view`：`conclusion=success`，head SHA 与 `git rev-parse HEAD` 一致；单 job 同名并 `success`；`--log` 实测 `Run tests` step：`test result: ok. 132 passed; 0 failed`（lib）+ `test result: ok. 1 passed`（main bin）+ 2 个 doc-test 二进制 `0 passed`。**133 项测试全部通过，0 失败。** |

- **取消链透明记录：** 链上 6 个 run 因文档-only 推送被 supersede 而 `cancelled`（`gh run list`，按时间序）：`35820240269`（`fix: correct tie-break order and add tie-break coverage`，head `eaba66d`，**修复代码的首次推送即被后续文档推送取消**）、`35820401923`（finalize M02 response）、`35820488883`（point at authoritative run）、`35820539585`（cite final head run）、`35820614489`（finalize for 42b7596）、`35820667447`（align with 2142c99）。这些 run 被取消均因后续文档提交取代，而**全部文档提交（`c0de7e5`..`093941a`）只改 `review/` 下文档，不改变任何构建输入**（`git diff --name-only 18c48f5 093941a` 确认仅 `review/` 2 文档 + `src/search/` 5 文件，其中 `eaba66d` 已全量含 5 个代码文件）。因此 build input 与 `eaba66d` 完全相同，权威依据取最终推送 head 的成功 run `35820715554`，该 run 历史中包含所述构建输入。此透明处理与 r01 response 声明一致。
- **统计口径核验（head）：** `git grep -c '#[test]' HEAD` 静态计数：lib 132 = app 8 + domain 1 + storage/repository_tests 33 + storage/tests 26 + version 1 + search（keys 7 + matching 6 + query 5 + scoring 7 + tests 38）= 132；另 main bin 1（`filego-version`）。**合 133，与 CI 日志 132+1 及 response 声称完全一致（无 ±1 口径差）。** r01 计 126 = 132 − 新增 6（scoring 3 + matching 1 + tests 2），自洽。
- **MSVC 门禁核验：** workflow 为 r01 已验证过的同一 `Windows CI`（顶层 `TARGET: x86_64-pc-windows-msvc`）；本 run 全绿涵盖 fmt / clippy `-D warnings` / 133 项测试 / release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact。修复不改 Cargo/CI 配置，门禁继续成立。

## 变更范围核验（无 scope creep）

`git diff 18c48f5..eaba66d --name-status` 精确为：

| 文件 | 类型 | 变更 |
|---|---|---|
| `src/search/keys.rs` | M | +8 行（pinyin 模块文档，F005） |
| `src/search/matching.rs` | M | +68/-x（`MAX_EDIT_DISTANCE_LEN` 护栏 + 测试，F003） |
| `src/search/mod.rs` | M | +12/-x（`search()` 文档补最弱 token 定级，F004） |
| `src/search/scoring.rs` | M | +96/-x（`tiebreak` 两个分支反向纠正 + 3 个表驱动单测，F001） |
| `src/search/tests.rs` | M | +91/-x（死代码清理 + 2 项端到端用例，F002） |
| `review/0-0-1/m02-search-review-r01.md` | A | r01 review 文档（先于 fix 存在，属审计记录） |

`git diff --name-only 18c48f5 eaba66d -- src/domain src/storage src/lib.rs Cargo.toml Cargo.lock .github`：**零输出**。`src/domain/*`、`src/storage/*`、`src/lib.rs`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/*`、`build.rs` 均未变。**无 scope creep。** 修复期未新增/移除依赖，无许可证/deny 变化。

## 需求/验收标准映射（head，与 r01 一致的验收点复核）

### M02.1 查询处理 — 全部保持 PASS

修复仅涉及 tie-break 方向、编辑距离护栏、文档；不影响 trim/空白合并、大小写/fold 归一化、多 token AND、8 策略、别名、开关门控、派生键不落盘、编辑距离不压过精确等任何查询处理语义（相关代码未变，r01 逐项 PASS 维持，本轮不重复逐行引证）。

### M02.2 评分与高亮 — PASS（含 r01 中两项 GAP 已在 head 关闭）

| 验收点 | head 证据 | 结论 |
|---|---|---|
| 权重顺序与 tier 结构 | 未变（`scoring.rs` tier/points/bonus 位域、`weights` 常量原样）；r01 PASS 维持 | **PASS** |
| pinned 仅相近时提升、不跨 tier | 未变；`tests.rs:448-474` `pinned_boost_does_not_jump_tier` 原样 | **PASS** |
| `manual_weight` 有限 tie-break、不压过 name-exact | 跨 tier 半分：`tests.rs:477` `manual_weight_does_not_overpower_name_exact`（别名 pinned+max-weight 不敌 name-exact，断言 id 序 `[2,1]`）；同 tier 半分：`tests.rs:501` `manual_weight_is_a_bounded_within_tier_tie_break`（weight 100 vs -100，断言高权重在前且 **total_score 不相等**，证明经 bonus 位段决定、非 tiebreak） | **PASS** |
| stable tie-breaker | `tiebreak`（`scoring.rs` head）四分支与文档注释逐项对齐：`right.manual_weight.cmp(&left.manual_weight)`（降序）→ `right.pinned.cmp(&left.pinned)`（true 先）→ `right.open_count.cmp(&left.open_count)`（降序）→ `left.id.as_uuid().cmp(&right.id.as_uuid())`（升序）；被 `mod.rs` 排序以 `total_score` 降序后调用。三个表驱动单测 + 一个端到端同分用例锁定（见 F001/F002）。**r01 的 GAP（方向相反）已在 head 关闭** | **PASS** |
| 输出 range / UTF-8 / 高亮开关 | 未变；r01 PASS 维持 | **PASS** |
| 拼音/编辑距离确定性 | 未变；pinyin 首读音文档补入 `keys.rs`（F005）；r01 PASS 维持 | **PASS** |

### M02.3/.4/.5（排除核验）— 未实现，同 r01（diff 未引入），非目标无回归。

## 按 finding 的复审结论（r01 F001–F005）

### F001（High）— **CLOSED**（代码修复 + 守门测试均核验）

**改动核验（scoring.rs head）：** `tiebreak` 现为

```rust
right.manual_weight.cmp(&left.manual_weight)
    .then_with(|| right.pinned.cmp(&left.pinned))
    .then_with(|| right.open_count.cmp(&left.open_count))
    .then_with(|| left.id.as_uuid().cmp(&right.id.as_uuid()))
```

- `manual_weight` 项：`right.cmp(&left)` → 降序（高权重者返回 `Less` → 排前）。与相邻 `pinned` 项惯用法一致。✓
- `open_count` 项：`right.cmp(&left)` → 降序（高 open_count 者排前）。**F001 正文中被反转、用户可见的正是此项；现已纠正。** ✓
- `pinned`（`right.pinned.cmp(&left.pinned)`，true 先）`id`（`left.id.cmp(&right.id)`，升序）不变，与文档一致。✓
- 文档注释（`scoring.rs` `tiebreak` 上方）为 `Order: manual_weight descending → pinned (true first) → open_count descending → entry id ascending`，与实现逐项对齐。✓

**守门测试正确性核验（scoring.rs 测试模块，head）：** 三个表驱动测试**直接调用** `tiebreak`，方向逐项可证：

1. `tiebreak_sorts_manual_weight_descending_then_pinned`：`weighted(1,100)` vs `weighted(2,0)` 断言 `Less`（100 排前）；`weighted(1,-100)` vs `weighted(2,0)` 断言 `Greater`（-100 排后）；等 weight 落 pinned：pinned-left `Less`、pinned-right `Greater`。✓（反向错误时上述断言全部失败——守门有效，且对旧方向确有区分力。）
2. `tiebreak_sorts_open_count_descending_then_id_ascending`：open 100 vs 0 断言 `Less`；翻转断言 `Greater`；等 count 落 id：`entry(1)` vs `entry(2)` 断言 `Less`、翻转 `Greater`。✓
3. `tiebreak_order_is_total_and_consistent`：链内跨 weight→pinned→open_count→id 四段单步断言 + 反对称三对（镜像返回 `Greater`）。✓

**关于「测试在修复前会失败」的独立复核：** 旧实现 `left.manual_weight.cmp(&right.manual_weight)` 下，`weighted(1,100)`（left）vs `weighted(2,0)`（right）返回 `Greater`，而新测试断言 `Less` → 修复前确实失败；3 号测试对 `entry(1)` vs `entry(2)` 在旧 id 分支下仍 `Less`，但 weight/pinned/open_count 段的反向断言在旧方向下全部失败。**结论：新测试真实承担 F001 回归守门，非空断言。**

**端到端用例（`same_score_tie_break_uses_open_count_then_id`，tests.rs:534）：** 4 条同 name-exact、"USB Driver"、同 `manual_weight=0`、同 `pinned=false` 的条目仅 `open_count` 与 `id` 不同。**逐对 `total_score` 相等断言（`results.windows(2)` 内 `assert_eq!(pair[0].total_score, pair[1].total_score)`）确证评分完全相同、由 tiebreak 决定序**（而非 tier/points）；最终序 `vec![3, 2, 4, 5]` = open_count 100(id3) > 50(id2) > 1(id4) > 0(id5) 降序、等 count 时 id 升序。id5(plain, open 0) 与 id4(open 1) 分居位次 4 与 3，正确落在降序。**该断言作用于最终结果顺序（`ids(&results)`），确证用户可见序。** ✓

**正确性再推演：** `open_count` 不参与 total_score 位域（tier/points/bonus 三段均不含），同 (tier, points, bonus) 的常见场景落第 3 分支，现在返回「常用者先」，与 README/REQ-SEARCH-011「最近使用优先」一致。`manual_weight` 分支经 `search()` 同分不可达（bonus 位段差异 → total_score 必不等）仍成立，属潜伏分支，已由直接单测锁定方向。**F001 关闭。**

### F002（Medium）— **CLOSED**（死代码清理 + 同分路径端到端覆盖核验）

- `git diff` 确认 `let _ = (unweighted, weighted);` 与其夹具行（旧 `tests.rs:483-485`）**已整体删除**；`alias_boosted_manual_weight` 辅助函数内联回 `manual_weight_does_not_overpower_name_exact`（现 `tests.rs:477-499`），保留原「pinned+max-weight 别名不敌 name-exact」跨 tier 断言并更新 id 序为 `[2,1]`。✓
- **r01 所述「死夹具不触达 tiebreak」这一具体 GAP 已被独立关闭**：新增 `same_score_tie_break_uses_open_count_then_id`（同分 → tiebreak 决定序，覆盖 `open_count`/`id` 两个经 `search()` 可达的分支）+ `scoring.rs` 直接 `tiebreak` 表驱动用例（覆盖 `manual_weight`/`pinned` 分支），合为 r01 要求的分支全集。`manual_weight_is_a_bounded_within_tier_tie_break`（tests.rs:501）实测验证「weight 是 within-tier bonus、由 total_score 位段决定、不跨 tier」，并明确以 `total_score` 不相等断言区分「平分由 tiebreak 决定」与「weight 由评分位段决定」两条路径——语义边界清晰。**F002 关闭。**

### F003（Low）— **CLOSED**（护栏 + 回归测试核验）

- **实现核验（matching.rs head）：** `pub(crate) const MAX_EDIT_DISTANCE_LEN: usize = 64;` 带两级文档（模块 `///` 与 `edit_distance_match` 函数文档），说明成本逻辑（O(|p|·|k|) 整键 DP、32_767 路径、64 远超现实 typo 容差窗口）且标注为 M02-B 前置护栏。✓
- **守卫逻辑核验：** ① 整键分支 `pattern.chars().count() <= 64 && key_text.chars().count() <= 64 && levenshtein(...) <= max` —— 任意一侧超长（32k 路径、整段粘贴 token）即跳过整键 DP；② **word 分支同样加守卫**：`split_whitespace().filter(|word| word.chars().count() <= MAX_EDIT_DISTANCE_LEN)` —— 阻止「未加空格的长路径块借 word 语义绕过整键护栏」的旁路。两侧守卫完整，可避免 32k×32k ≈ 10^9 单元/条 的最坏情形。✓
- **测试核验（`whole_key_edit_distance_is_bounded_by_length_guard`，matching.rs）：** ① `C:\x`×2000 + 空格 + `driver` 的长键内含可达短词 `driber`→`driver`（dist 1）仍命中（**word 分支保留，护栏不误伤**）；② 长 token + 长键断言不误报命中（整键 DP 被跳过）；③ 短键 `driber` vs `usb driver`（dist 1）仍命中（**短词容错不变**）；④ 超长无空格键（单 word 超界）断言整键/word 均被护栏跳过（不跑 DP）。✓
- **不误伤结论：** 64 chars 之上为「理论可命中、现实不合理的整键 DP」，0.0.1 既有全部用例（≤64）不变，行为无损；10k 基准仍为 M02-B 待办（与 r01 界定一致）。**F003 关闭。**

### F004（Low）— **CLOSED**（公共文档核验）

`mod.rs` `search()` `///` 文档（head）新增：*"For multi-token queries the result's tier is determined by the **weakest matched token** (the worst tier among the matched tokens), while every matched token still contributes its points — so a result whose second token only hits an editable field is ranked as that (weaker) tier, never promoted by the stronger token."* 与实现 `scoring.rs` `best_tier = current.max(tier_of(...))`（weakest link 定级、每 token 仍计 points）**语义一致**。对调用方（M03 presenter/RankedResult 消费者）足够透明。**文本存在且准确。** 关于该项是否构成「设计说明」而非缺陷：r02 维持 r01 结论——这是可辩护的多 token AND 折衷（名字再准、另一 token 只命中弱字段时整体相关度本就更低），非行为缺陷；补文档即满足本 milestone 的可审计性要求。**F004 关闭。**

### F005（Low）— **CLOSED**（文档核验）

`keys.rs` 模块文档（head）新增段：pinyin 用 `0.11.0`、`plain` 特性、**无 `heteronym` 表**、每字取**第一读音**（确定性、固定映射、不影响评分确定性）、卷舌声母只取首字母（`zh` → `z`）、非首读音搜索可能不命中并记为已知限制与手工验收项（M02-B 可选启用 `heteronym`）。与 r01 对抗项 3 的 crate 源码核实结论一致。**首读音语义在 0.0.1 属可接受的确定性设计说明**，不构成缺陷，无需代码修复。**F005 关闭。**

## Cross-cutting 检查（head）

- **正确性：** tie-break 方向（F001 核心）已在代码与文档逐项对齐并经直接单测 + 端到端同分用例双向锁定；`total_score` 构造、排序调用点（`mod.rs`）未变。总分排序 = 评分降序 → tiebreak，仍总序一致（单测 3 号链式 + 反对称佐证）。
- **错误处理：** 无新增 unwrap/expect/panic；`manual_weight_bias` 的 `try_from(...).unwrap_or(u64::MAX)` 为不可达兜底（weight+100 ∈ [0,200]）未变。
- **数据安全：** 无磁盘读改、无真实目录删除面；派生键仍每查询内存派生不落盘（无 `fs` 引用，grep 确认零新增）。修复不触碰用户数据。
- **隐私：** 无路径/搜索词/配置记录、无网络/遥测/时间引用；`src/search/` 零 `std::fs/std::net/std::time/std::env/std::process/std::thread`（r01 已 grep，本轮 diff 未引入任何此类引用）。
- **安全性：** 编辑距离护栏消除 32k 路径上的 O(len²) 放大（F003）；无新攻击面；无新依赖。
- **Windows 行为：** 无平台 API；MSVC CI 全绿（run 35820715554）。
- **测试覆盖：** 静态计数与 CI 日志 132+1=133 完全吻合；**无 `#[ignore]`/`#[should_panic]` 占位**（r01 grep 过全模块，diff 未引入）；6 项新测试全部有实际断言且至少一项（scoring 表驱动）对旧方向具区分力；既有 126 项保持通过（CI 全绿证明），tier 单调性（`tier_rules_match_required_weight_order`）、manual_weight 不压过 name-exact（改写后仍通过）、pinned 不跳 tier、UTF-8 高亮、开关、稳定性等关键既有测试**未弱化**（逐一核对 `manual_weight_does_not_overpower_name_exact` 改写仅替换夹具与 id 序，非删除断言；`same_score...` 新增而非替换既有先例）。
- **性能：** 编辑距离路径已护栏；pinyin 键派生成本同 r01 记录；10k 基准仍归 M02-B。
- **可访问性 / 可维护性：** 无 UI 变更；分层与注释先行未退化；`tiebreak` 现在是全模块注释最清晰的函数之一；`MAX_EDIT_DISTANCE_LEN` 常量文档完备。
- **质量检查（head）：** `git grep '#\[allow' src/search/` 零命中；`HashMap/HashSet/BTreeMap/BTreeSet` 零命中（无死循环/顺序依赖引入）；`MAX_EDIT_DISTANCE_LEN` 已文档化并设 `pub(crate)` 作用域（内部护栏，未泄漏公共 API）。

## 未能自动验证的项（同 r01，维持）

1. **pinyin 多音字语义（含 `zh→z` 首字母）与真实中文路径命中**需真实桌面/用户验收确认是否符合期望（F005 观察；当前为确定性首读音，属可解释的已知限制，已文档化并挂手工验收项）。
2. **10,000 条基准（M02-B）**：本 slice 无 benchmark；编辑距离护栏后的真实中位数/P95 与 pinyin 键派生开销仍由 M02-B release CI 或专用 harness 验证（不能用 debug 结果）。
3. **真实搜索输入框体验**（IME composition、快速连续输入的旧结果覆盖/取消）属 M02-B/M03，本 slice 纯核心无 UI。
4. 无 GUI；托盘、快捷键、多显示器/DPI 等桌面验收不在本搜索核心范围。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M02-A 纯搜索核心；非发布批准）

- **range 与独立性：** fix diff 精确命中 `src/search/{scoring,tests,matching,mod,keys}.rs` 5 个代码文件；domain/storage/lib/Cargo/CI 零变更。`src/search/` 全历史 = 2 个提交（实现 + fix），reviewer 对两者均未参与。
- **CI：** head `093941a` 的 MSVC 门禁 run `35820715554` 全绿：fmt / clippy `-D warnings` / **133 项测试通过 0 失败**（lib 132 + main 1，与静态计数逐项吻合）/ release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact。链上 6 个中间 run 被取消全部因文档-only supersede，不改变构建输入，已透明记录。
- **Finding 处置：** F001（High）**CLOSED**；F002（Medium）**CLOSED**；F003/LOW **CLOSED**；F004/LOW **CLOSED**；F005/LOW **CLOSED**。无新增 finding。
- **需求映射：** M02.1/M02.2 全部验收点在 head 保持满足（含 r01 中「stable tie-breaker」「manual_weight 有限 tie-break」两项由 GAP 转 PASS）；M02.3/4/5 显式排除无回归。
- **关于「result tier = 最弱 token」与「pinyin 首读音」两项设计说明的裁定：** 两者均属可辩护的 0.0.1 确定性设计，非行为缺陷；已在公共 API 文档（`mod.rs`）与模块文档（`keys.rs`/`scoring.rs`）明确记录，满足本 milestone 的可审计性与契约透明要求。**本 milestone 无需进一步代码修复**；M02-B 可选的 `heteronym` 多音字表与 10k 基准作为后续项记录。
- **非发布批准：** 依 CLAUDE.md §4.5，本结论仅 `APPROVED_FOR_MILESTONE`，不构成 `APPROVED_FOR_RELEASE`；发布需后续 Release Candidate 独立发布评审、真实桌面手工验收及对应响应闭环（含本 review「未能自动验证的项」）。
