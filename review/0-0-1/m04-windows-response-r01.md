# Response: M04 Windows 托盘、全局快捷键、单实例、窗口定位与 Shell 打开（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m04-windows`
- **轮次：** `r01`（对 `review/0-0-1/m04-windows-review-r01.md` 的逐条回应）
- **日期：** 2026-09-21（本地时间线；CI 时间见下方 CI 证据表）
- **Implementation agent：** 本 implementation agent（M04 r01 response）
- **独立性声明：** 本 response 由 **implementation agent** 撰写；按 CLAUDE.md §2，实施与评审分离，后续 r02 复审须由**未参与实现**的独立 code-review agent 执行。
- **对应 review 文件：** `review/0-0-1/m04-windows-review-r01.md`（verdict：`CHANGES_REQUESTED`；F001 High / F002 High 必须关闭，F003 Medium 建议修复）
- **修复前 commit SHA（review 对象 head）：** `828819b2819db61af089de75a8ba2e5b5b3add72`（`feat: wire Windows tray, global hotkey, single-instance and open`）
- **修复后 commit SHA（代码修复提交）：** `fba5f65e18031348a787ee820d5abfc27e1ca540`（`fix: register hotkey after window ready, make hidden window discoverable`，即 9 个代码文件修复；本 response 文档单独随同提交并记录该 SHA）
- **变更范围：** `src/platform/windows/mod.rs`、`src/platform/windows/hotkey_adapter.rs`、`src/platform/windows/window_placement.rs`、`src/main.rs`、`src/platform/hotkey.rs`、`src/platform/window_position.rs`、`src/presentation/{i18n,state,view_model}.rs`（F001–F003、F005–F008）、`review/0-0-1/m04-windows-response-r01.md`（本文件）。
- **未触碰：** `src/app.rs`、`src/storage/`、`src/search/`、`src/domain/`（settings 模型未变）、`src/lib.rs`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/`、`ui/app-window.slint`（tray 静态结构未变）、`src/platform/ipc.rs`、`src/platform/shell_open.rs`（纯错误映射未变）、生命周期状态机（M00）。

## 变更范围核验（无 scope creep）

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/platform/windows/mod.rs` | M | F001：`start()` 改为 pending machine + HWND 就绪后 `set()`（含重试一次）+ `hotkey_last_error` 暴露；F002：隐藏窗口改为顶层 popup（去掉 `HWND_MESSAGE`）；F005：worker 优雅退出（去掉 `std::process::abort()`）；新增 F002 类名一致性测试 |
| `src/platform/windows/hotkey_adapter.rs` | M | F001：新增 `publish_hwnd` 后注册目标切换的测试 |
| `src/platform/windows/window_placement.rs` | M | F003：新增纯函数 `physical_rect_for_monitor`（按目标 monitor DPI 换算）+ `placement_rect_for_cursor`（一次性入口）+ 3 个纯测试 |
| `src/main.rs` | M | F003：`place_window`（物理换算与放置改为按目标 monitor DPI 计算，并经 `set_position(Physical)`）；F006：`set_open_failure(kind)` 按 kind 选文案；F008：修正过时注释（invoke_from_event_loop → polling timer） |
| `src/platform/hotkey.rs` | M | F001：新增两段式启动注册的机器层测试 |
| `src/platform/window_position.rs` | M | F008：`TargetMonitor` 标注 RESERVED/预留 doc |
| `src/presentation/i18n.rs` | M | F006：新增 6 个按 kind 的 open-error 双语 key |
| `src/presentation/state.rs` | M | F006：`SearchFailure::Open(OpenErrorKind)` 携带匿名 kind，`as_detail` 委托 kind |
| `src/presentation/view_model.rs` | M | F006：`set_open_failure(kind)`；F007：新增 2 个直测 |

`git diff --name-only 828819b..HEAD` 不含 `src/app.rs`、`src/storage`、`src/search`、`src/domain`、`src/lib.rs`、`Cargo.toml`、`Cargo.lock`、`.github`；`ui/app-window.slint` 未变。无依赖新增、无 workflow 变更、无 task/ 文档入 Git。

---

## Finding 逐条回应

### F001（High）— 热键在隐藏窗口创建前注册、无重试且静默 Disabled → **ACCEPTED**

**评估：** 接受。reviewer 指出的竞态属实：旧代码 `NativePlatform::start` 在 spawn 之前 `HotkeyMachine::new(registry, hotkey_setting)`，而 `register` 需要 worker 的 HWND（slot 为空时返回 `Unavailable`）；注册先于 `CreateWindowExW` 时默认热键对整个会话静默 `Disabled`，且全树无后续重注册路径。

**修复内容（`src/platform/windows/mod.rs`，采纳 reviewer 建议的 option (b)）：**

1. **注册严格在 HWND 就绪之后：** `start()` 先以 pending 状态构造机器 `HotkeyMachine::new(registry, None)`（不注册），spawn worker，随后 `wait_for_hwnd`（既有 `mod.rs:347` 的轮询）等到 `Some(hwnd)` 后再调用 `hotkey_guard.set(hotkey_setting)`。`set` 在 `Disabled` 状态下对同一 combo 执行一次真实 `register`。
2. **重试一次路径：** 若 `set` 返回 `Err`，sleep 100ms 后对同一 `Disabled` 机器再 `set` 一次（F001 要求的 retry-once）。
3. **失败不再静默：** 注册失败后机器停留在 `Disabled` 且 `last_error = Some(kind)`；新增 `NativePlatform::hotkey_last_error()`（匿名 `HotkeyErrorKind`）可由 tray 侧读取并提示「快捷键不可用」，配合既有 `hotkey_state()`。
4. **WndProc 上下文先于发布：** worker 内先把 `set_thread_state`（events + hotkey 线程局部）建立好，再把 HWND 写入共享 slot——主线程一旦观察到 slot 非空，WndProc 上下文必然已就绪（WM_HOTKEY/WM_COPYDATA 不会在空上下文中被丢）。
5. **发布时机契约：** slot 只在 `CreateWindowExW` 成功后写入真实 HWND；创建失败则不发布（注册经 `Unavailable` 暴露）。

**新增测试（确定性，状态机/适配器层）：**

- `src/platform/hotkey.rs`：`startup_in_two_steps_registers_once_the_hwnd_is_ready` —— 模拟两段式启动：pending → `set` 后 `Active`、`last_error` 清空、`on_event` 触发 `Show`；冲突路径 → `Disabled + last_error(Conflict)` 而非静默成功。
- `src/platform/windows/hotkey_adapter.rs`：`publish_hwnd_changes_the_register_target_from_unavailable_to_the_window` —— 未发布前 `register` 按构造返回 `Unavailable`；`publish_hwnd` 后 `current_hwnd` 为目标窗口、`register` 走 OS 调用路径（不再按构造短路）。

**风险与剩余：** 真实 `RegisterHotKey` 的冲突/注册结果仍属 OS 时代行为（CI 无桌面），保留在 M07/M08 桌面验收（与 review「未能自动验证项 2/3」一致）；F001 的竞态已被结构性消除（顺序固定），非概率性。

---

### F002（High）— 隐藏窗口为 message-only、`FindWindowW` 不可枚举 → 第二实例激活可能失效 → **ACCEPTED**

**评估：** 接受。reviewer 指出实现与 `single_instance.rs` 模块顶部注释的意图（「deliberately use an invisible top-level popup」）矛盾：`mod.rs:117` 传 `Some(HWND_MESSAGE)` 创建 message-only 窗口，`FindWindowW`/`EnumWindows` 无法枚举，第二实例 `discover_primary_window` 可能在 2 秒预算内永远找不到端点 → `StaleEndpointExit`，主窗口不被激活。

**修复内容（`src/platform/windows/mod.rs`，采纳 reviewer 建议 (a)：真实不可见顶层 tool window）：**

- `CreateWindowExW` 的 parent 参数由 `Some(HWND_MESSAGE)` 改为 **`None`**（顶层窗口，无 owner）。
- 窗口样式保持 `WS_POPUP` + 扩展样式 `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`；**从不调用 `ShowWindow`**，窗口不可见、不进任务栏/alt-tab（toolwindow 抑制）；`WM_HOTKEY` / `WM_COPYDATA` 照常送达该 HWND。
- 该窗口现在是 `FindWindowW` 可枚举的 top-level 窗口，第二实例激活路径恢复可用。
- 删除 `HWND_MESSAGE` / `PostQuitMessage` 引用（模块顶部注释与 `single_instance.rs:3-5` 的注释与本实现一致）。

**新增测试（接线层，FFI 无法 headless）：**

- `src/platform/windows/mod.rs`：`hidden_window_class_is_shared_with_the_activator` —— worker 的 `HIDDEN_WINDOW_CLASS` 与 `activator::HIDDEN_WINDOW_CLASS` 必须一致且等于 `"FileGoHotkeyWindow"`；若两常量漂移，第二实例激活静默失效，本测试立即失败。

**风险与剩余：** 真实双实例 smoke（两个 exe 启动、第二实例激活首实例并退出、无第二托盘、主窗口 focus）无法托管 CI，列入 M07/M08 桌面手工验收（review「未能自动验证项 3」重点验证项）。

---

### F003（Medium）— per-monitor DPI 物理定位未按目标 monitor 换算 → **ACCEPTED**

**评估：** 接受。旧 `main.rs:place_window` 用 `window.scale_factor()`（Slint/wininit 窗口当前 scale）放大逻辑尺寸，未针对**目标（光标所在）monitor** 的 DPI；`window_placement.rs` 已封装 `dpi_scale_at`/`physical_size_for_cursor` 却零调用。混合 DPI（110/125/150/200%）多显示器下尺寸/夹取可能错误。

**修复内容（`src/platform/windows/window_placement.rs` + `src/main.rs`）：**

1. 新增**纯函数** `physical_rect_for_monitor(cursor, work_area, dpi_scale, logical_w, logical_h)`：用 `logical × 目标 monitor 的 dpi_scale` 计算物理尺寸，再交给既有纯 `compute_position` 做居中/顶部 10%/夹取。非有限/非正 scale 回退 1.0，尺寸下限 1px（抗退化）。
2. 新增一次性入口 `placement_rect_for_cursor(logical_w, logical_h)`：取 `GetCursorPos` → 该 monitor 的 work area（`placement_inputs`）→ `GetDpiForMonitor` scale（`dpi_scale_at`）→ 返回物理 `WindowRect`。
3. `place_window` 改为：`window.size().to_logical(window.scale_factor())` 取逻辑尺寸（仅作当前窗口逻辑尺寸来源），随后交给 `physical_rect_for_monitor`（按目标 monitor scale 换算物理尺寸），再 `window.set_position(Physical)`。不再用当前窗口 scale 直接放大。

**新增测试（3 项，纯函数确定性）：**

- `physical_rect_for_monitor_uses_the_target_monitor_dpi`：合成 200% 副屏（3840..7680×2160），600×140 逻辑 → 1200×280 物理、水平居中、顶部 10%、不跨屏。
- `physical_rect_for_monitor_clamps_with_the_target_scale`：150% scale 下超屏窗口按物理尺寸夹取在 work area 内。
- `physical_rect_for_monitor_defends_bad_scale_and_large_sizes`：NaN/0/负 scale 回退 1.0；0 逻辑尺寸下限 1px。

**风险与剩余：** 单 monitor 100% 缩放等价（scale=1）；混合 DPI 真实多显示器定位列入 M07/M08 桌面验收（review「未能自动验证项 4」）。

---

### F004（Low）— 同一快捷键实现为「显示/focus」而非真正 toggle → **RECORDED（首版显式决策，无代码变更）**

**评估：** 按任务指令与 reviewer「在 response 中显式记录，或复用 Toggle（VM 层已有测试覆盖）」二选一，**选择记录为显式实现决策（Show）**，本轮**不改代码**。理由：

1. **双实例/快捷键的「始终显示」语义被明确保留：** 本响应任务范围明确「不得破坏 always show on double-instance/hotkey 语义」。若 hotkey 路径改 `Toggle`，窗口已可见时再按热键会 *隐藏*，与启动器「召唤即出」的最不意外行为冲突（且第二实例激活也必须恒 Show——用户刚双击 exe）。
2. **`LifecycleController::Toggle` 已存在且有测试**（`app.rs::starts_hidden_and_toggles_both_directions` 等），未来切片切到 Toggle 只是适配层一行改动，无结构性风险。
3. **任务文档 M04.2 已记录**「实现为 Show（显示/focus）；toggle 语义处于 worker WM_HOTKEY 匹配，M04.2 验收项」；review 本身的 M04.2 验收行也判定「PASS（实现为 Show 语义）」，说明 Show 是该轮已接受的实现选择，需求/文档/实现语义已对齐。
4. 真实按下热键是否应隐藏、与前台限制 fallback 的交互，需真实桌面验证后才能定论 —— 记入 M07/M08 手工验收项。

**建议后续切片：** M07/M08 手工验收确认后，若产品要求 toggle，把 `main.rs` 的 hotkey 分支改为 `LifecycleCommand::Toggle`（第二实例 `ActivateFromSecondInstance` 保持 `Show`），并补充对应集成测试。

---

### F005（Low）— worker `std::process::abort()` 在正常/半正常退出路径无条件终止 → **ACCEPTED**

**评估：** 接受。`worker_main` 在 `GetMessage` 返回 0（WM_QUIT）/-1 后 `DestroyWindow` + `PostQuitMessage(0)` 再 `abort()`；正常退出路径用 `abort()` 跳过清理，属可维护性异味，若未来 worker 需退出前清理会受影响。

**修复内容（`src/platform/windows/mod.rs`）：** worker 改为返回 `()`，消息循环退出后 `DestroyWindow` 自然结束线程（无 `abort()`）。进程整体退出仍由主线程 `quit_event_loop` 主导（tray shell 存活依赖主循环，worker 单独结束不应杀进程）——注释保留该终态说明。

**风险：** `RegisterHotKey` 进程退出时由 OS 会话级清理（已文档化），worker 返回不影响超时/清理路径；`InstanceMutex::Drop` 仍在主线程 guard 上。无行为回退。

---

### F006（Low）— 打开失败未按 kind 展示可操作文案 → **ACCEPTED**

**评估：** 接受。`OpenErrorKind` 六种失败已由 `shell_open.rs`/`tray_open.rs` 正确映射，但 `main.rs:apply_open_result` 忽略 kind，UI 一律显示通用文案；AccessDenied/NotFound 等本可区分。

**修复内容（仍然匿名、无路径）：**

- `src/presentation/state.rs`：`SearchFailure::Open(OpenErrorKind)` 携带匿名 kind；`as_detail` 委托 `kind.as_detail()`。
- `src/presentation/view_model.rs`：`set_open_failure(kind: OpenErrorKind)`。
- `src/presentation/i18n.rs`：新增 6 个按 kind 双语 key（`error.open.not_found` / `access_denied` / `no_association` / `dde` / `shell_rejected` / `unavailable`），保留 `error.open.body` 作为通用回退。parity/去重测试自动覆盖新 key。
- `src/main.rs`：`apply_open_result` 传 `kind`；`sync_ui` 按 `SearchFailure::Open(kind)` 选择对应 `Msg`（通用回退 `ErrorOpenBody`）。

**测试：** F007 的 2 个 VM 直测覆盖 kind 携带；i18n parity 测试覆盖新 key 双语齐全；`state.rs` `as_detail` 委托不引入路径。

---

### F007（Low）— `set_open_failure`/`clear_failure` 无直接单测 → **ACCEPTED**

**评估：** 接受。VM 层 Open 失败状态机零直测。

**修复内容（`src/presentation/view_model.rs`，新增 2 项直测）：**

- `set_open_failure_sets_open_state_and_keeps_rows_selection`：`set_open_failure(AccessDenied)` 后 `failure == Some(Open(AccessDenied))` 且 rows/selection 不变。
- `clear_failure_only_clears_the_open_failure`：`clear_failure` 清 Open；不误清非 Open 的 `SearchFailure::Search`。

---

### F008（Info）— 过时注释 `invoke_from_event_loop` / `TargetMonitor::Primary` 未接线 → **ACCEPTED**

**评估：** 接受。

**修复内容：**

- `src/main.rs`：`apply_effect` 注释改为「结果经 channel 由 UI 线程轮询 timer（`open_drain`）drain 到 `apply_open_result`」，与实际实现的 polling timer 一致。
- `src/platform/window_position.rs`：`TargetMonitor` 增加 RESERVED doc（当前放置固定光标 monitor 策略，`Cursor`/`Primary` 两策略留给未来 settings-driven adapter 选择，不新增形状、不删除可用枚举）。

**风险：** 纯文档变更，无行为影响。

---

## GNU 本地验证证据（快速反馈；非 MSVC 权威）

按 CLAUDE.md §3.1 预检 + 验证命令（toolchain `1.92.0-x86_64-pc-windows-gnu`，MinGW64）：

| 预检项 | 结果 |
|---|---|
| `rustc --version` / `cargo --version` | `rustc 1.92.0` / `cargo 1.92.0` |
| `rustup target list --installed` | `x86_64-pc-windows-gnu` ✓ |
| `rustup component list --installed` | `rustfmt-x86_64-pc-windows-gnu` ✓ `clippy-x86_64-pc-windows-gnu` ✓ |
| `RUSTUP_AUTO_INSTALL=0` 与 `RUSTUP_TOOLCHAIN` | 已设置，无自动下载 |

| 命令（`--target x86_64-pc-windows-gnu`） | 结果 |
|---|---|
| `cargo fmt --all; cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**（无 warning） |
| `cargo test --workspace --all-features --locked` | **PASS**：lib **264 passed; 0 failed; 1 ignored**（ignored = M02-B 基准）+ main bin **3 passed** + 其余 bin 0；全部通过 |
| `cargo build --workspace --all-features --release --locked` | **PASS**（GNU release EXE 生成） |

- **统计口径：** 修复前 lib **257**（256 passed + 1 ignored，MSVC CI 实测）；本轮新增 **8** 项（hotkey 1 + hotkey_adapter 1 + window_placement 3 + windows/mod 1 + view_model 2）→ 静态 `#[test]` lib = **265**，与 `running 264 passed + 1 ignored` 一致。
- **GNU 链接器漂移备注：** 本环境需要 `D:\mingw64\bin` 在 PATH 且 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"`（windres 依赖）；未见 `cannot find -lshlwapi`。已按任务预授权工作区设置。
- **声明：** 本地 GNU 通过仅是快速反馈证据，不替代远程 MSVC CI；M04 为 Windows-native 里程碑，**MSVC CI 为权威门禁**（见下）。本地构建产物不用于发布/哈希。

## CI 证据（远程 MSVC，推送到 feature/m00-foundation 后触发）

| Run | Head SHA | Workflow / job | 结果 | 说明 |
|---|---|---|---|---|
| 见下方 `gh run list` 输出 | 本响应提交的 new SHA | `Windows CI` / 全 job（fmt, clippy, test, release, package, deny, about） | 推送后定位 run ID 并监控到结束 | 本响应随代码提交推送后触发；失败则修复重推直到全绿 |
| 见下方 `gh run list` 输出 | 同 new SHA | `Search benchmark` | 同上 | 同上 |

> 主 agent 在推送本 commit 后以 `gh.exe run list --branch feature/m00-foundation --limit 4 --json databaseId,workflowName,headSha,status,conclusion,url` 定位 run ID 并监控到结束；Windows CI 的 MSVC `cargo test`（264 passed）与 release/package 守卫为权威门禁。

## 未解决事项与待人工验证项（并入 M07/M08 桌面手工验收）

1. **双实例 smoke（F002 桌面验证重点）：** 启动两个 exe —— 第二实例应立即激活首实例并退出、无第二托盘、主窗口 focus；隐藏顶层 tool window 可被 `FindWindowW` 找到（隐式验证首实例激活成功）。
2. **全局快捷键真实注册与触发（F001 桌面验证重点）：** 默认 Ctrl+Alt+Space 可用性、快速连按、pause/resume、与其它软件冲突时 tray 能提示（`hotkey_last_error` 暴露路径）；确认 Start 后热键一直可用（F001 竞态已结构性消除）。
3. **混合 DPI 多显示器定位（F003 桌面验证重点）：** 光标在副屏 → 出现在副屏、垂直 10%、任务栏四边不遮；100/125/150/200% 的物理尺寸与位置。
4. **托盘真实渲染**（图标深/浅、7 项菜单、开机启动/暂停 glyph、关于、退出）、**前台限制 fallback**、**Explorer 重启托盘恢复**、**UNC/无权限打开**（F006 的按 kind 文案在桌面可见）、**IME preedit 接线**（M03/M04 遗留）。
5. **F004 决策的后续确认：** 若产品最终要求同一快捷键 toggle（显示↔隐藏），在手工确认后把 hotkey 分支切到 `Toggle`（第二实例保持 Show）。

## 请求复审

请独立 code-review agent 对本 response 对应 commit（`a7ff1d4..new` 的代码 diff、新增 8 项测试）与对应 MSVC CI run 进行 **r02 复审**，按 CLAUDE.md §4.5 确认：**F001（High）与 F002（High）已关闭**（本轮 MUST-close 项），F003（Medium）已修复，F004 记录可接受，F005–F008 已修复/记录，并给出复审结论（`CHANGES_REQUESTED` / `APPROVED_FOR_MILESTONE` / `APPROVED_FOR_RELEASE`）。**本 response 不构成发布批准**；发布仍须后续独立 release review `APPROVED_FOR_RELEASE` + 用户手工验收。
