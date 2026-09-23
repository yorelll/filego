# Response: M02-A 纯搜索核心（Round 01）

## Metadata

- **版本:** `0.0.1`
- **里程碑/topic:** `m02-search`
- **轮次:** `r01`
- **Implementation agent:** M02-A 修复 implementation agent（仅实现、补测与触发 CI；未参与 `m02-search-review-r01` 评审，修复期未做评审）
- **对应 review 文档:** [`m02-search-review-r01.md`](m02-search-review-r01.md)
- **评审前（base）commit SHA:** `18c48f5040193be3990251ad8a9c7a11f57efa7a`
- **修复后（head）commit SHA:** `eaba66d`（消息 `fix: correct tie-break order and add tie-break coverage`）＋随后的 response 文档提交 `c0de7e5`
- **Response 日期:** 2026-09-21

## Summary

| Finding | 严重级 | Assessment | Status |
|---|---|---|---|
| F001 | High | `ACCEPTED` | `tiebreak` 的 `manual_weight`/`open_count` 改为降序，与文档及相邻 `pinned` 惯用法一致（`right.x.cmp(&left.x)`）；补打分模块表驱动单元测试逐项锁定全部方向 |
| F002 | Medium | `ACCEPTED` | 清理 `tests.rs:477-485` 死代码；新增同分同 tier 端到端用例（open_count desc → id asc，实际经 `search()` 可达的 tiebreak 分支）＋打分模块直接 `tiebreak` 表驱动用例（manual_weight/pinned 分支） |
| F003 | Low | `ACCEPTED` | 新增 `MAX_EDIT_DISTANCE_LEN = 64` 护栏：整键分支与 word 分支均跳过超长串的 O(len·len) DP；配护栏回归测试 |
| F004 | Low | `ACCEPTED` | `search()` 公共文档补「result tier = 命中 token 中最弱 tier」规则 |
| F005 | Low | `ACCEPTED`（文档观察） | `keys.rs` pinyin 模块文档补「多音字取第一读音、确定性、zh→z 首字母」说明并记为手工验收项；无代码变更 |

无 `REJECTED` / `PARTIALLY_ACCEPTED` 条目。5 个 finding 全部接受并落地。

> **范围说明（对 F002 的关键澄清）**：review 建议在端到端用例中按「manual_weight desc → pinned → open_count desc → id asc」的完整文档顺序断言 tiebreak。经核实，`manual_weight` 与 `pinned` 是 **total_score 的 bonus 位段**（`build_score`，`scoring.rs`），二者不同 ⇒ bonus 位段不同 ⇒ `total_score` 必不相等 ⇒ **经 `search()` 可达的同分必然 weight/pinned 相同**。这正是 review 自身在 F001 中论证的「manual_weight 分支经 `search()` 不可达（潜伏）」的延续。因此：
> - 经 `search()` 实际可达的 tiebreak 分支只有 **open_count（非评分字段）与 id** —— 端到端同分用例只能覆盖这两项（而 F001 里被真正反转、用户可见的正是 open_count 分支，已由端到端用例直接回归）；
> - `manual_weight` / `pinned` 两个分支改由 **直接调用 `tiebreak` 的表驱动单元测试**覆盖（review 原文「补充 `tiebreak` 表驱动单元测试」），逐项锁定方向。
> 两类测试合起来即 review 要求的分支全集（open_count 高者先 / pinned true 先 / manual_weight 高者先 / id 小者先 / 组合 + 反对称）。

## 变更范围

- `src/search/scoring.rs` — F001 修复 + 3 项 `tiebreak` 表驱动单元测试（含 fixture 辅助函数）。
- `src/search/tests.rs` — F002：清理死代码、改写 `manual_weight_does_not_overpower_name_exact`、新增 2 个端到端用例。
- `src/search/matching.rs` — F003：`MAX_EDIT_DISTANCE_LEN` 护栏 + 模块/函数文档 + 1 项护栏测试。
- `src/search/mod.rs` — F004：公共文档补「最弱 token 定级」规则。
- `src/search/keys.rs` — F005：pinyin 模块文档补第一读音说明。

未改动 `src/domain/*`、`src/storage/*`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/*`、`build.rs`。基于 `18c48f5` 的合法 diff 精确等于上述 5 个 `src/search/` 文件（另加 2 份 review/response 文档）。

## Finding Responses

### `F001` — `ACCEPTED`（High）

- **评估:** 属实。`tiebreak` 的 `manual_weight`/`open_count` 两项写了 `left.cmp(&right)`（升序），与函数文档及相邻 `pinned` 项的降序惯用法 `right.cmp(&left)` 相反。open_count 不参与 total_score，故「同 (tier, points, bonus)」的常用场景必然落第 3 分支，结果是最常用者排后；经 `search()` 可到达（可达，High）。
- **修改（scoring.rs）:**
  - `manual_weight` 项改为 `right.manual_weight.cmp(&left.manual_weight)`（降序；高权重优先）；
  - `open_count` 项改为 `right.open_count.cmp(&left.open_count)`（降序；最近/最常用优先）；
  - `pinned` 与 `id` 项不变（`right.pinned.cmp(&left.pinned)`、`left.id.cmp(&right.id)`，与文档一致）。四分支现在与文档注释「manual_weight descending → pinned (true first) → open_count descending → entry id ascending」逐项对齐。
- **新增测试（scoring.rs 测试模块，直接调用 `tiebreak`）:**
  - `tiebreak_sorts_manual_weight_descending_then_pinned` — weight 100 < weight 0 < weight -100；等 weight 落到 pinned：true 先于 false，双向断言；
  - `tiebreak_sorts_open_count_descending_then_id_ascending` — open_count 100 < 0；等 count 落到 id：小者先，双向断言；
  - `tiebreak_order_is_total_and_consistent` — 一个链内同时跨 weight→pinned→open_count→id 四段 + 反对称断言。
- **验证:** 上述测试在修复前会失败（旧方向），修复后全绿；GNU 全量 suite 通过（见下）。未修复前的反例已由评分测试 dir 观察（左项更大返回 `Greater` → 排后 → 常用者靠后），修复后 `same_score_tie_break_uses_open_count_then_id` 端到端回归锁定 open_count 降序。

### `F002` — `ACCEPTED`（Medium）

- **评估:** 属实。`manual_weight_does_not_overpower_name_exact` 构造的 `unweighted`/`weighted`（`tests.rs:483-484`）被 `let _ = (…)`（`485`）丢弃，实际委托 `alias_boosted_manual_weight`（不同 tier fixture），永不触达 tiebreak；且全部既有 fixture `open_count=0`/`manual_weight=0`，`tiebreak` 各分支在既有测试中完全不可达——F001 因此未被拦截。
- **修改（tests.rs）:**
  - 删除 `let _ = (unweighted, weighted)` 死代码，并将 `alias_boosted_manual_weight` 辅助函数内联回主测试（保留原「pinned+max-weight 别名不敌 name-exact」跨 tier 断言，这是 `manual_weight` 不压过 name-exact 的跨 tier 半部分）；
  - 新增 `manual_weight_is_a_bounded_within_tier_tie_break` — 两条同 name-exact、weight 100 vs -100：断言高权重在前，且 **total_score 必不相等**（bonus 位段），证明 weight 是「within-tier bonus、经评分决定、不跨 tier」，而非依赖 tiebreak；
  - 新增 `same_score_tie_break_uses_open_count_then_id` — 4 条同 name-exact、同 weight、同 pinned 的条目，**total_score 逐对相等**（确证 tiebreak 决定序），断言 open_count 降序（100 > 50 > 1 > 0）再 id 升序。这是经 `search()` 真正可达、且正是 F001 被反转的分支的端到端回归。
- **与 F001 的联动:** scoring.rs 的直接 `tiebreak` 表驱动用例补齐了 manual_weight/pinned 分支（与端到端用例合为分支全集），覆盖 review 要求的全部方向。

### `F003` — `ACCEPTED`（Low）

- **评估:** 属实。整键 Levenshtein 为 O(|token|·|key|)，key 最长可达 `MAX_PATH_LEN = 32_767`（`folder.rs:10`），10k 条目（M02-B）前若不护栏，单查询最坏 O(10k·|t|·|path|)，有 >50ms 风险。
- **修改（matching.rs）:**
  - 公共常量 `MAX_EDIT_DISTANCE_LEN: usize = 64`，带文档说明成本逻辑与「64 远超可达 typo 容差窗口（max ≤ 2 内 ~20 字词的错拼），跳过不改变可达结果」；
  - 整键分支加守卫：仅当 `pattern` 与 `key_text` 均 ≤ 64 chars 才跑 O(|p|·|k|) DP；
  - **word 分支同样加守卫**：`split_whitespace()` 后 `filter(|word| len ≤ 64)`——否则一个未加空格的长路径块可借「word」语义绕过整键护栏，O(len²) 依旧可达；
  - 模块文档与 `edit_distance_match` 文档同步说明护栏（标为 M02-B 前置护栏，短词容错不受影响）。
- **新增测试:** `whole_key_edit_distance_is_bounded_by_length_guard` — 长键内含可达短词仍能命中（word 分支保留）；长 token vs 长键不跑整键 DP（不被误报命中）；短键 typo 容错不变；超长无空格键被护栏整体跳过。
- **说明:** 本修复对 0.0.1 行为无破坏（≤64 chars 的一切既有用例不变；超长输入的原行为是「理论上可命中」，现实为已不合理的整键 DP，属于 M02-B 前的修复而非行为回退）。10k 基准仍为 M02-B 待做。

### `F004` — `ACCEPTED`（Low）

- **修改（mod.rs）:** `search()` 公共 `///` 文档新增明确规则：多 token 查询的 result tier 由**命中 token 中最弱（worst）tier** 决定，同时每个 token 仍贡献 points——副 token 只命中较弱字段时，结果按该较弱 tier 定级，不被强 token 提升。与 `scoring.rs:167-173` 的实现注释一致。

### `F005` — `ACCEPTED`（Low，文档观察）

- **修改（keys.rs）:** pinyin 模块文档新增：`pinyin 0.11.0`、`plain` 特性、无 `heteronym` 表，多音字取**第一读音**（确定性）、卷舌声母只取首字母（`zh` → `z`）；非首读音搜索可能不命中，记为已知限制与手工验收项（M02-B 可选启用 `heteronym`）。无代码变更。

## 本地 GNU 验证证据（快速反馈，非 MSVC 权威）

- **Toolchain:** `1.92.0-x86_64-pc-windows-gnu`（Rust 1.92.0, cargo 1.92.0）；预检确认已安装 `x86_64-pc-windows-gnu`、`rustfmt`、`clippy`；`RUSTUP_AUTO_INSTALL=0`、`RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-gnu`，未经 rustup 下载任何组件。
- **结果（本会话均 PASS）:**
  - `cargo fmt --all` 且 `cargo fmt --all -- --check` — PASS；
  - `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` — PASS；
  - `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` — PASS，**lib 132 + main 1 = 133 项通过，0 失败**（base 126 + 新增 6：scoring 3 + tests.rs 2 + matching 1）；
  - `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` — PASS。
- **环境备注（本会话，非提交内容）:**
  - GNU linker 漂移出现，按已有会话设置解决：`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"`、`PATH` 前置 `D:\mingw64\bin`。**本次漂移先以资源编译错误形式出现**：`winresource`（`build.rs` 的 Windows 资源步骤）需 `windres`/`ar`；将其通过 `PATH` 暴露（`D:\mingw64\bin`）加 `WINDRES`/`AR` 显式指向后，改以已知的 `cannot find -lshlwapi` 出现，再用上述 linker 覆盖解决。
  - 本机 GNU 通过不替代远程 MSVC CI；本地产物仅做开发检查（CLAUDE.md §3.1）。

## CI 证据（远程 MSVC）

| commit SHA | Workflow | run ID/URL | 结果 | 说明 |
|---|---|---|---|---|
| 修复代码 `eaba66d` + response 文档 `c0de7e5`..`42b7596`（仅文档，构建输入与 head 相同） | [Windows CI](https://github.com/yorelll/filego/actions/runs/35820539585) | `35820539585` | `进行中`（head `42b7596`，含全部修复代码与文档） | 早期 run（`35820240269`/`35820401923`/`35820488883`）因文档-only 推送被取消；最终 head `42b7596` 的 run 为权威 MSVC 依据，构建输入与修复代码 head `eaba66d` 相同。GNU 全绿仅证明本地快速反馈；MSVC 门禁结论以该 run 为准，监控至结束并回填（CLAUDE.md §3.2/§3.4）。 |

## 未解决事项与待人工验证项

1. **F005（多音字首读音）为手工验收项:** 「乐」「行」等多音字以非常用读音拼音搜索可能不命中（取第一读音、确定性）；`zh/ch/sh` 首字母取 `z/c/s`。需真实桌面/用户确认是否符合预期；M02-B 可选启用 `heteronym`。
2. **F003 的 10k 基准仍属 M02-B:** 护栏已把超长串移出 DP 路径；但 10k 条目上的 pinyin 键派生、匹配策略命中率的真实中位数/P95 仍需 M02-B release 模式基准（不能用 debug 结果代替）。
3. **真实搜索输入框体验（IME composition、连续输入取消/旧结果覆盖）** 与结果排序的桌面观感属 M02-B/M03，本 slice 为纯核心无 UI。
4. 正常情况下 GNU 全绿 + 待 MSVC CI 通过后，请求 r02 复审。**本修复不构成发布批准**；`APPROVED_FOR_RELEASE` 需后续 Release Candidate 发布评审与真实桌面手工验收。

## r02 复审请求

已逐条落实 `m02-search-review-r01` 的 F001–F005（全部 `ACCEPTED`，无拒绝/部分接受）。F001 的 reachable 分支（open_count desc）由端到端同分用例回归，manual_weight/pinned 分支由直接 `tiebreak` 表驱动单元测试覆盖（见 F001/F002 小节对"经 search() 可达性"的澄清）。全部既有 126 项测试保持通过，新增 6 项全绿。恳请原 reviewer（或另一名独立 reviewer）对修复 commit `eaba66d`（+本 response 文档）的 diff、新测试与 CI 证据复核，并以 `APPROVED_FOR_MILESTONE`（或继续 `CHANGES_REQUESTED`）作结。
