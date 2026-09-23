# Review: M04 Windows 托盘、全局快捷键、单实例、窗口定位与 Shell 打开（Round 02）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m04-windows`
- **轮次：** `r02`（对 `review/0-0-1/m04-windows-response-r01.md` 及修复 commit `fba5f65` 的复审）
- **日期：** 2026-09-21（本地时间线；CI 时间 2026-09-23T17:07–17:12Z，见 CI 证据表）
- **Reviewer：** 独立 code-review agent（M04 r02 复审）
- **独立性声明：** 本 reviewer **未参与** M04 实现/修复 commit `fba5f65` 与 `828819b` 的任何编码，**未参与** r01/r02 response 与 review 文档撰写，未参与 CI 触发与监控。本评审严格只读：**未编辑任何源代码、未运行任何 `cargo` 命令（依照 CLAUDE.md §3.1）**、未提交、未推送；除本 review 审计文档外未写入任何其他 Git 追踪文件。
- **Base SHA（比较基准）：** `828819b2819db61af089de75a8ba2e5b5b3add72`（M04 r01 review 头，`feat: wire Windows tray, global hotkey, single-instance and open`）。
- **Head SHA（评审对象）：** `d50717493f1db53eb6d7e9dfb534ea74fa41f30d`（`docs: respond to M04 windows review r01`）——`git rev-parse HEAD` 实测一致；head 包含修复 commit `fba5f65e18031348a787ee820d5abfc27e1ca540`（`git merge-base --is-ancestor fba5f65 HEAD` 实测成立）。
- **比较范围：** `828819b..d507174`，`git diff --stat` **9 个代码文件** + `review/` 2 个文档（response + 本 review 的目标修正），+876/−61。
- **审查文件（代码）：** `src/main.rs`、`src/platform/hotkey.rs`、`src/platform/window_position.rs`、`src/platform/windows/{mod,hotkey_adapter,window_placement}.rs`、`src/presentation/{i18n,state,view_model}.rs`。
- **上下文读取（审计/上下文）：** `review/0-0-1/m04-windows-{review,response}-r01.md`、`src/platform/windows/single_instance.rs` 及其 `activator.rs`（r01 范围外，但为 F002 复审必须逐字重读）、`src/platform/window_position.rs`（`compute_position`）、`src/platform/windows/mod.rs`（head 逐字）、`src/main.rs`（head 逐字）。

## 复审方法

1. `git diff 828819b..d507174` 逐字审查 9 个代码文件 + response 文档；
2. 用 `git show fba5f65:<file>` 与 `git show d507174:<file>` 直接读取 head/修复 commit 代码（不依赖任何转述）核验关键实现，并对头注释、测试、F001/F002 时序、F003 几何、F006 文案、F007 VM 直测逐项对码；
3. 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 独立查询 2 个 CI run 的 head SHA、workflow、job/steps 结论、关键日志行（test result 计数、BENCH 行、守卫、release EXE 校验）；
4. 对抗性 grep：`Command::new`/`powershell`/`cmd`/interpreter、`#[allow]`、`std::process::abort`、`HWND_MESSAGE`/`PostQuitMessage` 残留、`invoke_from_event_loop` 过时注释、路径入日志、`HashMap` 迭代序依赖；
5. 统计口径核对（静态 `#[test]` 计数 vs CI 日志 `running N tests`）；
6. r01 F001–F008 逐条重新对码（**未参与实现的 reviewer 直接验证代码而非采信 response 总结**）。

## CI 证据（reviewer 独立核验）

任务给定 CI 范围在 head `d507174`；两 run 均由本 reviewer 用指定 gh.exe 独立查询：

| Run | Head SHA | Workflow / job | 结论 | 核验内容 |
|---|---|---|---|---|
| [35893500715](https://github.com/yorelll/filego/actions/runs/35893500715)（job 107291540276） | `d50717493…` | `Windows CI` / job `fmt, clippy, test, release, package` | `success`（completed，push event，`feature/m00-foundation`，displayTitle `docs: respond to M04 windows review r01`） | `gh run view --json headSha` = 与 `git rev-parse HEAD` 一致的 `d50717493f1db53eb6d7e9dfb534ea74fa41f30d`；job 内 39 step 中活跃 step 19 个全绿（fmt → clippy `-D warnings` → tests → release build → `Verify release outputs and version helper`（MSVC release EXE/版本守卫）→ cargo-deny advisory/licenses/bans/sources → cargo-about inventory + sanity-check → portable artifact 构建/上传）。`--log` 实测：lib **running 265 → `test result: ok. 264 passed; 0 failed; 1 ignored`**（ignored = M02-B release 基准）+ main bin **running 3 → `test result: ok. 3 passed`** + 其余 bin 0 passed。 |
| [35893500620](https://github.com/yorelll/filego/actions/runs/35893500620) | `d50717493…` | `Search benchmark` / job `release 10k benchmark` | `success`（completed，push event，同 head/同时间戳） | `gh run view --json headSha` 一致。`--log` 实测 `running 1 test` → `test result: ok. 1 passed; …; 264 filtered out` + 6 行 BENCH：`empty-query-default 0.79ms` / `filtered-with-clone 51.86ms` / `pinyin-heavy 68.70ms` / `english-initials 51.53ms` / `edit-distance 63.25ms` / `multi-token 48.05ms`（medians）；`if ($null -eq $summary) { throw }` 守卫未触发（BENCH 行存在且 job success）。 |
| 补 | `git diff --name-only 828819b..d507174 -- .github` | — | 零输出 | 两 workflow（ci.yml / benchmark.yml）在本 range 内零变更。 |

- **统计口径核验：** head 静态 `#[test]` 计数 **lib 265 + main 3**（逐文件 `git show` + `grep -c` 实测），与 response 声明「264 passed + 1 ignored = 265」及 CI 日志 `running 265` 完全一致；main 3 与 CI `running 3 / 3 passed` 一致。新增 8 项：hotkey 1（`startup_in_two_steps_…`）、hotkey_adapter 1（`publish_hwnd_changes_…`）、window_placement 3（`physical_rect_…` ×3）、windows/mod 1（`hidden_window_class_is_shared_…`）、view_model 2（`set_open_failure_…`、`clear_failure_only_…`）。**新增测试在 MSVC CI 中真实执行并通过。**
- **MSVC 门禁核验：** head Windows CI 全绿覆盖 §3.2 的 1–5 项（fmt / clippy `-D warnings` / 265 项 lib 测试 0 失败 / MSVC release build / EXE+版本守卫），并含 cargo-deny 与 cargo-about；M04 为 Windows-native 里程碑，MSVC CI 为权威门禁——**成立**。本 review 的上游 run（修复 commit `fba5f65` 未被单独推送，`fba5f65` 与 doc commit `d507174` 一并作为 head 推送触发 CI；head 已包含全部 9 个代码文件修复与 8 项新测试，故 head 单 run 即为有效证据）。

## 变更范围核验（无 scope creep）

| 文件（head 行号） | 类型 | 主要变更 |
|---|---|---|
| `review/0-0-1/m04-windows-response-r01.md` | A（doc，Git 追踪） | r01 逐条 response（F001–F008）；修复/记录 commit SHA 记录 |
| `review/0-0-1/m04-windows-review-r01.md` | A（doc） | r01 评审记录（本 range 内随 r02 一并入库） |
| `src/platform/windows/mod.rs` | M | F001 pending 启动两段式 + `hotkey_last_error`；F002 顶层 popup；F005 worker 返回 `()`；`hidden_window_class_is_shared_with_the_activator` 测试 |
| `src/platform/windows/hotkey_adapter.rs` | M | F001 `publish_hwnd_changes_the_register_target…` 测试 |
| `src/platform/windows/window_placement.rs` | M | F003 `physical_rect_for_monitor` + `placement_rect_for_cursor` + 3 纯测试 |
| `src/main.rs` | M | F003 `place_window` 走目标 monitor DPI；F006 `set_open_failure(kind)`/按 kind 选文案；F008 注释修正 |
| `src/platform/hotkey.rs` | M | F001 `startup_in_two_steps_…` 机器层测试 |
| `src/platform/window_position.rs` | M | F008 `TargetMonitor` RESERVED doc |
| `src/presentation/{i18n,state,view_model}.rs` | M | F006 `SearchFailure::Open(OpenErrorKind)` + 6 个按 kind 双语 key；F007 2 个 VM 直测 |

**越界零检出：** `git diff --name-only 828819b..d507174` 不含 `src/app.rs`、`src/storage`、`src/search`、`src/domain`、`src/lib.rs`、`Cargo.toml`、`Cargo.lock`、`.github`、`ui/app-window.slint`（上述路径 `git diff --quiet ..HEAD` 逐项实测 **UNTOUCHED**）。无依赖新增、无 workflow 变更、无 `task/` 入 Git。

## Finding 逐条复审（由 reviewer 对代码重新核验）

### F001（High）— 热键在隐藏窗口创建前注册 → **CLOSED**

**复审结论：已关闭（代码可证明）。**

- **两段式启动：** `NativePlatform::start`（`src/platform/windows/mod.rs:362-405`）现在以 **pending 机器** `HotkeyMachine::new(registry, None)`（`:369`）构造（不注册），spawn worker（`:376-379`）后 `wait_for_hwnd`（`:383`，2s 预算）等到发布，再 `if let Some(combo) = hotkey_setting` → `hotkey.lock()` → `hotkey_guard.set(combo)`（`:384-396`）。**注册严格发生在 `wait_for_hwnd` 观察到的 HWND 就绪之后**；旧的“spawn 前 `new(registry, Some(setting))` 直接注册”的竞态路径结构性消除。
- **retry-once：** 首次 `set` 返回 `Err` 时 sleep 100ms 再 `set` 一次（`:389-395`），有界重试（恰好一次，非循环）。
- **失败不再静默：** 机器 `Disabled + last_error`；新增 `NativePlatform::hotkey_last_error() -> Option<HotkeyErrorKind>`（`:430-432`）与既有 `hotkey_state()` 可由 tray 侧读取。**注意（新观察，非阻断）：** 本 head 中 `hotkey_last_error()` 尚**无调用方**（grep 仅定义处与 doc 注释命中）；`main.rs` 的 tray 启动刷新仍只设 `pause_hotkeys_glyph`（`main.rs:626`），未将“快捷键不可用”文案接入 tray。F001 的**结构性竞态消除**是关闭依据；“托盘可感知 Disabled”的接线留作桌面/后续切片义务（非本 finding 阻断面，但作为新 Low 建议记录，见下 N001）。
- **发布时序契约：** worker 内先 `set_thread_state`（WndProc 上下文，事件+机器入线程局部）再写共享 slot（`mod.rs:149-157`）——主线程观察到 slot 非空时 WndProc 上下文必然已就绪；slot 只在 `CreateWindowExW` **成功**后写入（失败 `return`，不发布；随后 `wait_for_hwnd` 返回 None，`set` 经 `Unavailable` 记录）。`HWND_MESSAGE`/`PostQuitMessage` 全树零残留（grep 实测仅注释提及）。
- **`set()` 对 Disabled 行为复核：** `hotkey.rs:249-` `set` 在 `state == Disabled` 时无 unregister 前置，直接 `registry.register(combo)`；失败走 keep-old（Disabled → 无 previous → `last_error` 记录），因此「pending → set 成功 → Active」与重试路径均真实注册，非虚构。
- **新增测试（真实断言就绪-再注册与发布前 Unavailable）：
  - `startup_in_two_steps_registers_once_the_hwnd_is_ready`（`src/platform/hotkey.rs:751-775`）：pending → `Disabled`/`last_error=None`；`set(ctrl_alt_space())` → `Active`、`last_error=None`、`on_event(0x20, CTRL_ALT) == Show`；且冲突路径（`FakeRegistry::fail(Conflict)`）→ `set` 返回 `Err(Conflict)`、`Disabled + last_error(Conflict)`、非静默成功。**直接断言 ready-后注册顺序与失败可见性。**
  - `publish_hwnd_changes_the_register_target_from_unavailable_to_the_window`（`src/platform/windows/hotkey_adapter.rs:165-194`）：未发布 → `current_hwnd()==None` 且 `register(combo)==Err(Unavailable)`（按构造短路）；`publish_hwnd(HWND(0x1234))` 后 `current_hwnd()==Some` 且 `register` 进入 OS 调用路径（不再按构造 Unavailable）。**直接断言发布前 Unavailable、发布后切换注册目标。**

### F002（High）— 隐藏窗口为 message-only、`FindWindowW` 不可枚举 → **CLOSED（代码层）**

**复审结论：代码层矛盾已消除（CLOSED）；真实双实例 `FindWindowW` 枚举行为保留为桌面手工验收项。**

- **顶层窗口：** `CreateWindowExW` 的 parent 参数由 `Some(HWND_MESSAGE)` 改为 **`None`**（`src/platform/windows/mod.rs:119-134`，`:129` 注释「top-level: not HWND_MESSAGE, no owning window」）；样式为 `WS_POPUP` + `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`（`:121-124`）；**从不调用 ShowWindow**；接口注释与 `single_instance.rs` 模块顶注释一致（`single_instance.rs:1-13` 现一致地写明「hidden *top-level* window … never shown」）。
- **FindWindowW 端：** `activator.rs:40-63` `discover_primary_window` 以 `HIDDEN_WINDOW_CLASS`（`"FileGoHotkeyWindow"`，`activator.rs:21`）轮询 `FindWindowW`；工作线程 `mod.rs:317` `pub const HIDDEN_WINDOW_CLASS = "FileGoHotkeyWindow"` 与之相等。
- **新增测试：** `hidden_window_class_is_shared_with_the_activator`（`mod.rs:575-583`）断言 worker 与 activator 两常量相等且等于 `"FileGoHotkeyWindow"`——防常量漂移静默破坏激活。**但该测试仅覆盖“类名接线一致”，无法 headless 证明真正顶层窗口可被 `FindWindowW` 枚举**（FFI 运行时行为）；这正是 r01「未能自动验证项 3」的限定：**真实双实例 smoke 仍为桌面强制性验证**，代码层文档/语义矛盾已修复（无 message-only，注释与实现一致）。
- **接受 reviewer 建议方案 (a)**：响应记载与代码一致；无行为回退风险（`WM_HOTKEY`/`WM_COPYDATA` 送达该 HWND 的路径不变）。

### F003（Medium）— per-monitor DPI 物理定位未按目标 monitor 换算 → **CLOSED**

**复审结论：已关闭。**

- **调用方：** `place_window`（`src/main.rs:59-77`）先 `window.size().to_logical(window.scale_factor())` 取**逻辑**尺寸（仅作当前窗口逻辑来源，不再用当前 scale 直接放大），随后 `placement_rect_for_cursor(logical_w, logical_h)`（`:72`）一次性获得物理矩形，`window.set_position(WindowPosition::Physical(…))`（`:76`）。**不再使用 `window.scale_factor()` 放大物理尺寸。**
- **目标 monitor DPI 路径：** `placement_rect_for_cursor`（`window_placement.rs:180-198`）＝ `placement_inputs()`（`GetCursorPos` → 目标 work area，`window_placement.rs:43-60`）→ `dpi_scale_at(cursor)`（`GetDpiForMonitor` MDT_EFFECTIVE_DPI，`:106-129`）→ `physical_rect_for_monitor(cursor, work_area, dpiscale, …)`（`:153-177`：`logical × target_scale` → `window_position::compute_position` 居中/10%/夹取，非有限/非正 scale 回退 1.0，0 尺寸下限 1px）。回退路径（无 cursor/无 monitor）以主屏 work area + scale 1.0 兜底，行为确定。
- **复用/一致性：** `physical_size_for_cursor`（`:132-146`）仍存在（DDE 无关、内部仍经 `dpi_scale_at`），调用方现统一走 `placement_rect_for_cursor`，新逻辑与旧 `placement_rect`（`:23`）并存但 `place_window` 已切换。
- **3 项纯测试复核（`window_placement.rs:202-268`，head 逐字）：
  - `physical_rect_for_monitor_uses_the_target_monitor_dpi`（`:207-231`）：合成 200% 副屏（3840..7680×2160）600×140 → 1200×280 物理；`rect.x == 3840+(3840-1200)/2`（水平居中）、`rect.y == 2160/10`（顶部 10%）、不跨屏——**用目标 scale 计算物理尺寸并断言，命中 F003 根因**。
  - `physical_rect_for_monitor_clamps_with_the_target_scale`（`:235-248`）：150% 下 1920×1040 → 2880×1560 物理 → clamp 回 1920×1040 且完全位于 work area。
  - `physical_rect_for_monitor_defends_bad_scale_and_large_sizes`（`:252-268`）：NaN/0/负 → 回退 1.0（宽度 600）；0 逻辑尺寸 → 1px 下限。确定性、纯函数（合成矩形，无 FFI）。

### F004（Low）— 同一快捷键实现为「显示/focus」而非真正 toggle → **RECORDED（可接受，无代码变更）**

**复审结论：记录合理。** `LifecycleCommand::Toggle` 仅用于 tray `on_toggle_window`（`main.rs:648-652`）；hotkey / 第二实例路径恒 `Show`（`main.rs:747-757`，`HotkeyShow | ActivateFromSecondInstance → LifecycleCommand::Show`），**无隐藏 behavior 变化**。response 已按 reviewer 建议显式记录该实现决策（恒 Show / toggle 延后）、给出理由（保持“召唤即出”与第二实例恒激活语义；后续切 Toggle 仅一行改动）并登记为 M07/M08 手工确认项——满足 r01「在 response 显式记录」要求。

### F005（Low）— worker `std::process::abort()` 正常/半正常退出路径终止 → **CLOSED**

**复审结论：已关闭。** `worker_main` 返回类型改为 `()`（`mod.rs:84`），消息循环退出（WM_QUIT 或 -1）后 `DestroyWindow` 自然结束（`:172-178`），`std::process::abort()` 与 `PostQuitMessage` 已删除（全树 grep：`abort` 仅残留注释提及，代码零命中）。终态说明注释保留（进程整体退出仍由主线程主导）。`RegisterHotKey` OS 会话级清理、`InstanceMutex::Drop` 在主线程 guard 上的语义不受影响。

### F006（Low）— 打开失败未按 kind 展示可操作文案 → **CLOSED**

**复审结论：已关闭（仍匿名）。** `SearchFailure::Open(OpenErrorKind)`（`state.rs:40-44`），`as_detail` 委托 `kind.as_detail()`（`state.rs:56`）；`view_model.set_open_failure(kind)`（`view_model.rs:203`）；`main.rs:apply_open_result` 透传 `kind`（`main.rs:298-302`）；`sync_ui` 按 kind 匹配 6 个双语 key（`main.rs:355-381`，`NotFound/AccessDenied/NoAssociation/DdeFailure/ShellRejected/Unavailable` → 对应 `Msg::ErrorOpen{NotFound,AccessDenied,NoAssociation,Dde,ShellRejected,Unavailable}`），`ErrorOpenBody` 保留为通用回退。`i18n.rs` 新增 6×（enum variant + key() + ALL_KEYS + zh_cn + en_us）共 **24 处**命中（grep 实测 24），同一循环内 i18n parity 测试自动覆盖新 key 双语齐全（`i18n.rs:410-428`）。**kind 不含任何路径**：`OpenErrorKind` 为匿名 enum，`as_detail` 只产固定文案；全树路径入日志 grep 零命中。

### F007（Low）— `set_open_failure`/`clear_failure` 无直接单测 → **CLOSED**

**复审结论：已关闭。** 新增 2 项 VM 直测（`view_model.rs:975-1016`）：
- `set_open_failure_sets_open_state_and_keeps_rows_selection`：`set_open_failure(AccessDenied)` 后 `failure == Some(Open(AccessDenied))` 且 **rows/selection 不变**（选定行、选中索引均保留）。
- `clear_failure_only_clears_the_open_failure`：`clear_failure()` 清 `Open`；替换为 `SearchFailure::Search` 后 `clear_failure` **不**误清（正确保留非 Open failure）。
两项均真实执行并通过（CI lib 265 项 0 失败）。

### F008（Info）— 过时注释 / `TargetMonitor` 未接线 → **CLOSED**

**复审结论：已关闭。**
- `main.rs:223-228` 注释已改为「结果经 channel 由 UI 线程轮询 timer（`open_drain`）drain 到 `apply_open_result`」，与实现一致（`invoke_from_event_loop` 全树 grep **零命中**）。
- `window_position.rs:100-106` `TargetMonitor` 增加 RESERVED doc（说明 Cursor/Primary 两策略留待 settings-driven adapter，不新增/删除形状），代码与文档一致。

## 新增 finding（r02 新观察）

### N001（Low / Info）— `hotkey_last_error()` 当前无护栏接线，`ErrorOpenBody` 通用回退文本仍存在（报告性，非阻断）

- **证据：** `NativePlatform::hotkey_last_error`（`mod.rs:430-432`）与 `hotkey_state` 存在，但 `main.rs` tray 启动/事件路径未引用它们（grep 仅定义处命中；tray 启动刷新仅设 `pause_hotkeys_glyph`）。i18n `ErrorOpenBody` 保留为回退，`sync_ui` 的 6 个 kind 分支是全枚举（无未列举 kind 落到回退的实际路径，因 `OpenErrorKind` 已穷尽）。
- **影响：** F001 的“失败不再静默”在**机器/API 层**成立，但**托盘 UI 尚未显示“快捷键不可用”提示**。F001 已按结构性消除竞态关闭（注册时机与 Retry 已证明），不因 N001 重开；属 UX 接线缺口。
- **建议：** 在 M07 接线时，于 tray 启动刷新或 `menu_open` 时读 `native.borrow().hotkey_last_error()`/`hotkey_state()`，将 `Disabled + last_error(Conflict/Unavailable)` 映射为匿名双语提示文案；同步删除或明确保留 `ErrorOpenBody` 回退（保留无碍）。**不阻断本里程碑。**

## 跨面检查（cross-cutting）

- **正确性：** 9 个变更文件逐字核对无逻辑错误；F001 时序（pending → wait → set → retry-once）、F002 顶层窗口、F003 目标 DPI 换算全部与响应/测试一致；纯几何（compute_position 与 3 项新测试）代数复核通过（例：200% 副屏 600×140 → 1200×280，x=5160、y=216 精确成立）。
- **错误处理：** `set` 失败重试有界（一次 100ms）；`hotkey_last_error` 匿名；`physical_rect_for_monitor` 对 NaN/0/负 scale 与 0 尺寸防守；worker 返回 `()` 后 RAII/DestroyWindow 收尾；`expect("hwnd slot lock")`/`expect("hotkey lock")` 为内部 Mutex 中毒 panic（不变式内）。无新增裸错误码入 UI。
- **数据安全：** 无任何 `std::fs` 删除 API 新增；`on_open_failure` 恒 KeepWindow（无删除路径）；无 shell 执行（全树 `Command::new|powershell|std::process::Command|/c` **零命中**——唯一 shell 调用点 `tray_open.rs` `ShellExecuteExW(lpFile=path)` 不变）。
- **隐私：** 打开失败按 kind 文案仍匿名（无路径）；`eprintln!` 全为固定匿名文案（`main.rs:594,868,881`）；平台层 `println!/eprintln!` 零命中；i18n 新 key 不含占位路径。
- **安全性：** 无新 FFI 面（仅复用已有 `window_placement`/`mod.rs` FFI）；`RegisterHotKey`/`FindWindowW` 调用点在既有边界模块内，返回值均被转换/检查；IPC 固定 Show 信封不变。
- **Windows 行为 / MSVC：** MSVC CI head 全绿（265 lib 0 失败、clippy `-D warnings`、release build、EXE/版本守卫、deny/about、portable artifact）；新增测试在 MSVC 实测执行。`GetDpiForMonitor`/`GetCursorPos` 属已有 FFI 数据库路径（headless 下 `dpi_scale_is_sane…` 测试通过）。
- **测试覆盖：** 8 项新增全部反映 r01 各 finding 根因（非表面测试）：F001 时序/发布、F002 常量接线、F003 目标 DPI 数学、F006 kind 携带、F007 VM 状态机均被断言；CI 实测 264 通过 + 1 ignored。
- **确定性：** 无新 `HashMap`/`BTreeMap` 迭代序依赖（grep 零命中）；重试有界（sleep 100ms 恰好一次）；`wait_for_hwnd` 轮询 10ms 步进、2s 预算有界；`physical_rect_for_monitor` 纯函数。
- **可维护性：** 注释与实现一致（F008 已清）；FFI 仍集中 `windows/` 边界；N001 的 tray 接线留 M07（已建议）。

## 未能自动验证的桌面项（必须进入 M07/M08 手工验收清单；延续 r01）

1. **托盘真实渲染与菜单**（图标深/浅、右键 7 项、开机启动/暂停 glyph、关于、退出清 tray）。
2. **全局快捷键真实注册与触发（F001 桌面复核重点）**：默认 Ctrl+Alt+Space 可用性、快速连按、pause/resume 即时性、与其它软件冲突时 tray 能通过新暴露的 `hotkey_last_error` 提示（**N001：当前 UI 未接线，需在 M07 接线后实测**）。
3. **双实例 smoke（F002 桌面复核重点）**：启动两个 exe —— 第二实例应立即激活首实例并退出、无第二托盘、主窗口 focus。**本项验证真正顶层隐藏窗口能否被 `FindWindowW` 枚举**（代码层已消除 message-only 矛盾，但枚举为 OS 运行时行为）。
4. **窗口定位（F003 桌面复核重点）**：多显示器光标在副屏 → 居中出现在副屏、垂直 10%、任务栏四边不遮；混合 DPI 100/125/150/200% 的物理尺寸与位置。
5. **前台限制 fallback**（热键触发时窗口获得 focus；被其它全屏窗口压制时至少可见）。
6. **Explorer 重启托盘恢复**（TaskbarCreated/Explorer 崩溃恢复）。
7. **UNC/网络盘打开（非阻塞）**；无权限目录打开 → 保持窗口 + 按 kind 文案（F006）+ 重试/复制可用。
8. **IME preedit 接线、Ctrl+A/Ctrl+Backspace、失焦隐藏行为**（M03/M04 遗留，若实现则实测；未实现明确 Skip+reason）。
9. **F004 决策确认**：若产品最终要求同一快捷键 toggle（显示↔隐藏），在手工确认后把 hotkey 分支切到 `LifecycleCommand::Toggle`（第二实例保持 Show）；当前为显式 Show 记录决策。

## 需求/验收标准映射（M04.1–M04.5，r02 复审聚焦 r01 GAP 项）

| 验收点 | r01 结论 | r02 复审证据（head 代码） | r02 结论 |
|---|---|---|---|
| M04.2 注册成功后启用（启动竞态） | **GAP（F001）** | `mod.rs:362-405` pending → wait_for_hwnd → set → retry-once；`hotkey.rs:751-775` / `hotkey_adapter.rs:165-194` 测试 | **PASS**（结构消除；UI 提示接线留 N001/M07） |
| M04.2 同一快捷键 toggle 主窗口 | **PASS（实现为 Show 语义，F004 记录）** | `main.rs:747-757` hotkey/第二实例恒 `LifecycleCommand::Show`；`LifecycleCommand::Toggle` 仅 tray 用（`main.rs:648-652`） | **PASS**（显式记录决策；无行为回退） |
| M04.3 首实例接收激活并显示/focus | **GAP（F002 message-only）** | `mod.rs:119-134` 顶层 popup（parent None，非 HWND_MESSAGE）；`mod.rs:575-583` 类名一致测试；`activator.rs:40-63` FindWindowW 目标 | **PASS**（代码层矛盾消除；桌面枚举验证在 M07/M08） |
| M04.4 per-monitor DPI 换算/clamp（按目标 monitor 物理尺寸） | **GAP（F003）** | `main.rs:59-77` → `placement_rect_for_cursor`（`window_placement.rs:180-198`）→ `physical_rect_for_monitor`（`:153-177` 目标 scale）+ 3 纯测试（`:207-268`） | **PASS** |
| M04.5 打开失败可操作提示（按 kind） | **部分（F006）** | `state.rs:40-44`/`view_model.rs:203`/`main.rs:355-381` 6 kind 双语 | **PASS** |
| M04.5 权限→可理解错误、路径不进日志 | **PASS** | F006 文案匿名；`as_detail` 委托 kind；全树路径日志零命中 | **PASS** |

## 对抗性检查结论（复审）

1. **Shell-open 安全（≥High）→ 通过，无命令解释器路径。** 全树 `Command::new`/`powershell`/`std::process::Command`/`/c` **零命中**；唯一 shell 调用点 `tray_open.rs` `ShellExecuteExW(lpFile=path)` 不变；`#[allow]` 零命中（新旧代码均无）。
2. **单实例 → 代码层已修（F002 关闭），桌面验证保留。** mutex 会话级命名/IPC 固定信封不变；隐藏窗口现为顶层可枚举；仅 OS 运行时枚举需桌面 smoke。
3. **Hotkey 方法 → F001 已修（关闭）。** 注册严格在 HWND 就绪后、retry-once、失败 `Disabled + last_error` 暴露；`set()` 对 Disabled 真实 re-register。N001：托盘 UI 尚未消费 `hotkey_last_error`（非阻断）。
4. **窗口定位 → F003 已修（关闭）。** `place_window` 走目标 monitor DPI；3 项纯测试断言物理尺寸/居中/夹取/退化 scale。
5. **错误映射 + 隐私 → 通过（F006 已修）。** 按 kind 文案匿名；6 个双语 key 齐全；无路径/裸 code 入 UI。

## 最终结论

**`APPROVED_FOR_MILESTONE`**（M04 Windows 集成里程碑；**非发布批准**——依 CLAUDE.md §4.5，发布仍须用户手工验收 + 独立 release review 的 `APPROVED_FOR_RELEASE`）。

- **范围与独立性：** `828819b..d507174` 恰 9 个代码文件 + 2 个 review 文档；`src/app.rs`/`src/storage`/`src/search`/`src/domain`/`src/lib.rs`/`Cargo.{toml,lock}`/`.github`/`ui/app-window.slint` 逐项实测未变；无 scope creep。本 reviewer 未参与任何 M04 实现。
- **CI：** head `d507174`（含修复 commit `fba5f65`，`git merge-base --is-ancestor` 实测）Windows CI `35893500715`（**lib running 265 → 264 passed + 1 ignored；main 3 passed**，MSVC release EXE/版本守卫、fmt、clippy `-D warnings`、deny/about、portable artifact 全绿）与 Search benchmark `35893500620`（1 passed + 6 行 BENCH + 守卫通过）独立核验；两 run head SHA 均与 `git rev-parse HEAD` 一致。MSVC 门禁成立。
- **阻断性 finding：** **F001（High）与 F002（High）已关闭（代码可证明）**——F001 两段式启动结构性消除注册竞态（含 retry-once 与 `last_error` 暴露），F002 隐藏窗口改为真实顶层 popup（parent None）使 `FindWindowW` 可枚举、文档与实现一致并有类名一致性测试。**但对两者的真实 Windows 运行时行为（F001 热键可用性、F002 双实例实际枚举激活）仍需桌面手工 smoke——代码层已无未修复矛盾，生成环境行为以 M07/M08 桌面验收为最终确认。**
- **非阻断 finding：** F003（Medium）、F005–F008（Low/Info）均已关闭；F004（Low）记录决策成立、无行为回退。
- **新增：** **N001（Low/Info）**——`hotkey_last_error()` 尚未被 tray UI 消费（Disable 热键的托盘提示接线留 M07）；`ErrorOpenBody` 保留为回退（无碍）。非阻断，已记录并建议后续切片处理；reviewer 接受当前延期。
- **桌面义务（M07/M08 强制，延续 r01）：** 末节 9 项——托盘渲染、真实热键（含 F001 复核）、**双实例 smoke（F002 复核重点）**、混合 DPI 定位（F003 复核重点）、前台 fallback、Explorer 重启托盘、UNC/无权限打开、IME 接线、F004 toggle 决策确认。
