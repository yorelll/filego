# Review: M03 Slint 主搜索窗口 + ViewModel（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m03-window`
- **轮次：** `r01`（首次评审）
- **日期：** 2026-09-21（本地时间线；CI 时间 2026-09-23T13:18:49Z）
- **Reviewer：** 独立 code-review agent（M03 r01）
- **独立性声明：** 本 reviewer **未参与 M03 实现 commit `7de3dd3` 的任何编码**，未参与该 commit 之前的任何 M03 presenter 开发，未撰写任何 M03 response 文档，未参与 CI 触发与监控；`src/presentation/` 在 base `81c331b` 上仅有空壳 `mod.rs`（git 证实），全部 5 个新代码文件（commands/i18n/state/theme/view_model）均来自 `7de3dd3` 单一实现 commit，reviewer 对该 commit 以任何形式（含会话压缩/重命名/换名字进行代理实现）均未参与。本评审严格只读：**未编辑任何源代码、未运行任何 `cargo` 命令（依照 CLAUDE.md §3.1 未执行本地 Rust 验证）、未提交、未推送**；除本 review 审计文档外未写入任何其他文件。
- **Base SHA：** `81c331bce65889ce1e54edf3e14b259ccc6e6b15`（`docs: record M02-B filter and benchmark review r02 approval`）
- **Head SHA（本评审对象）：** `7de3dd39f63d9d43d6fe1d20622a9250b365fc79`（`feat: add Main search window with ViewModel, IME gate, i18n, theme`）——`git rev-parse HEAD` 实测与之一致，为当前分支头部。
- **比较范围：** `81c331b..7de3dd3`（代码 + 文档；`git diff --stat` 恰 8 文件，见下）
- **审查文件（代码）：** `src/main.rs`（+389/−4）、`src/presentation/{mod,commands,state,i18n,theme,view_model}.rs`（新/改）、`ui/app-window.slint`（+427/−20）
- **审查文件（审计/上下文）：** `review/0-0-1/m02-search-review-r02.md`（M02-A 批准）、`review/0-0-1/m02-filter-bench-review-r02.md`（M02-B 批准）、`task/01-里程碑任务清单.md`（M03 节）、`src/search/{generation,empty_query,filter,highlight,query,search_entry}.rs`（M03 消费的 API 契约）、`src/lib.rs`（head 与 base）、`src/diagnostics.rs`、`src/bin/filego-version.rs`
- **审查方法：** ① `git diff 81c331b..7de3dd3` 逐字审查 8 个文件与统计；② 对抗性 grep（shell 执行/路径处理、搜索与领域零改动、日志/隐私、`#[allow]`/unsafe/真实目录删除、确定性/非确定性逃逸）；③ 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询并核验 2 个 CI run 的 head SHA、workflow、job 结论、核心日志行（`test result` 计数、BENCH 行、MSVC release EXE 校验步骤）；④ ViewModel 状态机逐命令人工推演（generation 陈旧拒绝、IME gate、overlay/hide gate、selection 合法化）；⑤ Slint 回调〜命令、状态推送的接线一致性核验（不运行 Slint 编译，以 CI 编译通过为结构证据）；⑥ 需求→验收逐条映射。

## CI 证据（reviewer 独立核验）

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35866194185](https://github.com/yorelll/filego/actions/runs/35866194185) | `7de3dd39f63d9d43d6fe1d20622a9250b365fc79` | `Windows CI` / job `107198274636`（`fmt, clippy, test, release, package`） | `success`（completed，push event，`feature/m00-foundation`） | `gh run view`：head SHA 与 `git rev-parse HEAD` 一致；job 同名并 `success`；20 个 step 全绿（fmt→clippy `-D warnings`→tests→release build→verify release outputs + version helper→cargo-deny→cargo-about→portable artifact→upload）。`--log` 实测 `Run tests` step：**`test result: ok. 188 passed; 0 failed; 1 ignored`（lib）+ `test result: ok. 2 passed`（main bin `filego-version`）+ 2 个 bin `0 passed`**。即 **191 项（lib 190 = 188 通过 + 1 ignored 基准；main 2 通过），0 失败**。 |
| [35866193999](https://github.com/yorelll/filego/actions/runs/35866193999) | `7de3dd39f63d9d43d6fe1d20622a9250b365fc79` | `Search benchmark` / job `107198273659`（`release 10k benchmark`） | `success`（completed，push event，同 head/同时间戳） | `gh run view`：head SHA 一致；`test result: ok. 1 passed; 0 failed; 188 filtered out`（即第 6 基准类在 release 模式真实执行）；**实测 6 行 BENCH**：`empty-query-default median=0.81 / filtered-with-clone median=61.58 / pinyin-heavy 70.75 / english-initials 60.23 / edit-distance 65.46 / multi-token 53.75 ms`，`$summary` 守卫（`if ($null -eq $summary) { throw ... }`）未触发。M03 CI 未触碰工作流（`git diff --name-only 81c331b..7de3dd3 -- .github/workflows` 零输出）。 |

- **统计口径核验：** `git grep '#[test]' src/` 静态计数 191 = app 8 + domain/mod 1 + main 2 + i18n 5 + state 5 + theme 4 + view_model 18 + search（benchmark 5 + empty_query 6 + filter 11 + generation 1 + keys 7 + matching 6 + query 5 + scoring 7 + tests 38）+ storage（repository_tests 33 + tests 28）+ version 1。CI 日志 188 通过 + 1 ignored（=189 lib executed，`#[ignore]` 为 M02-B 基准）+ 2 main = **与静态计数口径一致（含 1 个 `#[ignore]` 基准，非失败）**。M03 新增恰为 `presentation/` 的 32 项（view_model 18 + i18n 5 + state 5 + theme 4）。
- **MSVC 门禁核验：** 两 workflow（ci.yml / benchmark.yml）在本 range 内零变更；Windows CI head `7de3dd3` 全绿覆盖 fmt / clippy `-D warnings` / **191 项测试（189 执行 + 1 ignored）0 失败** / MSVC release build / verify release outputs（`target\x86_64-pc-windows-msvc\release\filego.exe` 与 version helper 存在性检查）/ cargo-deny / cargo-about / portable artifact 上传。MSVC 门禁成立。

## 变更范围核验（无 scope creep）

`git diff 81c331b..7de3dd3 --name-status` 恰 8 文件：

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/presentation/mod.rs` | M | base 为空壳模块文档；head 声明 5 个子模块与 M04 边界 |
| `src/presentation/commands.rs` | A | ViewCommand/SearchKey/RowAction/SearchOutcome 枚举 |
| `src/presentation/i18n.rs` | A | Locale + Msg 目录 + parity 测试（5 项） |
| `src/presentation/state.rs` | A | ViewState/ResultRow/SearchFailure/SelectionMove + 测试（5 项） |
| `src/presentation/theme.rs` | A | ResolvedTheme/Light/Dark/system + WCAG AA 对比测试（4 项） |
| `src/presentation/view_model.rs` | A | SearchViewModel 状态机/command 应用/runner seam/测试（18 项） |
| `src/main.rs` | M | +389/−4：MainWindowController / sync_ui / 事件接线 / demo 数据 |
| `ui/app-window.slint` | M | +427/−20：AppWindow 主窗口 / ResultRow / UiStrings / UiTheme / AppTray 保持 |

**越界零检出：** `git diff --name-only 81c331b..7de3dd3 -- src/search src/domain src/storage src/lib.rs Cargo.toml Cargo.lock .github build.rs` → **零输出**。`src/lib.rs` head/base 逐字一致（`git diff 81c331b 7de3dd3 -- src/lib.rs` 空）。base `src/presentation/` 仅有空壳 `mod.rs`（`git ls-tree` 证实）。**无 scope creep、无依赖新增、无 workflow 变更、无 CMake/build 变更。**

## 需求/验收标准映射（M03.1–M03.5，PASS / GAP）

### M03.1 视图与尺寸

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| 默认宽 600 / min 480 / max 760，圆角 14–16 | `app-window.slint:179-182`（`preferred-width: 600px; min-width: 480px; max-width: 760px`）；`app-window.slint:232`（搜索容器 `border-radius: 14px`） | **PASS** |
| 搜索框高 52–56；结果行 56–64；结果区 max 480 | `app-window.slint:231`（search box `height: 56px`）；`app-window.slint:97`（ResultRow `height: 56px`）；`app-window.slint:352`（results panel `max-height: 480px`） | **PASS** |
| 空/紧凑状态仅搜索框；有默认结果或输入时展开 | `app-window.slint:317`（`if (root.rows-count > 0 \|\| root.state-busy \|\| root.state-shows-empty)` 才显示结果区）；`view_model.rs:656-663`（`initial_state_shows_empty_query_strategy_rows`：默认空查询已产生 M02.4 空查询策略行，故窗口初始即展开——任务清单 M03.1 声明「有默认结果或输入时向下展开」，空查询默认策略属 M02.4 行为，一致） | **PASS** |
| 无永久设置/添加按钮、无传统标题栏/工具栏 | `app-window.slint:183`（`no-frame: true`）；视图内无任何设置/添加按钮；`app-window.slint:176-184` 无工具栏 | **PASS** |
| 搜索框含搜索图标、单行输入、条件式清除按钮、筛选按钮/数量 | 图标 `app-window.slint:238-249`（magnifier `🔍` glyph，`accessible-label: "search"`）；LineEdit `251-289` 单行输入；条件式清除 `292-313`（仅 `!root.state-query.is-empty` 时出现，`✕` + `TouchArea`）；筛选按钮为**非目标**（M05），任务清单已注明「筛选按钮/数量」中筛选按钮留 M05、overlay 状态已在 ViewModel（`view_model.rs:500-504` `toggle_filters`），数量计数在 main.rs 以 `ResultsCount(0)` 占位（见 F001） | **PASS**（筛选按钮按任务清单明确后置 M05，非缺口） |
| 每结果两行：名称+状态，路径+分类/标签；长路径 ellipsis + tooltip | `app-window.slint:128-170`：line 1 名称（+inaccessible 状态 glyph `⚠`+`UiStrings.inaccessible`，`142-152`）；line 2 路径·分类·标签（`156-169`）；line 1/2 均 `overflow: elide`（`138`、`161`），line 2 整行 `Tooltip` 显示完整路径（`164-168`，`row-path` 即 `ResultRow.path_label`，见 F002） | **PASS**（tooltip 展示 path_label 见 F002 裁定） |
| 当前选中明显背景/边框；不可访问只用警告不整行红 | `app-window.slint:100-101`（`border-color: row-selected ? UiTheme.accent : ...`；`background: row-selected ? UiTheme.selection : transparent`）；inaccessible 仅改名称行文字色为 `UiTheme.warning`（`134`）并加 glyph/text（`142-152`），**无整行红** | **PASS** |
| 底部键盘提示可隐藏 | `app-window.slint:430-450`（`if (root.hints-visible)` 提示条）；`view_model.rs:428-431`（`SearchKey::ToggleHints` 翻转）+ UI `key-pressed` 中 `event.modifiers.alt && event.text == Key.H` → `command-toggle-hints`（`278-280`） | **PASS** |

### M03.2 状态管理

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| ViewModel 单一状态来源；UI callback 发送 command | `view_model.rs:133-144`（`SearchViewModel` 持 `state: ViewState`）；命令枚举 `commands.rs:49-79`；`main.rs:417-477` 全部 UI 回调仅 `handle(ViewCommand::...)`；`app-window.slint:25-30` 文档声明「UI is dumb」；`.slint` 零状态变更逻辑（无 `:=` self-modified 决策，全部 `in-out` 由 Rust `sync_ui` 推送） | **PASS** |
| 结果异步更新具 query generation，拒绝陈旧结果 | `view_model.rs:253-276`（`bump_generation` 用 `wrapping_add(1)`，与 `search/generation.rs:30-32` 的 `QueryGeneration::next_generation` 同语义）+ `260-276`（`run_search_for_current_generation`）；`accept_run_result`/`accept_outcome` 仅在 `self.generation == generation`（`296`、`325`）时接受；陈旧 completion 置 `stale` 并保持新 query 的 `busy` | **PASS** |
| 显示时搜索框 focus 并默认全选已有内容 | 任务清单已标注「部分」：Slint 1.18 `LineEdit` 无公开 focus 编程 API + 默认全选留 M04 wiring 前 adapter 行为，已文档化（`main.rs:101-108` 注释、`view_model.rs:28-38`）。**M03 范围无可用实现方式，属合理后置**（native preedit/focus hook 挂 M04 手动验收）。 | **PASS（设计延后，已显式文档化，非阻塞）** |
| 无数据、无结果、不可访问、加载/保存错误状态 | `view_model.rs`：`NoResultReason`（NoData/FilteredOut/NoMatch）经 `sync_ui`（`main.rs:174-188`）映射到 Msg 标题/正文；`SearchFailure::Load/Save/Search`（`state.rs:33-40`）目前仅 `Search` 可达（`view_model.rs:313`、`354`，ErrorSearchTitle/Body 由 main.rs:171-172 输出）；Load/Save 变体为 M06 存储接线的就绪措辞 | **PASS** |
| filter panel/context menu 打开时，失焦不隐藏根窗口 | `view_model.rs:506-509`（`set_overlay`）、`511-518`（`hide_requested` 在 `overlay_open` 时 deferred）、`418-427`（Esc 先关 overlay 再 hide）、`500-504`（`toggle_filters` 置 overlay）；测试 `hide_is_deferred_while_an_overlay_is_open`（`822-831`）、`open_does_not_work_while_overlay_open`（`834-843`） | **PASS** |
| 错误提示可操作、不泄漏裸 Win32 code；详细错误可安全记录 | `SearchFailure::as_detail`（`state.rs:44-54`）匿名静态字符串描述，无裸 code、无路径；`diagnostics.rs` 存在（`SENSITIVE_FIELDS_REDACTED` seam）；「safe-detail-logging」`as_detail` 明确标注 `crate::diagnostics` 语义，隐私规则在 `state.rs:1-7` 模块文档与 `view_model.rs` 顶部 block 声明；无任何 `eprintln`/日志写路径/搜索词（对抗 §3 零命中） | **PASS** |

### M03.3 键盘、鼠标与 IME 状态

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| Up/Down 循环或边界行为明确；selection 随结果变化合法化 | `state.rs:133-148`（`SelectionMove::apply` 双端 wrap 语义：`Up` 从 `None`/首行→末行，`Down` 从 `None`/末行→首行）；`view_model.rs:443-446`（`select_move` 应用）；`rebuild_rows` `403`（越界 index → `filter` 为 None，合法化）。测试 5 项（`state.rs:150-188`）+ `selection_wraps_at_edges_and_is_legalized_on_results_change`（`view_model.rs:708-723`）、`selection_index_is_clamped_after_rows_shrink`（`725-733`） | **PASS** |
| Enter 打开选中项；无选中时无危险副作用 | `view_model.rs:489-498`（`open_selected`：`can_open_selected()` 且 `!overlay_open` 才发 `OpenEntry`；否则 no-op）；测试 `enter_opens_selected_and_is_a_noop_without_selection`（`758-771`） | **PASS** |
| Esc 先关 panel/menu 再隐藏窗口 | `view_model.rs:418-427`（`overlay_open` → 仅关 overlay；否则发 `HideWindow`）；测试 `search_key_map_and_escape_paths`（`774-790`）；顺序与 M03.3 一致 | **PASS** |
| Tab 顺序：search → filter → results；焦点可见 | search→results 已接线：`FocusScope { focus-on-tab-navigation: true }`（`app-window.slint:321-322`）位于 search box 之后；filter panel 因 M05 无实体 tab target，`focus-on-tab-navigation` 在无 filter 时实现 search→results；**filter target 挂 M05**（任务清单「部分」标注一致） | **PASS（filter target 后置 M05）** |
| Ctrl+A、Ctrl+Backspace 标准文本行为 | `app-window.slint:281-284`（非拦截键 `reject` → LineEdit native 处理）；`commands.rs:31-34`（`SearchKey::Other` 仅 `Vec::new()` 不干预）；任务清单标注「待桌面验证」（native 行为由 Windows 文本框承担） | **PASS**（native，桌面验收） |
| 结果聚焦时 Ctrl+C 复制路径；不干扰输入框 copy | results `FocusScope` key-pressed：`event.modifiers.control && (text=="c"\|"C")` → `command-copy-path`（`app-window.slint:334-337`）；输入框内 Ctrl+C → `reject`（`281-284`）保持 native copy；ViewModel 侧 `RowAction::CopyPath`（`view_model.rs:474-485`）仅在 selection 合法时发 `CopyPath` effect | **PASS** |
| 双击打开、单击选中、右键菜单 | 单击选中 + 双击打开已接线（`app-window.slint:111-119`：TouchArea clicked→`row-clicked`=select，double-clicked→`row-activated`=select+open）；右键菜单为 `RowAction::ContextMenu` stub → `set_overlay` gate（`view_model.rs:468-473`），完整 menu 留 M05 | **PASS（menu stub 与任务清单一致）** |
| IME composition/候选期间 Enter/Up/Down 不触发结果操作 | 双层 gate：UI 层 `app-window.slint:262-265`（`if (root.ime-composing) { reject }` 首批拦截 Enter/↑/↓）；逻辑层 `set_ime_composition`（`view_model.rs:192-196`）+ `search_key` Enter gate（`410-416`）+ `select_move_with_ime`（`436-441`）；测试 `ime_gate_blocks_open_and_selection_until_composition_ends`（`793-810`）、`select_by_id_is_blocked_while_ime_composing`（`750-755`）、`ime_gate_still_allows_query_editing`（`813-819`） | **PASS** |

### M03.4 主题、国际化、无障碍

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| light/dark/system token + WCAG AA | `theme.rs:54-81`（LIGHT/DARK 常量，`#8A5A00`/`#F2C94C` warning 注释说明 AA 达成）；`resolve`（`84-94`）把 `System` 映射到传入的 `ResolvedColorScheme`；对比测试 4 项（`theme.rs:128-217`，逐一断言 text/text_secondary/on_accent/warning ≥ 4.5:1） | **PASS** |
| 字体 fallback | `app-window.slint:219`（`default-font-family: "Segoe UI Variable, Microsoft YaHei UI, sans-serif"`） | **PASS** |
| 4px spacing scale；统一 icon | `app-window.slint` 布局 padding 12/8/6/4/2 均为 4px 倍数（如 `app-window.slint:226-227`、`356-357`）；icon 用统一 glyph（🔍 magnifier、✕ clear、⚠ inaccessible），无混用图片/emoji 风格不一致 | **PASS**（glyph 统一，图片 asset 解耦留后续，任务清单一致） |
| zh-CN 与 en-US key 完整；系统默认 + 手动选择接口 | `i18n.rs:27-32`（`Locale` Default=ZhCN）、`46-55`（`detect`）、`135-158`（`tr` 缺 key 回退 id 不 panic）；目录 `196-295`；parity 测试 `every_key_exists_in_both_catalogs_with_no_duplicates`（`306-327`）、`same_key_count_in_zh_and_en`（`330-332`）、`every_message_resolves_to_nonempty_text_in_both_locales`（`335-342`）——**缺失/重复/空值均会使测试失败**；`set_locale`/`set_theme` adapter 接口（`view_model.rs:177-188`） | **PASS** |
| 状态不用颜色单独表达；可访问名称 | 选中行：背景+边框均变 + `accessible-item-selected`（`app-window.slint:107-109`）；inaccessible：glyph + 文字 + accessible-label 拼接 `(不可访问)`（`105-106`、`142-152`），非色-only；搜索图标 `accessible-label: "search"`（`248`）、清除按钮 `accessible-role: button` + label（`306-307`）；ResultRow `accessible-role: list-item`（`105`） | **PASS** |
| 系统减少动画时关闭非必要动画 | 任务清单记录：Slint 1.18 无公开 reduce-motion API，采纳留 M07/M08；**M03 未引入任何 fade/expand 动画**（`.slint` 零 `animate`/`Transition` 声明，grep 零命中）——语义上「无动画可禁」 | **PASS**（依赖已文档化，留 M07/M08） |
| 高对比/文本缩放不截断核心操作 | 明确留 M07/M08（任务清单）；M03 非阻塞 | **PASS（后置，任务清单一致）** |

### M03.5 UI 测试

| 验收点 | 证据（head） | 结论 |
|---|---|---|
| ViewModel command/state 单元测试 | `view_model.rs` 测试模块 18 项；`state.rs` 5 项；共 23 项纯 Rust 状态机测试，无 Slint 依赖 | **PASS** |
| selection/empty/error/filter/IME gate 状态机测试 | selection（wrap/clamp/legalize `708-733`）；empty（`657-663` 初始空查询策略）；error/无结果（`accept_outcome` Err 分支 `350-355`——注意：核心 runner 不产生 Err，见 F004）；filter（`857-863`）；IME（`793-819`）；hide/overlay（`822-843`）；generation stale（`866-954` 两例，见对抗项 4） | **PASS** |
| i18n parity 与缺 key failure | 见 M03.4 行；4 项测试直接覆盖缺/重/空 | **PASS** |
| 关键 UI screenshot/结构 smoke；不得用截图代替交互逻辑测试 | M03 记录：Slint 无可用稳定 screenshot 测试；窗口可显示由 MSVC CI artifact 构建验证（GNU 本地 fake data 搜索已验证）；交互逻辑全部由无 UI 状态机测试承担 | **PASS**（结构 smoke 以 CI 编译 + GNU 本地运行佐证，见 F003） |

## Findings（按严重级排序）

### F001（Medium）— 结果计数显示恒为 0

- **文件：** `src/main.rs:312-316`、`ui/app-window.slint:422-426`
- **问题：** `sets_count` 用 `Msg::ResultsCount(0)` 硬编码显示计数 0；`sync_ui`（`main.rs:135-193`）未把 `state.rows.len()` 推入任何 count 属性，`.slint:422-423` 在条件 `rows-count > 0` 下显示 `UiStrings.count`（此时恒含 `0`,如「0 个结果」）。`apply_localization_and_theme` 只执行一次，query 变化后也不会更新。M03.1 验收点「筛选按钮/数量」中的数量因此无真实语义（任务清单将筛选按钮后置 M05，但数量是本窗口固有）。
- **影响：** 可见可复现的错误显示（已淡入淡出、以 toggle 允许）——搜索结果数量始终错报为 0；与 ViewModel「单一状态来源」的设计意图不符（count 未纳入任何 ViewState 推送路径）。
- **证据：** `main.rs:312-316` 仅一处 `set_count` 调用且参数常量 `0`；`main.rs:135-193` 无其它 setter 触点；`.slint:422` 显示条件与 `rows-count` 绑定。
- **建议：** 将 count 加入 ViewState（或复用 `rows.len()`）并在 `sync_ui` 里每次 `set_count(Msg::ResultsCount(rows.len() as u16).tr(locale))`；或 M05 随筛选面板一起接线。此为中低优先级用户可见瑕疵，不阻断里程碑（见结论）。

### F002（Low）— 行 2 路径 tooltip 显示的是 path_label 而非 full_path

- **文件：** `ui/app-window.slint:140`（`row-path` 即 `ResultRow.path_label`）、`:156-168`、`state.rs:18-21`（`path_label` = 相对路径标签）、`view_model.rs:383-387`（`full_path` 填入但 `.slint` 未消费）
- **问题：** `ResultRow` 携带两个路径字段（`path_label` 与 `full_path`），ViewState 中也含 `full_path`（`state.rs:22-23`），但 `.slint` 的行 2 文本 `row-path` 绑定的是 `rows-path`（path_label），其 Tooltip（`164-168`）也只显示同一 `row-path`。M03.1 原文要求「长路径 ellipsis + tooltip」——语义上 tooltip 应给完整路径；当前实现 tooltip 与可见文本同内容，长路径无廉价的完整查看途径。
- **影响：** 低。可用性缺口：无法从 tooltip 查看全长路径（需复制/打开）。其余标注（圆角/行高/灰化）均达成。
- **证据：** `state.rs:21` 注明 `full_path` 用途是「path tooltip / Copy-Path」；`main.rs:150-153` 只 push `rows_path = path_label`；`.slint` 无第二个路径属性。
- **建议：** 把 `full_path` 作为独立 `row-full-path` 属性推入并绑定 tooltip（Copy-Path 已在 M03 stub 层次备好 `full_path`，复制时直接用），使 tooltip/Copy-Path 与显示文本解耦。修复顺手（M03.1 语义才完整）；不阻断里程碑。

### F003（Low）— 交互路径无自动化覆盖（单测零 UI）

- **文件：** `src/presentation/view_model.rs` 测试模块（`583-975`）、`ui/app-window.slint`
- **问题：** 所有 ViewModel 测试直接调用 `handle(...)`，`.slint` 的 key-pressed 映射（`Enter/Esc/↑/↓/Alt+H/Ctrl+C`）与 touch（click/double-click）没有自动化测试；`.slint` 编译由 CI 证明，但「UI 层的拦截/转发对错」只能由结构审查推断。
- **影响：** 低（非截图依赖——交互逻辑集中在无 UI 状态机，M03.5 已满足）；但 UI 层 key 分发（尤其 `ime-composing` 时 `reject` 分支、Ctrl+C 于 input 内 reject / results 内 accept 的分歧）属 wire 错误高发点，无回归保护。
- **证据：** `.slint` 无测试钩子；任务清单 M03.5 记录截图测试不可用。
- **建议：** 接受现状（仅记录的已知限制）或后续用轻量 ISA（Slint 的 key-pressed 映射这类纯转发，可用 rust 测试对 `SearchKey` 分布覆盖向量化——MGN 层无成本）；并明确列入桌面验收清单。不阻断里程碑。

### F004（Low）— `SearchFailure::Search` 可达路径无覆盖，`Load/Save` 为当前死变体

- **文件：** `view_model.rs:350-355`（`Err(())` 分支）、`state.rs:33-40`
- **问题：** 核心 `search_with_filter` 不返回 `Result`，`default_runner` 恒为 `Ok`（`view_model.rs:567-580`）；`SearchFailure::Search` 仅当注入 runner 返回 `Err` 时出现，测试未覆盖该分支；`Load`/`Save` 变体在 M06 存储接线前不可达。
- **影响：** 低。当前是「已实现的错误 UI 措辞，但无触达路径」——M06 接线时需重新验证这些措辞与真实错误映射；O 未引入 panic/dead-code 之外的死代码（enum 变体被明确保留为下一里程碑接口）。
- **证据：** `default_runner` 无 `Err` 路径；测试模块无 `Err(())` 用例。
- **建议：** 接受现状（M06 前置接线项，明确记录）；M06 评审时要包含 error 端到端。不阻断里程碑。

## Cross-cutting 检查（head）

- **正确性：** generation 陈旧拒绝两端道路径一致（同步 `accept_run_result` 与异步 `accept_outcome` 均比较 `self.generation != generation`）；`wrapping_add(1)` 与 `QueryGeneration` 同语义（M02.5 契约在 presenter 层复刻）；selection 合法化在 `rebuild_rows:403` + `SelectionMove::apply:137` 双处钳制；hide/overlay/Esc 顺序单一且无状态泄漏。
- **错误处理：** 零新增 `unwrap`/`expect`/`panic`（`main.rs:222` `expect("theme token must be #RRGGBB")` 与 `main.rs:215` `expect("fixed date")` 均为编译期常量解析、与既有 M00/M01 同模式）；`apply_effect` 对窗口已 dropped 静默接受（`main.rs:117-119` `if let`）；错误措辞匿名可操作。
- **数据安全：** `src/presentation/` + `src/main.rs` 零 `std::fs`、零真实目录删除引用（grep 零命中）；`view_model.rs` / `state.rs` 纯内存数据，不落盘；demo `SearchEntry`（`main.rs:66-71`）路径仅供展示，无探测、无删除。
- **隐私：** 零用户路径/搜索词进入日志（`src/presentation/`+`main.rs` 零 `eprintln`/log/`dbg!`；`std::process` 仅 `main.rs:507` 启动失败 `exit(1)`）；`report_platform_error` 明确「不打印 native 细节」（`main.rs:490-495`）；`diagnostics::SENSITIVE_FIELDS_REDACTED` seam 存在且 `as_detail` 匿名；`state.rs` 模块文档声明展示状态「never serialized, never logged」。`grep std::{fs,net,time,env,process,thread}` 于 `src/presentation/` 零命中。
- **安全性：** 无 shell 执行（对抗项 1 全绿）；`std::process` 唯一用途是 `--version` 路径与初始化失败 `exit(1)`；无新攻击面；无新依赖。
- **Windows 行为：** 8 文件范围不触 `src/platform/`、`src/app.rs`、tray/hotkey wiring（M04 保留面）；MSVC CI 全绿（含 release EXE 验证步骤）。`no-frame: true` + `always-on-top: true`（`app-window.slint:183-184`）为 M04 通知窗口风格，无 self-move/drag 逻辑（无 `Window.self.move`），真实窗口拖动语义待桌面。
- **测试覆盖：** 191 = 189 执行 + 1 ignored（benchmark）+ 2 main；CI 日志与静态计数逐项吻合；无 `#[should_panic]` 占位；M00 lifecycle（app 8）、M01 storage（61）、M02 search（87）、M03 presentation（32）均在 head 全绿（CI 证明既有测试未删除未弱化）。既有 M00/M02 的测试对 head 无回归。
- **性能：** `default_runner` 同步路径每键克隆 resolved（`view_model.rs:284` `self.resolved.clone()`）在 10k 基准语义下由 M02-B 覆盖（`filtered-with-clone median 61.58ms` head 实测）；M03 未新增热路径。
- **可访问性：** 见 M03.4 行（非色-alone、accessible-label/role、tab 焦点、无动画）；高对比/文本缩放留 M07/M08。
- **可维护性：** `presentation/mod.rs` 模块文档、`view_model.rs` 顶部 5 个设计文档块、`i18n.rs` 选择说明、`theme.rs` 像素注释——分层清晰，Rust 逻辑与 Slint 解耦良好；enum（ViewCommand/ExternalEffect/RowAction）集中、单一职责。

## 对抗性检查结论（task 指定的 11 项）

1. **Shell 执行 / 路径执行（对抗 §1）→ 无问题（ruling）。** `grep -rn "cmd\|powershell\|Command::new\|std::process\|explorer\|OpenProcess\|ShellExecute"` 于 `src/main.rs` + `src/presentation/` 唯一命中 `main.rs:507 std::process::exit(1)`（启动失败，不执行用户路径）；`ui/` 唯一命中「tray shell」注释。`ExternalEffect::OpenEntry` 在 `apply_effect` 中为**空实现 stub**（`main.rs:121-123`，注释明确「No shell command is constructed」）；`RowAction::Open` → `open_selected` → 仅发 `OpenEntry` effect（`view_model.rs:489-498`）。**无任何实际进程 spawn 用户路径的路径。**
2. **Search/domain 未触碰（对抗 §2）→ 通过。** `git diff --name-only 81c331b..7de3dd3 -- src/search src/domain src/storage src/lib.rs Cargo.toml Cargo.lock` 零输出；`src/lib.rs` 逐字一致。M03 仅消费（`default_runner` 用 `search_with_filter` / `QueryParser` / `FilterSet::into_filter` / `HighlightOptions::default`），全部 API 存在且签名匹配。
3. **日志/隐私（对抗 §3）→ 通过（见 Cross-cutting 隐私项）。** 无路径/搜索词写入日志；错误 Display 匿名；`SearchFailure::as_detail`（`state.rs:44-54`）即文档化安全 detail 接缝；`diagnostics.rs` seam 存在。
4. **Query generation 正确性（对抗 §4）→ 通过（ruling）。** `stale` 两个测试真实走「生成号不匹配」路径并对新 query 状态零污染：`generation_rejects_stale_completions`（`view_model.rs:866-891`，同步 runner 路径下用 `SearchCompleted` 注入旧 gen completion，断言新 gen busy/query 未被清）与 `stale_flag_shows_when_an_inflight_result_arrives_after_a_newer_query`（`895-954`，`Deferred` runner 真实异步语义，断言 older completion 置 `stale` 且新 query 保持 busy，然后 fresh completion 清状态并恢复 rows）。`query_edited` 在 `run_search_for_current_generation` 先 `bump_generation`（`261-264`），同步默认路径 `Done` 立即 `accept_run_result` 同 gen 接受，无竞态。generation 计数与 M02.5 `QueryGeneration`（`wrapping` 同）语义对齐。
5. **IME gate 逻辑级（对抗 §5）→ 通过（ruling）。** `open_selected`（`489-498` 经 `can_open_selected` 检验 `ime_composition_active`）+ `select_move_with_ime`（`436-441`）+ `select_index`（`449`）+ `select_id`（`456-457`）全部逻辑级拒绝；`.slint:262-265` 在 UI 层 `ime-composing` 时 `reject` Enter/箭头/Esc（IME 拥有按键）；测试 `793-810` 断言 composition 期间 Enter 无 effect、Down 不动，结束后续行为恢复。双层 gate 均非单一 UI 层。
6. **Selection 合法化（对抗 §6）→ 通过。** `SelectionMove::apply:137` 先把越界 index 过滤为 None 再 wrap；`rebuild_rows:403` 对 rows 变更后 clamp；wrap 边界确定性（`state.rs:126-148` 注释 + 4 测试）。Edge：`Down` 从 `Some(last)` → `Some(0)`（`144`）与注释一致。
7. **Hide gate（对抗 §7）→ 通过。** `hide_requested`（`511-518`）overlay open 时 deferred；Esc 路径 `search_key`（`418-427`）先关 overlay 再发 `HideWindow`；`toggle_filters`/`ContextMenu`（`468-473`、`500-504`）都会置 overlay；测试两处覆盖。
8. **i18n（对抗 §8）→ 通过。** Rust 侧目录唯一文本源；`.slint` 无 `@tr()` 业务文案（仅 tray 的 Slint 内建 `@tr("Open FileGo"`/`"Exit"` 为 M00 已有）；`Msg::id` 与目录 key 一一对应；parity 测试对缺 key/重复/空值均 fail。
9. **Theme（对抗 §9）→ 通过。** AA 对比测试 5 组配对×2 schema（text/background、secondary/background、secondary/surface、text/surface、on_accent/accent、warning/background）；`ResolvedTheme::resolve` system seam 文档化（`theme.rs:10-12`、`.slint:73-78`）；warning/selection 非色-only（glyph + 文字）。
10. **无 scope creep / 回归（对抗 §10）→ 通过。** 恰 8 文件；`#[allow]`/`unsafe` 于 `src/presentation/`+`main.rs`+`.slint` 零命中；真实目录删除零引用；CI 191 项测试 0 失败（M00 lifecycle、M01 storage、M02 search 全部在 head 通过）；确定性（presentation 零 `HashMap` 迭代序敏感？——`default_runner` 用 `HashMap<uuid,&ResolvedEntry>` 仅作 `by_id.get` 查询，遍历序不参与输出；`kept` 顺序由 `Ranked`/`EmptyQuery` 序决定，确定性保持）。
11. **Slint 薄适配（对抗 §11）→ 通过。** `.slint` 无状态决策逻辑（全部 `in-out` 由 Rust 推送；`if (root.ime-composing) { reject }` 是 UI 级第一道拦截但属「转发决策」且 ViewModel 侧有逻辑级第二道防御）；callbacks ↔ commands 一一接线（`main.rs:417-477` 17 个回调）；`.slint` 编译由 CI 证明；无死代码（`result` 参数在 sync 路径 unused 但为异步契约参数，非死代码）。

## 未能自动验证的项（列桌面/后续验收）

1. **真实 native IME preedit wiring**（Slint 1.18 无公开 preedit 事件）：`set_ime_composition` 的驱动 hook、`ime-composing` 的 UI 反射、中文输入法组合/候选窗口期间的 Enter/箭头行为——挂 M04 手动验收（`view_model.rs:28-38`、`main.rs:101-108`、`.slint:16-23` 已文档化）。
2. **system 深色模式的 widget palette**：`.slint:73-78` 注明 Slint 内建 Fluent widget 的 color-scheme 驱动在 Slint 1.18 无公开 `Window` API，当前自定义 token 由 `ResolvedTheme::resolve` 在 adapter 侧解析；原生 widget 调色板深色切换挂 M07/M08 桌面手动验证。
3. **真实文件夹打开/复制剪贴板**：`OpenEntry`/`CopyPath` 均为 effect stub（`main.rs:121-127`），真实 Explorer open 与 clipbboard 复制挂 M04/M05（`view_model.rs:113-117` 注释）。
4. **高对比 / 文本缩放不截断核心操作**：明确留 M07/M08（任务清单 M03.4 一致）。
5. **Ctrl+A/Ctrl+Backspace native 文本行为**、无边框窗口拖动/可访问性手势、真实键盘焦点环——桌面验收。
6. **早期视觉/IME smoke**：MSVC artifact 由 CI 构建上传，但窗口显示、中文输入法组合、主题切换等需用户早期桌面 smoke（任务清单 M03 验收）。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M03 主搜索窗口 + ViewModel；非发布批准）

- **范围与独立性：** `81c331b..7de3dd3` 恰 8 文件；search/domain/storage/lib/Cargo/CI 零变更；`src/presentation/` 从空壳到 5 模块全来自单一未参与的实现 commit `7de3dd3`。reviewer 独立性与范围边界严格。（对抗项 1/2/4/5/6 分别裁定：无 shell 执行、搜索/领域未动、generation 陈旧拒绝真实两测、IME gate 逻辑级、selection 合法化完整。）
- **CI：** head `7de3dd3` 的 Windows CI `35866194185`（job `107198274636`，**191 项测试 0 失败**，MSVC release EXE 验证通过，全绿）+ Search benchmark `35866193999`（job `107198273659`，`1 passed` + **6 行 BENCH 含 `filtered-with-clone median 61.58ms`**，守卫未触发）两个 run 独立核验，head SHA 均与 `git rev-parse HEAD` 一致。工作流零变更。
- **需求映射：** M03.1/M03.2/M03.3/M03.4/M03.5 全部验收点 **PASS**（其中「focus+全选」「Tab filter target」「右键菜单完整 menu」「reduce-motion」「高对比」五项按任务清单已显式后置 M04/M05/M07/M08 或以结构证据满足）；无 GAP。
- **Finding 处置：** F001（Medium，结果计数恒 0）应修复，但不具副作用/数据/安全影响，建议在 M05 或 M04 前关闭；F002（Low，tooltip 显示 path_label）与 F003（Low，UI 交互层无自动覆盖，记录为已知限制 + 桌面验收）、F004（Low，error 死变体留 M06）均不阻断。**本里程碑无需阻断性修复**；F001 作为未关闭的 Medium 延期项按 CLAUDE.md §4.5 需 reviewer 明确接受其影响（已在本结论接受）并在 M05/发布前关闭。
- **非发布批准：** 依 CLAUDE.md §4.5，本结论仅 `APPROVED_FOR_MILESTONE`，不构成 `APPROVED_FOR_RELEASE`；发布需后续独立发布评审、真实桌面手工验收（含上面「未能自动验证的项」）、F001 关闭及对应 response/re-review 闭环。
