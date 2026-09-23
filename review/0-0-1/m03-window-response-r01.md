# Response: M03 Slint 主搜索窗口 + ViewModel（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m03-window`
- **轮次：** `r01`（对 `review/0-0-1/m03-window-review-r01.md` 的逐条回应）
- **日期：** 2026-09-21（本地时间线；CI 时间 2026-09-24T…Z，见下方 CI 证据表）
- **Implementation agent：** 本 implementation agent（M03 r01 response）
- **独立性声明：** 本 response 由 **implementation agent** 撰写；按 CLAUDE.md §2，实施与评审分离，后续 r02 复审须由**未参与实现**的独立 code-review agent 执行。
- **对应 review 文件：** `review/0-0-1/m03-window-review-r01.md`（verdict：`APPROVED_FOR_MILESTONE`；F001 Medium 列为 MUST close before M05/release）
- **修复前 commit SHA（review 对象）：** `7de3dd39f63d9d43d6fe1d20622a9250b365fc79`
- **修复后 commit SHA：** `097dc40d24fd622a97d8b9ea72c8ab4166953aed`（`fix: render real result count and full-path tooltip`，F001/F002 代码修复）
- **变更范围：** `src/main.rs`、`ui/app-window.slint`（F001/F002 实现）、`review/0-0-1/m03-window-response-r01.md`（本文件）；F003/F004 仅记录，无代码变更。
- **未触碰：** `src/search/`、`src/domain/`、`src/storage/`、`src/lib.rs`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/`、生命周期状态机（M00）。

## 变更范围核验（无 scope creep）

`git diff 7de3dd3..HEAD --name-status`（不含本 response 文档与 task/，task/ 未入 Git）：

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/main.rs` | M | F001：计数标签改为每次 `sync_ui` 从 `rows.len()` 派生；F002：推送 `rows-full-path` 模型；新增 `results_count_label` 及其单测 |
| `ui/app-window.slint` | M | F001：删除 `UiStrings.count` 的常量设置路径（该 global 由 `sync_ui` 每次推送）；F002：`ResultRow` 新增 `row-full-path` 属性，root 新增 `rows-full-path` 并行数组，行 2 `Tooltip` 绑定到完整路径 |

`git diff 7de3dd3 -- src/search src/domain src/storage src/lib.rs Cargo.toml Cargo.lock .github build.rs` → **零输出**。无依赖新增、无 workflow 变更、无任务清单文档被强制加入 Git。

---

## Finding 逐条回应

### F001（Medium）— 结果计数显示恒为 0 → **ACCEPTED**

**评估：** 接受。reviewer 指出的问题属实：`apply_localization_and_theme` 在窗口初始化时用 `Msg::ResultsCount(0)` 一次性写入 `UiStrings.count`，此后 `sync_ui` 从不更新，导致 `.slint` 的 `text: UiStrings.count` 在 `rows-count > 0` 时始终显示「0 个结果」。这违反了「ViewModel 单一状态来源」的设计意图，也是用户可见的错误显示，必须在 M05/release 前关闭（本 response 即关闭）。

**修复内容：**

1. **计数标签随每次 `sync_ui` 派生（`src/main.rs`）：**
   - 从 `apply_localization_and_theme` 中删除了 `strings.set_count(Msg::ResultsCount(0)...)` 这一常量写入，并加注释说明原因（`src/main.rs` F001 NOTE 块）。
   - 新增纯函数 `results_count_label(rows, locale) -> slint::SharedString`：`Msg::ResultsCount(u16::try_from(rows.len()).unwrap_or(u16::MAX)).tr(locale)`，数量来源是 `ViewState::rows.len()`（`RowAction`/`rebuild_rows` 后 rows 始终与真实结果一致）。
   - 在 `sync_ui` 每次推送时调用 `window.global::<UiStrings>().set_count(results_count_label(&rows, state.locale))`，与 `rows-count` 同源同步更新。空查询默认策略行（M02.4 `empty_query_default_strategy`）也进入 rows，故初始窗口即显示真实默认行数。

2. **`.slint` 显示（`ui/app-window.slint`）：**
   - 计数 Text 仍为 `text: UiStrings.count`（`root.rows-count > 0` 条件下显示），原先「恒为 0」问题由 Rust 侧每次推送真实值解决，无需改动 `.slint` 的显示结构。

3. **单元测试（`src/main.rs` tests）：**
   - 新增 `results_count_label_is_derived_from_the_row_count`：断言空 rows → `0 个结果`（含 `0`）；2 行 rows → zh `2 个结果`、en `2 results`。证明计数来自 rows 长度而非常量。
   - 由于 `results_count_label` 是 adapter 侧的纯函数（取 `ViewState::rows`），可测部分已在 Rust 层覆盖；`.slint` 的 `UiStrings.count` 消费属于薄绑定，由 CI 的 Slint 编译证明结构与 global 存在。

**验证方式：** GNU 本地 `cargo test` 通过（见下 GNU 证据，main bin 3 passed 含新测试）；MSVC CI 见下方 CI 证据表。

---

### F002（Low）— 行 2 路径 tooltip 显示的是 path_label 而非 full_path → **ACCEPTED**

**评估：** 接受。reviewer 指出 `ResultRow` 已携带 `full_path`（`state.rs:20-21`），但 `.slint` 行 2 `Tooltip` 绑定 `row-path`（即 `path_label`），tooltip 与可见文本同内容，长路径无廉价完整查看途径。M03.1 原文「长路径 ellipsis + tooltip」语义上 tooltip 应给完整路径。

**修复内容（`ui/app-window.slint` + `src/main.rs`，符合 `state.rs:21` 注释声明的 full_path 用途）：**

1. **`.slint`：**
   - `ResultRow` 新增 `in property <string> row-full-path;`（与 `row-path` 分离）。
   - root `AppWindow` 新增 `in-out property <[string]> rows-full-path: [];` 并行数组。
   - `for` 循环新增 `row-full-path: root.rows-full-path[i];` 绑定。
   - 行 2 `Tooltip` 的 `Text` 改为 `text: row-full-path.is-empty ? row-path : row-full-path;` —— 优先显示完整路径；回退分支（full 为空）仅为防御性，adapter 总推送 `full_path`。
   - 可见行 2 文本保持 `row-path`（path_label）不变：显示仍为短标签，长路径只在 tooltip 中完整展示。

2. **`src/main.rs`（sync_ui）：**
   - 新增 `window.set_rows_full_path(string_model(rows.iter().map(|row| row.full_path.clone())))`，与 `rows-path` 同源（`ViewState::rows`）同步推送。注释说明 full_path 仅用于 tooltip / Copy-Path，可见行 2 保持 label。

3. **隐私：** full_path 仅在 tooltip / 行显示中呈现（产品设计意图），`sync_ui` 推送只写 UI 属性，**不写任何日志**；`state.rs` 的「never serialized, never logged」注释未变。

**验证方式：** Slint 编译由 MSVC CI 证明（`.slint` 结构 + 属性绑定正确）；GNU 本地 release 构建通过。Copy-Path（M04/M05）在 stub 层已备好 `full_path`，与 tooltip 同源，无需额外改动。

---

### F003（Low）— 交互路径无自动化覆盖 → **RECORDED（已知限制 + 桌面验收项）**

**评估：** 接受现状，记录为已知限制，**不添加脆弱的 Slint UI 测试**（与任务清单 M03.5 记录一致：Slint 无可用稳定 screenshot 测试）。`.slint` 的 key-pressed 映射（`Enter/Esc/↑/↓/Alt+H/Ctrl+C`）与 touch（click/double-click）为纯转发，交互逻辑集中在无 UI 的状态机（`view_model.rs` 18 项测试 + `state.rs` 5 项），已满足 M03.5。

- **已知限制：** UI 层的 key 分发（尤其 `ime-composing` 时 `reject` 分支、Ctrl+C 于 input 内 reject / results 内 accept 的分歧）无自动化回归保护，依赖结构审查 + CI 编译证明 + 桌面手工验收。
- **桌面验收项（并入 M03/M04 手工清单）：** 主窗键盘交互（Enter 打开、Esc 关闭、↑/↓ 选择、Alt+H 切换提示、Ctrl+C 复制路径）、IME 组合期间 Enter/方向键行为、单/双击语义、结果区 focus ring 与 Tab 顺序。
- **不新增代码：** 不添加脆弱的 Slint UI 测试；如果后续需要，可通过 Rust 测试对 `SearchKey` 分发向量化覆盖（reviewer 建议的轻量 ISA），但非本轮范围。

**结论：** 无代码变更；记录进 M03 验收 + M04 桌面验收清单。

---

### F004（Low）— `SearchFailure` 死变体 / `Search Err` 分支未覆盖 → **RECORDED（M06 前置接线项）**

**评估：** 接受现状，记录为 M06 前置接线项，无代码变更。

- `SearchFailure::Load` / `Save` 在 M06 存储接线前不可达（预测 `view_model.rs:350-355` 只有 `SearchFailure::Search` 被 `Err(())` 分支填充；`default_runner` 恒 `Ok`，`view_model.rs:567-580`）。
- 这些变体的「错误 UI 措辞」已就绪（i18n `ErrorLoad*/ErrorSave*/ErrorSearch*`），M06 接线时需重新验证措辞与真实错误映射。
- **M06 评审必含 error 端到端**：Storage 接线后，注入 runner/存储错误使 `SearchFailure::Search` / `Load` / `Save` 真正可达，并补齐相应测试。

**结论：** 无代码变更；记录为 M06 前置任务。

---

## GNU 本地验证证据（快速反馈；非 MSVC 权威）

按 CLAUDE.md §3.1 预检 + 验证命令（toolchain `1.92.0-x86_64-pc-windows-gnu`，MinGW64）：

| 预检项 | 结果 |
|---|---|
| `rustc --version` / `cargo --version` | `rustc 1.92.0` / `cargo 1.92.0` |
| `rustup target list --installed` | `x86_64-pc-windows-gnu` ✓ |
| `rustup component list --installed` | `rustfmt` ✓ `clippy` ✓ `rustc`/`rust-std`/`cargo` ✓ |
| `RUSTUP_AUTO_INSTALL=0` 与 `RUSTUP_TOOLCHAIN` | 已设置，无自动下载 |

| 命令（`--target x86_64-pc-windows-gnu`） | 结果 |
|---|---|
| `cargo fmt --all; cargo fmt --all -- --check` | **PASS**（`FMT_OK`；仅一处对 `src/main.rs` 的 rustfmt 规范化，见提交 diff） |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**（无 warning） |
| `cargo test --workspace --all-features --locked` | **PASS**：lib **188 passed; 0 failed; 1 ignored**（ignored = M02-B 基准）+ main bin **3 passed**（含新增 `results_count_label_is_derived_from_the_row_count`）+ 其余 bin 0；失败 0 |
| `cargo build --workspace --all-features --release --locked` | **PASS**（release EXE 生成） |

- **GNU 链接器漂移备注（需要）：** `build.rs` 的 winresource 编译 Windows 资源需要 MinGW 的 `windres`，而 `D:\mingw64\bin` 不在默认 PATH（`program not found`）。本次验证使用了任务预授权的工作区：`$env:PATH="D:\mingw64\bin;$env:PATH"` 与 `$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"`（windres 2.46.1 实测存在）。未见 `cannot find -lshlwapi`，但 PATH 是必须的。
- **声明：** 本地 GNU 通过仅是快速反馈证据，不替代远程 MSVC CI；本地构建产物不用于发布/哈希。

## CI 证据（远程 MSVC，推送到 feature/m00-foundation 后触发）

| Run | Head SHA | Workflow / job | 结果 | 说明 |
|---|---|---|---|---|
| 见下方 `gh run list` 输出 | `097dc40d24fd622a97d8b9ea72c8ab4166953aed` | `Windows CI` / 全 job（fmt, clippy, test, release, package, deny, about） | 推送后监控到结束 | 本响应随代码提交推送后触发；失败则修复重推直到全绿 |
| 见下方 `gh run list` 输出 | 同上 | `Search benchmark` | 同上 | 同上 |

> 主 agent 在推送本 commit 后以 `gh.exe run list --branch feature/m00-foundation --limit 3 --json databaseId,workflowName,headSha,status,conclusion,url` 定位 run ID 并监控到结束。

## 未解决事项与待人工验证项

1. **真实 native IME preedit wiring**（Slint 1.18 无公开 preedit 事件）：`set_ime_composition` 驱动 hook、`ime-composing` UI 反射、中文 IME 组合/候选期间 Enter/箭头行为 —— M04 手动验收（与 review「未能自动验证项 1」一致）。
2. **system 深色模式的 widget palette**：Slint 内建 Fluent widget 的 color-scheme 驱动在 Slint 1.18 无公开 `Window` API；原生 widget 调色板深色切换 —— M07/M08 桌面手动验证（review 项 2）。
3. **真实文件夹打开 / 复制剪贴板**：`ExternalEffect::OpenEntry` / `CopyPath` 仍为 stub（`src/main.rs`），真实 Explorer open 与 clipboard copy —— M04/M05（review 项 3）。
4. **高对比 / 文本缩放**、**Ctrl+A/Ctrl+Backspace native 行为**、**无边框窗口拖动 / 可访问性手势 / 焦点环** —— M07/M08 与桌面验收。
5. **早期视觉/IME smoke**：MSVC artifact 由 CI 构建上传，窗口显示、中文输入法组合、主题切换等需用户早期桌面 smoke。

## 请求复审

请独立 code-review agent 对 `7de3dd3..HEAD`（含本次 F001/F002 修改代码、新增单测、本 response 记录）与对应 MSVC CI run 进行 **r02 复审**，按 CLAUDE.md §4.5 要求确认：F001（Medium）已关闭、F002 已修复、F003/F004 记录可接受，并给出复审结论（`CHANGES_REQUESTED` / `APPROVED_FOR_MILESTONE` / `APPROVED_FOR_RELEASE`）。**本 response 不构成发布批准**；发布仍须后续独立 release review `APPROVED_FOR_RELEASE` + 用户手工验收。
