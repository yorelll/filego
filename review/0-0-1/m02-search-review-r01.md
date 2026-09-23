# Review: M02-A 纯搜索核心（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m02-search`
- **轮次：** `r01`
- **日期：** 2026-09-21
- **Reviewer：** 独立 code-review agent（M02-A r01）
- **独立性声明：** 本 reviewer 未参与 M02-A 实现 commit `18c48f5`，也未参与 M00/M01 的任何实现、修复、测试、提交、推送、CI 监控或 response 撰写；本仓库 `src/search/` 的全部历史仅为 `18c48f5` 单个提交（`git log --all -- src/search/` 仅一条），reviewer 未以任何形式（含会话压缩/重命名/换名字）实现过其中代码。本次仅作只读评审：未编辑源代码、未运行 `cargo`（严格依照约束 3.1 未执行任何本地 Rust 命令）、未提交或推送；除本 review 审计文档外未写入其他文件。
- **Base SHA：** `716abd904a…`（M01-B approval head）
- **Head SHA（评审对象）：** `18c48f5040193be3990251ad8a9c7a11f57efa7a`
- **比较范围：** `716abd9..18c48f5`
- **审查文件（代码）：** `src/search/{mod,query,keys,matching,scoring,highlight,search_entry,tests}.rs`（全新）、`src/lib.rs`（+`pub mod search`）、`Cargo.toml`、`Cargo.lock`（新增依赖 `pinyin = "=0.11.0"`）。
- **审查文件（审计/上下文）：** `review/0-0-1/m01-storage-review-r02.md`、`review/0-0-1/m01b-persistence-review-r02.md`（约定与格式）、`task/01-里程碑任务清单.md`（M02.1/M02.2 验收点）、`task/02-验收与测试矩阵.md`（REQ-SEARCH-001..019）、`src/domain/settings.rs`、`src/domain/folder.rs`、`src/domain/ids.rs`、`deny.toml`、`.github/workflows/ci.yml`、pinyin crate 本机源码缓存（`D:\Users\lawrence_lv\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\pinyin-0.11.0`）。
- **审查方法：** 完整阅读 head 上 `src/search/*.rs` 全部源码与测试；核对 `716abd9..18c48f5` 完整 diff 与统计；用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询并核验 CI run `35816387166` 的 head SHA、workflow、单 job 全部步骤与 `test result` 日志行；读取 pinyin 0.11.0 crate 源码验证多音字确定性、`plain` 特性门控与无运行时依赖；对评分位域布局、高亮映射、开关语义做数值与语义推演；未以 implementation 的 response/summary 代替代码核验。

## CI 证据

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35816387166](https://github.com/yorelll/filego/actions/runs/35816387166) | `18c48f5040193be3990251ad8a9c7a11f57efa7a` | `Windows CI` / `fmt, clippy, test, release, package` | `success`（completed，push event） | 单 job 全部步骤 success：`Check formatting`、`Run Clippy with warnings denied`、`Run tests`、`Build release executable`、`Verify release outputs and version helper`、`Install pinned dependency audit tools`、`Audit advisories, licenses, bans, and sources`（cargo-deny）、`Generate third-party license inventory`（cargo-about）、`Sanity-check third-party license inventory`、`Build portable development artifact`、`Upload portable development artifact`。 |

- **MSVC 门禁核验：** `.github/workflows/ci.yml` 顶层 `TARGET: x86_64-pc-windows-msvc`（`ci.yml:22`），toolchain 安装含 target（`ci.yml:39`），clippy（`83`）/ test（`86`）/ release build（`89`）/ EXE 路径（`94-95`）/ portable 产物（`127`）全部显式一致使用 `x86_64-pc-windows-msvc`。符合 CLAUDE.md §3.2。
- **测试实际执行（run log 实测，非仅编译）：** `Run tests` 步骤输出 `test result: ok. 126 passed; 0 failed`（lib，含 69 项既有 + 57 项 `src/search/*` 新增）+ `test result: ok. 1 passed`（main bin，`filego-version.rs`）+ 2 个 `0 passed`（doc-test 二进制）。**127 项测试全部通过，0 失败。** 与 `#\[test\]` 静态计数完全吻合：base 上 lib 非搜索测试 69 项（app 8 / domain 1 / storage repo 33 / storage 26 / version 1），head 净增 57 项搜索相关测试（tests.rs 36 / keys.rs 7 / matching.rs 5 / query.rs 5 / scoring.rs 4），合计 lib 126。
- **许可证门禁：** cargo-deny `Audit…` 与 cargo-about `Generate third-party license inventory` step 均 success。新建依赖 `pinyin 0.11.0` 为 MIT（crate `LICENSE` = The MIT License, © 2016 mozillazg），`deny.toml` 的 `allow` 列表含 `"MIT"`（`deny.toml:21`）。pinyin 无任何 `dependencies`/`build-dependencies`（其生成表代码在 `build.rs`，产物为静态内联 `PINYIN_DATA`），Cargo.lock 中 `pinyin` 包项无依赖边，运行时不引入新树。`allow`/`deny` 通过与 CI 全绿一致，无需改动配置。
- **CI 结论：** head `18c48f5` 通过 MSVC 质量门禁（fmt / clippy `-D warnings` / 127 项测试 / release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact 打包上传）。本证明该 head 在 MSVC 环境可构建、可测试、可打包；不证明 10k 性能、真实桌面 IME/键盘行为等（见「未能自动验证的项」）。

## 变更范围核验（无 scope creep）

`git diff 716abd9..18c48f5 --name-only` 精确为 11 个文件：

| 文件 | 类型 | 说明 |
|---|---|---|
| `Cargo.toml` | +1 行 | 新增 `pinyin` 依赖（`=0.11.0`, `default-features=false`, `features=["plain"]`），MIT |
| `Cargo.lock` | +7 行 | 新增 `pinyin` 包项，无依赖边 |
| `src/lib.rs` | +1 行 | `pub mod search;` |
| `src/search/mod.rs` | +76 | 模块入口与 `search()` 编排 |
| `src/search/keys.rs` | +295 | 派生键（folded / pinyin full / pinyin initial / english initial）+ `origins` 映射 |
| `src/search/matching.rs` | +415 | 8 种策略 + Levenshtein + 每 token 最佳命中 |
| `src/search/scoring.rs` | +350 | 分 tier 评分 + u64 位域 + tie-break |
| `src/search/query.rs` | +165 | 查询解析与归一化 |
| `src/search/search_entry.rs` | +148 | 输入/输出类型 |
| `src/search/highlight.rs` | +86 | UTF-8 安全高亮 |
| `src/search/tests.rs` | +761 | 36 项集成风格测试 |

`src/domain/*`、`src/storage/*`、`src/app.rs`、`src/presentation/*`、`src/platform/*`、`.github/workflows/ci.yml`、`deny.toml` **均未变**。新增搜索层通过 `domain::settings::AppSettings` 与 `domain::folder::FolderId/MIN/MAX_MANUAL_WEIGHT` 只读依赖既有 domain，无回改。**无 scope creep。** M02.3/4/5 未实现（无筛选矩阵、无空查询展示、无 10k 基准/取消），未混入 M02-A。

## 需求/验收标准映射

验收点来源：`task/01-里程碑任务清单.md` M02.1/M02.2（标 ASP pending review）与 `task/02-验收与测试矩阵.md` REQ-SEARCH-001..019。

### M02.1 查询处理

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| trim 首尾空白；连续 Unicode whitespace 合并为 token 分隔 | `query.rs:65-87`：`is_whitespace()` 按当前 token 边界切分，连续空白只产生一次分隔；解析测试 `query.rs:110-140` | **PASS** |
| 大小写无关，支持中英文混合；normalization 避免破坏路径显示 | `query.rs:89-102`：纯 ASCII token 小写（Latin）；含非 ASCII token 逐字 `fold_char`（`keys.rs:33-39`，仅 1:1 单字小写折叠，多字折叠保留原字，键与显示串保持 1:1，显示串从不变异）；测试 `query.rs:143-164`、`tests.rs:95-114`、`keys.rs:239-252` | **PASS** |
| 多 token AND；每个 token 可命中不同字段 | `matching.rs:329-351`：`best_hit_for_all_tokens` 对每 token 取 `?`，任一失败整体无结果；`mod.rs:52`。跨字段测试 `tests.rs:351-370`（token "usb" 命中 Name、token "installer" 命中 Path） | **PASS** |
| 策略：exact/prefix/substring/拼音全拼/拼音首字母/英文首字母/ordered-subsequence fuzzy/受控编辑距离 | `matching.rs:32-56` 枚举 8 策略；`exact/prefix/contains`（`89-110`）、`subsequence`（`114-147`，有序非连续）、`pinyin full/initial/english initial`（`294-322`，仅 Latin token 且键存在）、`edit_distance_match`（`175-187`）+ Levenshtein（`150-168`，char-based）；全策略测试 `matching.rs:353-415`、`tests.rs:164-269` | **PASS** |
| 别名与名称一样参与搜索；定义别名/路径/标签/备注的明确权重 | `search_entry.rs:72-76`（别名以同样方式建键）、`matching.rs` 对别名同样跑全策略；权重表见 M02.2 映射 | **PASS** |
| 可按设置关闭 fuzzy/路径/分类/标签/备注；拼音与编辑距离开关默认行为明确 | `matching.rs:241`（`fuzzy_matching` 门控 subsequence `269` 与 edit distance `277`）、`FieldKeys::new`（`keys.rs:156-175`：路径 `settings.search_paths`、分类/标签/备注对应 toggle、拼音 `search_pinyin`、英文首字母 `search_english_initials`）、`matching.rs:232`（`is_enabled` 门）；默认值 `settings.rs:47-61`（`search_notes=false`、fuzzy=true、pinyin=true、english_initials=true、`max_edit_distance=MAX_EDIT_DISTANCE`，`validate` 上限 2 `settings.rs:75-77`）；开关测试 `tests.rs:559-695` | **PASS** |
| 派生搜索键可扩展、文件搜索可复用，但 0.0.1 不搜索未添加文件 | `keys.rs:1-21`（模块文档）、`17-22`（每搜索内存派生、明确「never persisted」）；`src/search/` 无 `fs::write/File::/persist`（grep 确认，仅模块注释提及 "never persisted"） | **PASS** |
| 受控编辑距离不压过精确/前缀（REQ-SEARCH-008B） | 编辑距离为最底层 Tier::EditDistance（`scoring.rs:56`、`100`）；`tier_of` 保证任意 exact/prefix 严格在上位 tier；测试 `scoring.rs:244-298`、`tests.rs:400-445` | **PASS** |

### M02.2 评分与高亮

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 权重顺序：名称 exact/prefix/contains 优先；别名/拼音与首字母/标签/分类/路径/子序列/编辑距离依次 | `scoring.rs:44-57` 的 9 级 `Tier` 完全对齐 `task/01` M02.2 顺序与 `REQ-SEARCH-010`；`tier_of`（`86-102`）/ `strategy_points`（`105-115`）内 tier 次级顺序含 pinyin full 4 > pinyin initial=english initials 3 > subsequence 2 > edit distance 1；`weights` 公共常量（`118-131`）；tier 顺序测试 `scoring.rs:300-319`、tier 规则测试 `243-298`、端到端顺序测试 `tests.rs:399-445` | **PASS** |
| 名称精确/前缀严格高于别名、路径、编辑距离 | tier 结构性保证（Tier::Name=0 最低）；`tests.rs:400-417`（name-exact > name-prefix > alias）、`420-445`（name-prefix > path > edit-distance） | **PASS** |
| `pinned` 仅在相关性相近时提升，不跨 tier 跳级 | bonus 位域 `scoring.rs:176-187`：`manual_weight_bias` ∈ [0,200k]（`144-148`）+ 置顶 `PINNED_BONUS=10_000`（`128`），≤ 210_000 = BONUS_MAX（`140`），封装在 bits 20..37，远低于 POINTS 位域下界（bit 40）；bonus 永不进位到 points/tier；测试 `tests.rs:448-474`（置顶别名 hit 不得压过未置顶 name-prefix hit） | **PASS** |
| `manual_weight` 仅作有限 tie-break，不得压过精确名称命中 | 跨 tier 保证成立（同为 bonus 位域，最大 200k < 2^18=262144，只在同 tier 同 points 时生效，绝不跨 tier/points；测试 `tests.rs:476-508`）。字符串比较方向另行核对见 F001 | **PASS（跨 tier 部分）/ GAP（tie 内方向，见 F001）** |
| stable tie-breaker，重复查询顺序完全一致 | 确定性成立（纯函数无时间/随机/哈希序，`src/search/` 无 `HashMap/HashSet/std::time/std::env/std::process`（grep 确认零出现）；测试 `tests.rs:733-748`）。但 tiebreak 的 `manual_weight`/`open_count` 比较方向与自身注释「descending」相反，见 F001 | **PASS（确定性）/ GAP（tie 内方向，见 F001）** |
| 输出匹配 range；UTF-8/Unicode 边界不产生非法切片 | `matching.rs:69-87`：字节定位后一律换算成原串 **char 索引**；`map_key_range`（`79-87`）经 `origins` 映射回原字符串（`first_origin..last_origin+1`）；`highlight.rs:55-86` `compute_highlights` 最终用 `source.chars().count()` 校验 `0≤start<end≤char_count` 后才产出 range（`76-83`），杜绝越界/非法切片；pinyin `origins` 单调（`keys.rs:79-93`，测试 `280-286`）；混合中文+emoji 高亮测试 `tests.rs:514-526` | **PASS** |
| 高亮关闭时行为一致 | `highlight.rs:60-62`：`compute_highlights=false` 直接返空（不跑策略）；测试 `tests.rs:528-544`（开启/关闭两组 total_score 逐位相等、仅 highlights 空/非空差异） | **PASS** |
| 拼音/编辑距离无网络依赖；确定性 fixture 与分数（M02.2 未选项拆分说明） | pinyin crate `default-features=false, features=["plain"]`（`Cargo.toml:18`）；crate 无依赖树（见 CI 节）；`src/search/` 无网络/时间调用（grep 确认）。**说明：** `task/01` M02.2 列表项「拼音、首字母和编辑距离必须有中英文 fixture、确定性分数和 10,000 条性能基准」本身是跨 M02-A/M02-B 的验收点；其中「中英文 fixture 与确定性分数」在 M02-A 已覆盖（中文 pinyin/key 测试 `tests.rs:271-297`、`keys.rs:266-294`，中文编辑距离 `matching.rs:363`，确定性分数测试 `scoring.rs` 与 `tests.rs:399-474`），「10,000 条 release 基准」由任务清单明示属 M02-B 未做（`task/01`：「10k release 基准属 M02-B 待做」）。M02-A 范围内不含基准，非缺口。 | **PASS（范围界定）** |

### M02.3/.4/.5（排除核验）

- **M02.3 筛选矩阵：** 未实现。`src/search/` 无任何 filter 维度/组合/AND/OR/清除逻辑（grep `filter` 仅注释提及「filter matrix … intentionally absent」，`mod.rs:11-12`）。
- **M02.4 空查询展示/收藏区/最近：** 未实现。空查询（`query.tokens().is_empty()`）在 `mod.rs:41-45` 显式返回空 Vec（「M02-A is scoped to non-empty queries」「Empty-query display strategies are M02.4」，`mod.rs:42-43`）。
- **M02.5 10k 基准/性能 harness/查询取消与世代：** 未实现。`Cargo.toml` 无 `criterion`/bench；无 cancellation/generation 代码。以上均不在 M02-A diff 中（`--name-only` 核验）。
- **结论：** 三项非目标均被**显式排除**（注释声明 + 无对应代码），既未静默加入也未遗漏混入。

## 强制对抗性清单逐项结论（decisive verdicts）

### 对抗项 1：评分位域单调性与溢出 — **PASS（经推演，无出血风险）**

`total_score = tier_bits(56..59) | points_bits(40..55) | bonus_bits(20..37)`（`scoring.rs:26-31, 135-140, 176-187`），各段不重叠且 `tier_bits` 左移 56 位为最高位 → 按 `total_score` DESC 排序等价于按（quality_index, points, bonus）三元组 DESC。**tier 出血：** `points_sum` 在 `scoring.rs:177` 用 `.min(POINTS_MAX)`（65535）饱和，`POINTS_MAX = 2^16-1`，其左移 40 位后最高 bit 55，任何取值都无法进位到 bit 56 的 tier 段——**无论 token 数多寡均饱和，无下行风险**（token 数在查询条目上无硬上限，但语义上多 token AND 每增一个 token 均 `.min` 钳制，最高位不会进位）。**bonus 出血：** `manual_weight_bias ∈ [0,200_000]`（`-100..100` → 0..200k，`144-148`）+ 置顶 10k ≤ **210_000 < 2^18-1 = 262_143** = BONUS_MAX（`140`），`min(BONUS_MAX)` 后左移 20 位最高 bit 37，无法进位到 bit 40 的 points 段。**correctly bounded。** `tier_bits` 用 `quality_index`（`76-83`）高位为佳、`best_tier.map_or(Tier::EditDistance)` 仅理论不可达兜底（`best_hit_for_all_tokens` 有 `Some` 才有 hits）。`candidate_rank`（`203-222`）以 tier<<8 | 反 points 的低者为佳，`min_by_key` 确定性选优，与最终排序方向一致。数值验证：`2^16-1=65535`、`2^18-1=262143`、210000 ≤ 262143 ✓。

### 对抗项 2：结果 tier = 全部命中 token 中最弱 tier（`scoring.rs:167-173` `best_tier = max`）— **设计正确（记录设计说明，非缺陷）**

语义：多 token AND 中「每个 token 均须命中」→ 结果实际相关度由最弱一环决定，属可辩护设计，且与需求「不同 token 可命中不同字段」兼容。已确认**不违反**「名称 exact 必须压倒其他」的保证：`best_tier` 只影响该结果落在哪个 tier 段，per-token 的 `points_sum` 依然计入；跨结果比较中，「名称 exact + 路径 token」的结果落在 `Tier::Path`（较弱 token 决定），与「名称 exact + 名称 contains」的结果比较时后者（Path 上游）优先——这是多 token 场景下的合理折衷：名字再准，another token 只命中路径，整体相关性本就该低于两 token 都强命中的结果。**单一缺陷风险不成立**：任一 token 若只命中 Name 而另一 token 命中 Path，结果确实不进 Name tier，但该结果对单 token 查询（只查那个 Name token）会以 Name tier 出现——`best_tier` 只在每 token 的 `FieldCoin` 集合上取 max，不影响单 token 结果。仅为已记录的设计说明，建议在模块文档补一句「多 token 结果以命中 token 中最弱 tier 定级」以利可审计性（已由 `167-171` 注释说明，无需修复）。

### 对抗项 3：pinyin 正确性与多音字确定性 — **PASS（读 crate 源码核实）**

- **无全局可变/随机/哈希序态：** `pinyin 0.11.0` 的 `get_block_and_index`（`src/lib.rs:57-65`）为静态 `CHAR_BLOCKS.iter()` 顺序线性查找，命中后索引 `PINYIN_DATA` 的**首个字节索引**——即每个汉字在 pinyin-data 数据表中取**第一读音**，确定且无偏。crate 未启用 `heteronym` 特性（`Cargo.toml:18` 仅 `plain`），不加载多音字表、无随机性。`first_letter`（`pinyin.rs:67-68`）取首字母。
- **派生键正向正确性：** `keys.rs:78-114` `pinyin_full`/`pinyin_initial` 逐字 `syllable.plain()`/`first_letter()` 拼接、（`83-90` 对多 key-char 的 origin 逐字节追 index，`origins` 单调非降）；测试 `280-286`（"中文字"→"zhongwenzi" origins `[0,0,0,0,0,1,1,1,2,2]`）、`266-279`。
- **仅中文源可派生：** `is_name_or_alias && enabled && settings.search_pinyin && has_han`（`keys.rs:163-174`），非 Han 文本不建拼音键；`contains_han`（`67-71`）以 `to_pinyin().is_some()` 判定。
- **派生键绝不落盘/不写文件：** `src/search/` grep 确认无 `fs::write/File::/persist/save`；`keys.rs:21` 模块注释声明「computed per search and are never persisted」。每次 `search()` 内存派生，不生成索引文件，TXT 无数据外泄。
- **轻微已知限制（诚实记录，非缺陷）：** `fold_char` 对 `ß`/`İ` 保留原字（`keys.rs:12-15, 247-252` 文档化行为），CJK 首字母查询表（`不实际存在`）——`pinyin_initial` 对 `zh`/`ch`/`sh` 等卷舌声母只取首字母 `z`/`c`/`s`（`first_letter` 单字符），与国内常用「首字母=zh→z」习惯一致，无问题。
- **多音字缺口（设计观察，非本 slice 缺陷）：** 因取首读音，例如「银行」→ `yinhang`（hang 而非 xing）是 crate 数据的第一读音，属已知产品语义（`heteronym` 特性在 0.0.1 不启用），符合「确定性评分」要求；用户可改查全拼/其他 token，且后续可在 M02-B 引入多音字表而不破坏现有 API。**记录为待人工确认项**（见下）。

### 对抗项 4：编辑距离语义与复杂度 — **PASS（语义正确；复杂度已量化，M02-B 待办）**

- 语义：`edit_distance_match`（`matching.rs:175-187`）在 `max==0` 时返回 false（由 exact/contains 覆盖，正确不重复）；否则先整键、再对每个 whitespace 分隔 word 做 char-based Levenshtein ≤ max（`181-186`）。满足「within max of any word or the whole folded key」。`max` 上限 2 由 `AppSettings::MAX_EDIT_DISTANCE` 与 `validate`（`settings.rs:12,75-77`）封闭。
- 命中语义为「布尔 + Tier::EditDistance + 无高亮 range」（`matching.rs:284-288`，`highlight.rs:67-68`），不携带具体 span——可接受（编辑距离本身无可定位子串）。
- **复杂度：** `levenshtein(a,b)` 为 O(|a|·|b|) DP（`150-168`，两行数组滚动，内存 O(min len)），对极长路径（MAX_PATH_LEN 32_767 chars = 域约束 `folder.rs:10`）整键 DP = 32k×32k ≈ 10^9 单元；**但**仅在 fuzzy 开 + 该 token 未被更优策略命中的 entry 上运行全泄露。**M02-B 10k 基准前必须量化**：若一次查询 token 集使大量条目落入 edit-distance 全键路径，单查询可达 O(10k·|t|·|path|)。当前 0.0.1 无 10k 数据规模，单次 <50ms 目标未声明于本 slice。已挂为 cross-cutting 性能观察 + M02-B 待办（非阻断性，不阻塞 M02-A 里程碑批准；基准属 M02-B）。
- 另：查询 token 数无硬上限，极长 token（如粘贴整段文字）会放大 `levenshtein` 成本——0.0.1 在 M02-B 前应界。**记录为 M02-B 待办。**

### 对抗项 5：Range→原始串映射非法切片 — **PASS（多重边界防护，无 panic 路径）**

- `map_key_range`（`matching.rs:80-87`）先验 `key_start < key_end` 且 `key_start < origins.len()`，返回 `(first_origin, last_origin+1)`（`last_origin+1` 至多为源串 char 数，因 origins 为源索引）；pinyin 多键字符 origin 单调非降（`keys.rs:44-47` 文档+`281-286` 测试），末字符 origin+1 不会越界。
- `highlight.rs:76-77` 再以 `source.chars().count()` 复核 `end <= char_count`，越界/非法范围一律丢弃，绝不 panic。混中文+emoji 测试 `tests.rs:514-526` 验证含 `🗂️`（含 ZWJ 序列）的字符串按 char 数正确切片；`compute_highlights` 对 `end<=char_count` 之外的 range `continue`（`77`），无 panic 面。
- 空字符串/仅 emoji 等输入：`find_contiguous` 对空 needle/空 haystack 返回 None（`matching.rs:70-75`）；`subsequence` 对空 pattern 返 None（`116`）；`derive_keys` 对空字段建空键，自然不命中，无索引越界。Rust `String` 不能含孤立代理项（UTF-8 保证），`chars()` 天然在字符边界。**无 panic 路径。**

### 对抗项 6：设置开关语义 — **PASS（每开关均实证门控其字段/策略；`substring` 与 fuzzy 语义与需求一致）**

| 开关 | 门控实现 | 门控内容 | 测试 |
|---|---|---|---|
| `fuzzy_matching` | `matching.rs:241,269,277` | 仅门控 `Subsequence` 与 `EditDistance`；**不**门控 exact/prefix/**contains(substring)** | `tests.rs:300-323`（fuzzy off: subsequence 与 edit-distance 均无结果，`usb` prefix 仍命中） |
| `search_paths` | `keys.rs:78-83`（field enabled=false 时不参与） | 路径字段仅 | `tests.rs:560-585` |
| `search_categories` | `keys.rs:84-91` | 分类仅 | `tests.rs:618-645` |
| `search_tags` | `keys.rs:92-104` | 标签仅 | `tests.rs:618-645` |
| `search_notes` | `keys.rs:105-110`；默认 `settings.rs:52` false | 仅启用后 note 字段参与 | `tests.rs:588-615`（默认 off 验证） |
| `search_pinyin` | `keys.rs:163-171`（键不存在即无该策略） | pinyin full/initial | `tests.rs:648-695`（off → `zwwd` 无结果） |
| `search_english_initials` | `keys.rs:172-173` | english initial 键 | `tests.rs:648-695`（fuzzy off 隔离验证 off → 无结果） |
| `max_edit_distance` | 上层 validate 上限 2（`settings.rs:75-77`）；`matching.rs:277-283` | ≤2 才跑编辑距离 | `tests.rs:241-269`（1 vs 2 边界） |

**需求语义对齐说明：** 需求把 `substring` 与 fuzzy 分开列举（M02.1「exact、prefix、substring、…、ordered-subsequence fuzzy 与受控编辑距离容错」）。本设计 `fuzzy_matching` 只门控 subsequence 与 edit-distance 两类 true-fuzzy 策略，**substring 不受 fuzzy 影响**——与需求语义一致（substring 是直接文本策略，非 fuzzy）。**未被错误门控。** pinyin/english-initial 与 string 策略同属「直接派生键匹配」，由各自开关独立门控，不受 `fuzzy_matching` 影响（注释 `290-293` 明示）——亦与需求一致。

### 对抗项 7：确定性 — **PASS**

`src/search/` 零 `HashMap/HashSet`（grep 确认），键迭代通过 `DerivedKeys::enabled_fields`（`keys.rs:217-231`）以固定顺序（name → aliases 序 → path → category → tags 序 → note）链式迭代；`enabled_fields` 内层排序仅对同一 token 的候选 `min_by_key` 用 `candidate_rank`（确定性），结果排序 `mod.rs:60-73` 用总分数 DESC + 纯函数 `tiebreak`，无哈希/随机/时间依赖。重复查询字节级一致测试 `tests.rs:733-748`。**PASS。**

### 对抗项 8：有界性/DoS — **PARTIAL（0.0.1 可接受；M02-B 前必须边界化）**

- token 数：无硬上限（`QueryParser` 按空白切分任意长度）；策略尝试次数：每 entry 每 token ≤ 7 策略 × 启用字段数（≤ 名称+别名×20+路径+分类+标签×N+备注），有界常量。
- 成本放大点：`levenshtein`（`matching.rs:150-168`，O(|pattern|·|key|)，key 含整路径最长 32_767 chars `folder.rs:10`）在 fuzzy on 时对每个漏过更优策略的 entry 在「整键 + 每 word」上运行；极长 token（粘贴长文本）会线性放大。0.0.1 数据规模小、可接受；**10k 条目场景（M02-B）前必须加策略护栏**：如仅对 ≤某长度 token/键跑编辑距离、或按 word 分而治之（现有按 word 已缓解多词命中，但整键分支仍 O(|p|·|k|)）。已在 cross-cutting 性能节记录为 M02-B 必做项。

### 对抗项 9：隐私/性能/Windows — **PASS**

`grep` 于 `src/search/`：无 `std::fs`/`std::net`/`std::env`/`std::time`/`std::process`/`std::thread`（零命中），无网络/遥测/磁盘/时间引用；搜索纯函数（`mod.rs:1-8` 模块文档声明）输入仅 `SearchEntry` 与 `AppSettings`，输出纯函数结果,不触碰真实路径判断（可访问性检查不在本层，属 M02-B 筛选）。pinyin 依赖离线（crate 数据内嵌于 bin，无网络调用；crate 无网络依赖）。**无 fs/network/time，符合约束。**

### 对抗项 10：测试覆盖对照报告声称（58 search tests）— **PASS（实际 57，±1 口径已核验；关键项全在）**

- 实际静态计数：`tests.rs` 36 + `query.rs` 5 + `keys.rs` 7 + `matching.rs` 5 + `scoring.rs` 4 = **57**（CI 实测 lib 126 = 既有 69 + 新 57）。implementation 报告称「58」——为**口径微差**（可能把 `mod.rs` 的 doc 哨兵或计数时多计 1；`mod.rs`/`highlight.rs`/`search_entry.rs` 均 0 测试），不构成数据不实，CI `test result: ok. 126 passed; 0 failed` 为权威。
- 需求映射的每项必测在（均逐行核验）：**acceptance usb drv** `tests.rs:702-726`；**pinyin 中英混合** `tests.rs:271-297`、`keys.rs:266-294`；**编辑距离 1 vs 2** `tests.rs:241-269`；**tier 顺序表** `scoring.rs:300-319,243-298` + 端到端 `tests.rs:399-445`；**manual_weight 不压过 name exact** `tests.rs:476-508`（第 483-485 行残留未使用变量 `unweighted/weighted` 与占位 assert——见 F001）；**pinned 不跳 tier** `tests.rs:448-474`；**highlight UTF-8/中英+emoji** `tests.rs:514-526`；**开关** `tests.rs:559-695`；**稳定性** `tests.rs:733-748`；**空查询** `query.rs:136-140`、`tests.rs:89-93`；**max_results 截断** `tests.rs:750-761`。**无缺测。**

### 对抗项 11：无 scope creep — **PASS**

见「变更范围核验」节：diff 精确 11 文件，`domain`/`storage` 零变更。

### 对抗项 12：质量 — **PASS（1 处残留需清理）**

- `src/search/` **零 `#[allow]`**（grep 确认）。
- 评分权重非魔法数字：tier/points/bonus 常量集中声明于 `scoring.rs:44-140`，公共 `weights` 模块（`118-131`）导出；tier/点位文档级注释（`1-37`）与实际一致。
- 公网 API 文档：`mod.rs`（模块）、`search()`（`35-73`）、`SearchEntry/SearchScore/RankedResult`（`search_entry.rs:51-67,123-148`）、`Query/QueryParser/Token/TokenKind`（`query.rs:5-52`）、`HighlightRange/HighlightOptions`（`highlight.rs:17-38`）、`weights` 各常量均带 `///` 文档。`pinyin` crate 内部 API 未泄漏（仅内部使用，未 re-export）。
- 错误处理：纯搜索无 I/O，设计上无错误类型可返回（`SearchScore`/`Vec<RankedResult>` 无失败面），`search()` 对空查询空返回而非 panic。符合「核心逻辑无错误模式」的合理设计。
- **F002（残留）：** `tests.rs:477-485` `manual_weight_does_not_overpower_name_exact` 函数体前段构造 `unweighted`/`weighted` 两个 SearchEntry（`483-484`）但 `let _ = (unweighted, weighted);`（`485`）后完全未使用，随后委托 `alias_boosted_manual_weight`（`492-508`）用不同 fixture 实际断言「pinned+max-weight 别名不敌 name-exact」。残段死代码：手动权重边界语义（未加权重 vs 加权重同 name-exact 时按权重排序）未被该测试直接验证，而 `tiebreak`（`scoring.rs:230`）第一判据 manual_weight 的「同分排序」只有在同 name-exact 情况下才触发——现测试两条目（`boosted` 别名 vs `name_exact`）不同 tier，**永不走到 tiebreak 的权重分支**。即：**manual_weight 作为同分 tie-break 的「同分排序」路径目前无测试覆盖**，F001（方向反转）因此未被拦截。见 F002（Medium）。

### 附加对抗项 3c：fuzzy off 对 pinyin/英文首字母的影响 — **PASS**

`fuzzy_matching=false` 只关 subsequence/edit-distance；pinyin full/initial 与 english-initial 经 `matching.rs:294-322` 独立于 fuzzy 运行（只要各自 toggle 开）。`tests.rs:674-695` 正是利用「fuzzy off 隔离英文首字母键」验证该独立语义。与需求一致（拼音/首字母是独立策略，非 fuzzy 子集）。

## 按严重级排序的 Findings

| ID | 严重级 | 文件:行 | 问题 | 影响 | 证据/复现 | 建议 |
|---|---|---|---|---|---|---|
| **F001** | **High** | `src/search/scoring.rs:230-236`（`tiebreak`，被 `mod.rs:60-66` 排序使用） | **`tiebreak` 的 `open_count` 项比较方向反了（升序），与自身文档（`227-228`：`… open_count descending`）及产品意图（README「最近使用项」、REQ-SEARCH-011「最近」= 更常用者优先）冲突；同一函数内 `manual_weight` 项同样写了升序（与文档「descending」冲突）。** 具体：`left.open_count.cmp(&right.open_count)` 与 `left.manual_weight.cmp(&right.manual_weight)` 是「升序」写法（left 更大 → `Greater` → left 排后），而同函数紧邻的 `pinned` 项用的是正确的降序惯用法 `right.pinned.cmp(&left.pinned)`（`233`）、id 项用 `left.id.cmp(&right.id)` 与文档「id ascending」一致（`235`）——同一函数内两种惯用法并存，且方向相反的两项都存在。 | **`open_count` 分支：用户可见的排序反转（可达，High 理由）**。`open_count` 完全不参与 total_score 位域（`scoring.rs:26-31`：tier/points/bonus 三字段均不含 open_count），因此**任何**「同 (tier, points, bonus)」的条目（例如两条都 name-exact 命中、同 manual_weight、同 pinned 的目录，或同 name-prefix 命中、同分的目录）在现实中极常见地 total_score 相等，tiebreak 第 3 分支必然触发，结果是：**越常用（open_count 高）的目录排得越低**——与「最近使用优先」期望完全相反。逐步验证（Rust `sort_by` 契约：comparator 返回 `Greater` = left 排后）：A.open_count=100 与 B.open_count=0 平分时，`left=A` 返回 `Greater` → A 排 B 后 → **B（从没用过）排在 A（用过 100 次）之前**。**`manual_weight` 分支：潜伏（latent）**——因 total=disjoint 位域拼接，total 相等 ⇒ `bonus_bits` 相等 ⇒ bias 相等 ⇒ weight 相等（bias 单调 `scoring.rs:322-330`），故经 `search()` 可达的平分必然同 weight，该分支实际恒返回 `Equal`，其反转暂不可观察；但它是函数文档契约的一部分，且 `tiebreak` 若被未来调用方直接复用（或补单元测试按文档断言）即会出错，应一并修正。根因：`tiebreak` **无任何单元测试**（`scoring.rs` 测试无 tiebreak 用例；`tests.rs` 全部 fixture `open_count=0` `manual_weight=0`，`tests.rs:35-36`），故方向错误从未被捕获。 | 两条 name-exact 完全同名、`manual_weight=0`、`pinned=false`、`open_count` 不同（如 0 与 100）的目录；`search()` 输出顺序。`tiebreak`（`scoring.rs:230-236`）第 1、2 分支（weight、pinned）相等后落第 3 分支 `left.open_count.cmp(&right.open_count)`：open_count 较大者返回 `Greater` → 排后 → **打开次数多的在后的错误方向**。 | 交换两项操作数，与相邻 pinned 项保持一致降序惯用法：`right.open_count.cmp(&left.open_count)` 与 `right.manual_weight.cmp(&left.manual_weight)`。**必须**补充 `tiebreak` 表驱动单元测试（open_count 高者先 / pinned true 先 / manual_weight 高者先 / id 小者先，逐项 + 组合），锁定各分支方向——现有测试因 `open_count=0`、`manual_weight=0` 的 fixture 无法触及任何分支。此为行为缺陷，非纯文档/测试问题。 |
| **F002** | **Medium** | `src/search/tests.rs:477-508` | 承接 F001：`manual_weight_does_not_overpower_name_exact` 构造 `unweighted`/`weighted`（`483-484`）后 `let _ = (…)`（`485`）丢弃，实际断言委托 `alias_boosted_manual_weight`（`492-508`）。后者两 fixture 处于**不同 tier**（pinned 别名 vs name-exact），**永不触达 tiebreak**；且 `tests.rs` 全部 fixture `open_count=0`、`manual_weight=0`（`35-36`）意味着 `tiebreak` 的 open_count/manual_weight/id 各分支在**任何**现有端到端测试中都不可能触发——F001 正是因此溜过。 | 测试宣称覆盖「manual_weight 为有限 tie-break」，实际只验证「不跨 tier」；tiebreak「同分排序」方向（尤其 open_count 的反转）无任何约束。与 F001 强耦合，是 F001 未被拦截的直接原因。 | `git show 18c48f5:src/search/tests.rs:477-508`；`tiebreak` 仅在 total_score 相等时调用（`mod.rs:64`）；`scoring.rs` 测试模块亦无 tiebreak 用例。 | 清理 `477-485` 死代码；与 F001 一并补「同名同分」表驱动用例（设置 `open_count`/`manual_weight`/`pinned` 差异，直接对 `tiebreak` 断言返回方向，覆盖全部分支）。 |
| **F003** | **Low（观察/记录）** | `src/search/matching.rs:150-187` | 编辑距离的整键 Levenshtein 为 O(|token|·|key|)，key 最长可到整路径 32_767 chars（`folder.rs:10`），在 fuzzy on 时对每漏过 entry 全键运行；查询 token 数无上限，极长 token 放大成本。 | 0.0.1 数据规模可接受；10,000 条目（M02-B）场景下若不加护栏，单查询最坏可达 O(10k·|t|·|path|)，有 >50ms 风险。非本 slice 缺陷，属 M02-B 前置约束。 | `matching.rs:181-187` 整键分支无条件执行；无一长度阈值。 | M02-B 基准前实现：仅对 token/key 长度 ≤ 阈值（如 64 chars）跑编辑距离；保留「整键或 word」语义但对整键分支加长度上限；在 10k release 基准中单独报告编辑距离路径成本（任务清单已要求）。 |
| **F004** | **Low（文档/可读性）** | `mod.rs:28-34`（设计语义已注释于 `scoring.rs:167-173`） | result tier = 命中 token 中最弱 tier 的设计在评分模块注释（`scoring.rs:167-171`）已写清，但**公共 API 文档**（`mod.rs:28-34`）只写「tier-based and fully deterministic」未言明「多 token 结果以最弱 token 定级」。 | 对调用方（M03 presenter/RankedResult 消费者）不够透明，可能误以为多 token 结果取最强 token 定级。非行为缺陷。 | `mod.rs:28-34` 与 `scoring.rs:167-173` 注释差异。 | 在 `mod.rs` 的 `search()` 文档补一句多 token 定级规则（最弱 token）。 |
| **F005** | **Low（观察）** | `src/search/keys.rs:67-71` / pinyin crate | pinyin 取第一读音（无 `heteronym`），多音字（如「乐」）确定性取首读；个别中文首字母（如 `zh`→`z`）为单个首字母。均属 0.0.1 已记录的确定性语义。 | 「乐」类多音字路径/名称若以非常用读音拼音搜索可能不命中；符合「确定性评分」要求，非缺陷。 | pinyin crate `get_block_and_index`（`src/lib.rs:57-65`）+ `pinyin-data` 首项。 | 记录为已知限制/手工验收项（见下），M02-B 可选引入 `heteronym`。 |

**汇总：** **1 High（F001：tiebreak 的 `open_count` 分支方向反转——可达的用户可见排序缺陷；`manual_weight` 分支同向错误，潜伏）**、1 Medium（F002 测试缺口，与 F001 强耦合，是 F001 未被拦截的原因）、3 Low（F003 M02-B 编辑距离护栏、F004 公共文档补一句多 token 定级、F005 多音字首读音观察）。

## Cross-cutting 检查

- **正确性：** 匹配、得分、排序、高亮全链路经 57 项测试覆盖；位域推演无出血；tier 结构性保证权重顺序；跨字段多 token 与别名语义符合需求。**例外：** `tiebreak`（`scoring.rs:230-236`）的 `manual_weight`/`open_count` 两项比较方向与自身文档及产品意图相反（F001 High），该函数无任何单元测试，故 57 项全绿未覆盖此缺陷。
- **错误处理：** 纯搜索无 I/O，无错误类型是正确设计（`search()` 空查询返回空列表而非 panic）；无 unwrap/expect/panic（grep 确认非测试代码唯一 `.unwrap()` 在 `matching.rs:406` 的测试用例内），`manual_weight_bias` 的 `try_from(...).unwrap_or(u64::MAX)`（`scoring.rs:147`）为不可达兜底（`weight+100∈[0,200]`）。
- **数据安全：** 不从磁盘读/不改写任何用户数据；派生键每查询内存生成、绝不持久化（多次 grep 确认 + `keys.rs` 模块注释 + `search_entry.rs:51-52` 注明 caller 负责任何数据来源）。无真实目录删除面（`src/search/` 无 `remove_dir/remove_folder/remove_file`）。
- **隐私：** 无路径/搜索词/配置内容记录；不触碰真实路径状态；无网络、无遥测、无环境变量；日志无新出口。
- **安全性：** 新依赖 pinyin 0.11.0 单包无依赖、MIT、仅离线内嵌数据表，carry deny 白名单已含 MIT；无攻击面增量（搜索纯内存）。编辑距离为已验证的 len 级 DP，无指数回溯。
- **Windows 行为：** 无平台 API 调用；路径仅为纯字符串比较（不解析/不归一化盘符——正确边界，路径等价判断属后续层）；MSVC CI 全绿。
- **测试覆盖：** 57 项全新搜索测试 + 既有 69 项全绿（CI 126 通过）；无 `#[ignore]`/`#[should_panic]` 占位（grep 确认）；覆盖需求映射全部必测项。**缺口：** `tiebreak` 无单元测试 + 端到端 fixture 全部 `open_count=0`/`manual_weight=0`（`tests.rs:35-36`），导致 F001 方向反转完全未被任何测试触及。
- **性能：** 每 entry 每 token 策略数有界；Levenshtein 为唯一超线性成本点（F003）；10k 基准明确移出本 slice。M02-B 前须护栏化编辑距离、量化 pinyin 键派生开销（每查询对含 Han 的 name/alias 派生，必然量级同 O(字段长度)）。
- **可访问性：** 无 UI 变更，无新增 a11y 结论。
- **可维护性：** `keys/matching/scoring/highlight/query/search_entry` 分层清晰，pure function 边界明确；注释先行（模块文档写清 tier 次序、位域布局、归一化语义）；测试按「归一化/派生键/策略/评分/高亮/开关/稳定性」分节。`tiebreak` 的 `manual_weight`/`open_count` 方向与注释相反是薄弱点（F001）；F002 死代码待清理。
- **依赖与许可证：** pinyin MIT 已在 deny allow 列表（`deny.toml:21`），cargo-deny 与 cargo-about 均成功；锁文件不可变（`Cargo.toml:18` 精确锁版本）。`Cargo.lock` pinyin 条目 3282-3287 无依赖边。

## 未能自动验证的项

1. **pinyin 多音字语义（含 `zh→z` 首字母习惯）与真实中文路径命中**需真实桌面/用户验收确认是否符合期望（F005 观察；当前为确定性首读音，可解释但用户可能期望另一读音）。
2. **10,000 条基准**（M02-B）：本 slice 无 benchmark；编辑距离路径（F003）与 pinyin 键派生在 10k 数据上的中位数/p95 需 M02-B release CI 或专用 harness 验证，且必须用 release 模式而非 debug。
3. **真实搜索输入框体验**（IME composition、输入法候选、快速连续输入的旧结果覆盖/取消机制）属 M02-B/M03，本 slice 纯核心不含 UI 与取消。
4. **Windows 路径长串 + fuzzy on 的真实耗时**（32k 路径场景）：单元测试不覆盖真机 P95，由 F003 挂 M02-B。
5. 无 GUI；托盘、快捷键、多显示器/DPI 等桌面验收仍不在本搜索核心范围。

## 最终结论

`CHANGES_REQUESTED`（M02-A 纯搜索核心）

- **范围与独立性：** 11 文件精确切分，domain/storage 零回改；`src/search/` 全部历史 = 单个 M02-A 提交，reviewer 完全独立。
- **CI：** head `18c48f5` 的 MSVC 门禁 run `35816387166` 全绿（fmt / clippy `-D warnings` / **127 项测试通过 0 失败** / release build / EXE 与版本核对 / cargo-deny / cargo-about / portable artifact 打包上传）。pinyin（MIT）在 deny 白名单内，license audit 与 inventory 步骤成功，无需配置变更。CI 全绿如实记录，但不覆盖 tiebreak 方向缺陷（该函数无单元测试）。
- **需求映射：** M02.1/M02.2 全部验收点 PASS（含 M02.2 跨 slice 项的范围界定；其中「stable tie-breaker」「manual_weight 有限 tie-break」两项按**文档声明意图**应满足，但实现方向与文档相反——见 F001，故这两项在实际行为上未达标）；M02.3/4/5 被显式排除、未静默加入。
- **对抗性清单：** 位域无出血（PASS）、result=最弱 token 定级属可辩护设计、pinyin 确定性经 crate 源码核实（PASS）、编辑距离语义正确 + 复杂度已量化（F003 挂 M02-B）、range 映射无 panic 路径（PASS）、开关语义与需求一致含 substring 不误门控（PASS）、确定性零哈希依赖（PASS，但 tiebreak 方向错误是确定性的错方向，非不确定性）、有界性 PARTIAL（F003）、隐私/Windows 无面（PASS）、测试覆盖对照 15 项必测全在（PASS，但 tiebreak 无直接测试为唯一缺口）、无 scope creep（PASS）、零 `#[allow]`（PASS）。
- **Findings：** **1 High（F001：`tiebreak` 的 `open_count` 比较方向升序与自身文档及产品意图相反，致同分时越常用者排得越低——可达；`manual_weight` 项同样写反向，但经 `search()` 不可达（total 平分强制 weight 相等），属潜伏）+ 1 Medium（F002：与 F001 强耦合的测试缺口 + 死代码） + 3 Low（F003 M02-B 编辑距离护栏、F004 公共文档补一句多 token 定级、F005 多音字首读音观察）。
- **处置要求（阻断本里程碑批准）：**
  1. F001（High）**必须修复**：交换 `open_count`/`manual_weight` 项操作数，保持降序惯用法与相邻 `pinned` 项一致（`right.open_count.cmp(&left.open_count)`、`right.manual_weight.cmp(&left.manual_weight)`），并**补充 `tiebreak` 表驱动单元测试**逐项锁定方向（open_count 高者先 / pinned true 先 / manual_weight 高者先 / id 小者先 / 组合）。
  2. F002（Medium）随 F001 一并处理（清理 `tests.rs:477-485` 死代码 + 同名同分用例）。
  3. 修复后须重新推送并通过 MSVC CI，由本 reviewer（或另一名独立 reviewer）复审后再裁定 `APPROVED_FOR_MILESTONE`。
  - 按 CLAUDE.md §4.5，全部 blocking finding（含本 High）关闭前**不批准里程碑**；本结论亦不构成发布批准，`APPROVED_FOR_RELEASE` 需后续 Release Candidate 独立发布评审、真实桌面手工验收与对应响应闭环。
