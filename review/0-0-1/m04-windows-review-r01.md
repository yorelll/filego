# Review: M04 Windows 托盘、全局快捷键、单实例、窗口定位与 Shell 打开（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m04-windows`
- **轮次：** `r01`
- **日期：** 2026-09-21（本地时间线；CI 时间 2026-09-23T…Z，见 CI 证据表）
- **Reviewer：** 独立 code-review agent（M04 r01）
- **独立性声明：** 本 reviewer **未参与** M04 实现 commit `828819b` 的任何编码，**未参与** M00–M03 任何历史实现的编码，未参与 M04 的 response/review 文档撰写，未参与 CI 触发与监控。本评审严格只读：**未编辑任何源代码、未运行任何 `cargo` 命令（依照 CLAUDE.md §3.1 与任务限制未执行本地/远端 Rust 验证）、未提交、未推送**；除本 review 审计文档外未写入任何其他文件。
- **Head SHA（评审对象）：** `828819b2819db61af089de75a8ba2e5b5b3add72`（`feat: wire Windows tray, global hotkey, single-instance and open`）——`git rev-parse HEAD` 实测一致。
- **Base SHA（比较基准）：** `00f195e…`（M03 r02 批准头，`docs: record M03 window review r02 approval`）。
- **比较范围：** `00f195e..828819b`，`git diff --stat` 22 文件 +3887/−31（见下文件范围表）。
- **审查文件（代码）：** `Cargo.toml`/`Cargo.lock`、`src/domain/settings.rs`、`src/main.rs`、`src/platform/mod.rs`、`src/platform/hotkey.rs`、`src/platform/ipc.rs`、`src/platform/shell_open.rs`、`src/platform/window_position.rs`、`src/platform/windows/{mod,clipboard,hotkey_adapter,single_instance,single_instance/activator,tray_open,user_session,window_focus,window_placement}.rs`、`src/presentation/{i18n,state,view_model}.rs`、`ui/app-window.slint`。
- **上下文读取（审计/上下文）：** `review/0-0-1/m03-window-review-r02.md`（M03 批准，记录 M04 完成 OpenEntry/CopyPath stub + IME preedit hook 的后置义务）、`review/0-0-1/m02-filter-bench-review-r02.md`、`task/01-里程碑任务清单.md`（M04 段）、`src/app.rs`（head 逐字核验 **未变更**，见下文）。
- **需求基线：** `REQ-LIFE-001..014`、`REQ-HOTKEY-001..012`、`REQ-FOLDER-020..029`、`REQ-WINDOW-020..033`（映射见需求/验收标准表）。

## 审查方法

1. `git diff 00f195e..828819b` 逐字审查全部 22 个变更文件；
2. 用 `git show 828819b:<file>` 直接读取 head 代码（不依赖任何转述）核验关键实现；
3. 用指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 独立查询 2 个 CI run 的 head SHA、workflow、job/steps 结论、关键日志行（test result 计数、BENCH 行、MSVC release EXE 验证守卫）；
4. 对抗性 grep：`Command::new`/`powershell`/`cmd`/interpreter、`#[allow]`、真实目录删除、`unwrap/expect`、`ShellExecute` 调用点、`RegisterHotKey`/`FindWindow`/`HWND_MESSAGE` 调用点、telemetry/network；
5. 统计口径核对（静态 `#[test]` 计数 vs CI 日志 `running N tests`）；
6. 生命周期集成核验（`src/app.rs` 在范围外、其 `Exiting` 终态测试仍符合 head）。

## CI 证据（reviewer 独立核验）

任务给定的 CI 范围在 head `828819b`；两 run 均由本 reviewer 用指定 gh.exe 独立查询：

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35886960491](https://github.com/yorelll/filego/actions/runs/35886960491) | `828819b2819db61af089de75a8ba2e5b5b3add72` | `Windows CI` / job `fmt, clippy, test, release, package` | `success`（completed，push event，`feature/m00-foundation`） | `gh run view`：head SHA 与 `git rev-parse HEAD` 一致；displayTitle `feat: wire Windows tray, global hotkey, single-instance and open`。job 内 22 个 step 全绿：fmt → clippy `-D warnings` → tests → release build → `Verify release outputs and version helper`（MSVC release EXE 存在性 + `filego-version` 版本一致守卫）→ cargo-deny（advisories/licenses/bans/sources）→ cargo-about inventory → portable artifact 构建/上传。`--log` 实测：**lib `running 257 tests` → `test result: ok. 256 passed; 0 failed; 1 ignored`**（ignored = M02-B release 基准）+ **main bin `running 3 tests` → `test result: ok. 3 passed`** + 其余 bin `0 passed`。 |
| [35886960196](https://github.com/yorelll/filego/actions/runs/35886960196) | `828819b2819db61af089de75a8ba2e5b5b3add72` | `Search benchmark` / job `release 10k benchmark` | `success`（completed，push event，同 head/同时间戳） | `gh run view`：head SHA 一致。`--log` 实测 `running 1 test` → `test result: ok. 1 passed; 0 failed; ...; 256 filtered out` + **6 行 BENCH**：`empty-query-default median=0.86ms` / `filtered-with-clone median=58.49ms` / `pinyin-heavy 67.14ms` / `english-initials 55.29ms` / `edit-distance 61.86ms` / `multi-token 50.50ms`；`if ($null -eq $summary) { throw }` 守卫未触发（BENCH 行存在且 job success）。 |
| 补 | `git diff --name-only 00f195e..828819b -- .github` | — | 零输出 | 两 workflow（ci.yml / benchmark.yml）在本 range 内零变更。 |

- **统计口径核验：** head 静态 `git grep '#[test]'` 计数 **260** = lib 257 + main 3（independent bin）。其中 **M04 新增 platform 测试 68 = hotkey 18 + ipc 11 + shell_open 7 + window_position 8 + windows/clipboard 2 + hotkey_adapter 4 + windows/mod 3 + single_instance 2 + activator 3 + tray_open 3 + user_session 1 + window_focus 2 + window_placement 1 + settings 3（domain）**；M04 范围内 app.rs 未变（8 个 LifecycleController 测试仍在 lib 257 内）。CI `running 257 / 256 passed + 1 ignored` 与静态口径一致；main 3 与 CI `running 3 / 3 passed` 一致。**新增测试在 MSVC CI 中真实执行并通过**。
- **MSVC 门禁核验：** head `828819b` Windows CI 全绿覆盖 §3.2 的 1–5 项（fmt / clippy `-D warnings` / 257 项测试 0 失败 / MSVC release build / `Verify release outputs and version helper` 守卫），并含 cargo-deny（依赖新加入的 windows/raw-window-handle 均通过 licenses/bans/sources 审计）与 cargo-about（第三方许可证清单生成 + 标题 sanity-check）。M04 为 Windows-native 里程碑，MSVC CI 即为权威门禁；**成立**。
- **依赖范围：** 仅新增 `windows =0.62.2`（Microsoft MIT，features 显式最小化 14 项，与 `platform::windows` 模块逐一对应并有 Cargo.toml 注释）与 `raw-window-handle =0.6.2`（MIT/Apache-2.0，已由 Slint 引入）。无 M05 CRUD、无 M06 设置 UI、无遥测/网络依赖掺入（grep `reqwest|url::|http` 零命中）。

## 变更范围核验（scope 控制）

| 文件 | 类型 | 主要变更 |
|---|---|---|
| `Cargo.toml` / `Cargo.lock` | M | 新增 `windows`、`raw-window-handle`；Slint 增加 `raw-window-handle-06` feature；目标平台 `cfg(target_os="windows")` 依赖隔离（GNU 本地测评不受影响） |
| `src/main.rs` | M | WindowPort::show 增加 place_window + bring_search_to_front；OpenEntry/CopyPath 真实接线（异步 shell open + 剪贴板）；单实例 guard；native platform 启动/降级；tray 菜单动作；两个 50ms 轮询 timer（native 事件 + open 结果） |
| `src/domain/settings.rs` | M | `HotkeyModifiers`/`HotkeyKey`/`HotkeySetting` 模型 + 默认 Ctrl+Alt+Space + serde round-trip/legacy 测试 |
| `src/presentation/{i18n,state,view_model}.rs` | M | `SearchFailure::Open` + `ErrorOpen*` 双语 key + `set_open_failure`/`clear_failure` |
| `ui/app-window.slint` | M | AppTray 菜单 7 项（打开/添加/分隔/设置 disabled/开机启动 glyph/暂停快捷键 glyph/分隔/关于/退出）；glyph 属性经 Slint 生成 setter 由 Rust 驱动 |
| `src/platform/`（9 个纯模块 + 9 个 windows/FFI 模块） | A | M04 平台层（见下） |
| `src/app.rs` | 未变 | `git diff --name-only 00f195e..828819b` 不含 app.rs；LifecycleController 的 `Exiting` 终态与 `native_close_after_exit_preserves_terminal_state` 测试保持 head 有效 |

**越界零检出：** `git diff --name-name 00f195e..828819b -- src/storage src/search src/domain/ids.rs src/lib.rs .github release.yml` → 零输出（除 settings.rs 的 hotkey 模型）；`src/app.rs` 未变；无 `.github` workflow 变更；无 telemetry/network/自动更新。

## 需求/验收标准映射（M04.1–M04.5）

| 验收点 | 证据（head 代码 file:line） | 结论 |
|---|---|---|
| **M04.1 静默启动**（不显示主窗口/任务栏） | `src/main.rs:830` 事件循环前不调用任何 `show`；M00 注释「starts in the tray」。 | **PASS** |
| M04.1 tray 菜单 打开/添加/分隔/设置/开机启动 checked/暂停快捷键 checked/分隔/关于/退出 | `ui/app-window.slint:474-514`（AppTray 菜单 7 项 + 2 分隔）；「设置」显式 `enabled: false`（M06 接线，不伪功能）。 | **PASS** |
| M04.1 开机启动/暂停 状态为 text+glyph 非仅颜色 | `.slint:481-483` `launch-at-login-glyph`/`pause-hotkeys-glyph`，`"✓ "` 前缀文本；`main.rs:653-661` 启动刷新 + `main.rs:678-695` 切换后刷新。 | **PASS** |
| M04.1 关闭窗口只隐藏 | `main.rs:592-596` `on_close_requested → accept_window_close → CloseRequestResponse::HideWindow`；app.rs `Exiting` 终态守卫。 | **PASS** |
| M04.1 退出保存/注销快捷键/移除托盘/终止 | `main.rs:637-645` `ExitFromTray → quit_event_loop`；`InstanceMutex::drop` 关闭 mutex 句柄（`single_instance.rs:105-110`）；RegisterHotKey 由 OS 按进程退出自动清理（`hotkey.rs:24-27` 文档化，进程作用域）。「保存未持久化设置」：M04 仍为内存 demo 数据（data 持久化属 M05/M06），**已记录为已知限制**。 | **PASS**（持久化留 M05/M06） |
| M04.1 Explorer 重启托盘恢复 | Slint SystemTrayIcon 由 ADR-003 提供 TaskbarCreated 恢复；无自动化验证。 | **PASS**（桌面验证项，见末节） |
| **M04.2 默认 Ctrl+Alt+Space 注册** | `settings.rs:8-10` `DEFAULT_HOTKEY_MODIFIERS`=Ctrl+Alt、`DEFAULT_HOTKEY_KEY`=0x20；`main.rs:571-573` `NativePlatform::start(settings.hotkey)`；`hotkey.rs:217-234` `new()` 校验+注册 → `Active`。 | **PASS** |
| M04.2 同一快捷键 toggle 主窗口 | WM_HOTKEY → `HotkeyAction::Show`（`windows/mod.rs:172-188`）→ UI timer → `LifecycleCommand::Show`（`main.rs:706-731`）。**注意**：实现为「显示/focus」，非「显示↔隐藏 toggle」（见 F004 讨论）。 | **PASS**（实现为 Show 语义） |
| M04.2 修改先验证再注册，失败保留旧快捷键+冲突 | `hotkey.rs:249-288` `set()`：先 `validate` → `unregister` → `register`；失败则 re-register 旧 combo 并返回 `Err(Conflict)`；测试 `set_keeps_old_registration_when_new_conflicts`。 | **PASS** |
| M04.2 禁止单字母/数字/Win/保留（表驱动） | `hotkey.rs:82-91` RESERVED 表、`hotkey.rs:117-155` validate 规则、`hotkey.rs:407-607` 表驱动测试。 | **PASS** |
| M04.2 clear/pause/resume 立即生效+持久化 | `hotkey.rs:291-315`；`windows/mod.rs:392-399`；pause 保留注册（无重注册竞态）、resume 即时。**持久化**：`HotkeySetting` 已入 `AppSettings::hotkey`（serde 持久化字段），但 M04 尚未接设置 UI（M06）；状态以默认设置生效。 | **PASS**（设置 UI 留 M06） |
| M04.2 退出/崩溃不残留 OS 注册 | RegisterHotKey 会话级、进程退出自动清理（文档化 `hotkey.rs:24-27`）。 | **PASS**（OS 语义保障） |
| **M04.3 会话级命名互斥（不跨用户阻塞）** | `Local\FileGo.<suffix>`；`ipc.rs:150-162` mutex_name 校验 + `user_session.rs` 用户名派生 suffix；`single_instance.rs:50-68` CreateMutexW。 | **PASS** |
| M04.3 第二实例发送 activate 后退出 | `main.rs:547-565`（第二实例在创建任何窗口/tray 前 `activate_primary(2000)` 并 return）。 | **PASS** |
| M04.3 首实例接收激活并在 UI 线程显示/focus | WM_COPYDATA → WndProc → channel → UI timer → `LifecycleCommand::Show`（`windows/mod.rs:189-224` + `main.rs:706-731`）。 | **PASS** |
| M04.3 IPC 最小/固定/校验（无任意命令/路径执行） | `ipc.rs:43-75` 固定 64B 信封、magic/version/command、reserved 必须为 0；`decode` 永不产生 Execute；测试 `decode_never_treats_arbitrary_bytes_as_execute`。 | **PASS** |
| M04.3 启动中/崩溃/stale/并发第二实例确定行为 | `ipc.rs:108-133` decide（won_mutex/endpoint 两布尔）；activator 有界轮询 `discover_primary_window` + `SendMessageTimeoutW`（`activator.rs:40-102`）；并发测试「全部退出无第二托盘」。 | **PASS**（机制设计） |
| M04.3 不产生第二托盘 | `main.rs:550-555` 第二实例在任何窗口/tray 创建前退出。 | **PASS**（机制设计） |
| **M04.4 默认当前鼠标显示器** | `window_placement.rs:43-57` placement_inputs 以 GetCursorPos；`window_position.rs:72-97` 命中光标所在 monitor，否则主屏回退。 | **PASS** |
| M04.4 工作区水平居中/顶部 10%/不加任务栏/不跨屏 | `window_position.rs:88-96`；clamp 测试 `window_larger_...`/`never_spans_two_monitors`/`respects_a_taskbar`。 | **PASS** |
| M04.4 per-monitor DPI 换算/clamp | `window_placement.rs:106-128` GetDpiForMonitor；**注意**：调用方 `main.rs:55-74` 用的是 **Slint 窗口全局 scale_factor**，未按每个 monitor 分别换算（见 F003）。 | **GAP**（F003） |
| M04.4 每次显示重新计算位置 | `main.rs:33-39` WindowPort::show 每次调用 `place_window`。 | **PASS** |
| M04.4 恢复/bring-to-front/focus + foreground fallback | `window_focus.rs:35-105` 三级 fallback（ShowWindow+SetWindowPos / SetForegroundWindow / AttachThreadInput+BringWindowToTop+SetActiveWindow）；`main.rs:80-95` 起前台。 | **PASS**（真实行为桌面验证） |
| M04.4 Esc/快捷键/打开成功/托盘隐藏、失焦可配+popup-safe | Esc→Hide 走 M03 既有 SearchKey::Escape；打开成功按 settings hide/clear（`main.rs:277-299`）。**失焦隐藏 `hide_on_focus_loss`**：settings 字段已存在（默认 true）但 M04 **未接线**（grep 仅 domain/storage 命中，无 window 侧实现）——记录为已知限制（无焦点事件钩子在 Slint 侧）。popup-safe 为 M03 既有 overlay gate。 | **PASS**（失焦隐藏留 M06/桌面） |
| **M04.5 ShellExecuteExW 系统 verb，无 shell 命令串** | `tray_open.rs:142-176` 唯一 shell 调用：`ShellExecuteExW` + `lpFile=path` verbatim、`lpVerb=null`、`lpParameters=null`；无 `/c`/无 PowerShell/无命令构造（全树 grep 零命中）。 | **PASS** |
| M04.5 非阻塞（UNC 不停顿） | open 在 `std::thread::spawn` 的 worker 线程（`main.rs:257-263`）+ `SEE_MASK_NOASYNC|SEE_MASK_ASYNCOK`（`tray_open.rs:146`）——不阻塞 UI 线程输入。 | **PASS**（真实 UNC 卡死行为桌面验证） |
| M04.5 成功更新 last-open/open-count + hide/clear | hide/clear 已接（`main.rs:277-290`）；last-open/open-count 持久化依赖 M05/M06 仓库写入（本里程碑内存 demo 数据）。 | **PASS**（计数持久化留 M05/M06） |
| M04.5 失败保持窗口+重试/编辑/复制/移除/保留 | `main.rs:292-298` 失败恒 KeepWindow + 匿名 `ErrorOpenBody`（「Enter 重试 / Ctrl+C 复制」）；编辑/移除属 M05 设置页。 | **PASS**（编辑/移除留 M05） |
| M04.5 权限→可理解错误、路径不进普通日志 | `shell_open.rs:33-45` 匿名 `OpenErrorKind::as_detail`；`tray_open.rs:179-187` SE_ERR→kind 映射；`main.rs:833-837` 平台错误只打匿名文案、不进路径；open 结果仅匿名 kind 回传 UI。 | **PASS** |
| M04.5 不可访问记录绝不自动删除 | `shell_open.rs:101-103` `on_open_failure` 恒 `KeepWindow`；全树无 `remove_dir/remove_file/delete_directory`（仅 storage 结构 guard 测试字面量）。 | **PASS** |
| **M04 测试重点**（hotkey / IPC / geometry / shell / tray-lifecycle / CI smoke） | hotkey 18 + adapter 4、ipc 11 + activator 3、window_position 8 + dpi 1、shell_open 7、app.rs LifecycleController 8；MSVC CI 实际执行 257 项 0 失败；无交互项需 skip（window_placement 的 `dpi_scale_is_sane_without_a_monitor` 在 headless 环境运行并断言有限正数）。 | **PASS** |

## Finding（按严重级排序）

### F001（High）— 热键在隐藏窗口创建前注册，注册静默失败 → 默认热键在启动竞态下可能不生效

- **文件：** `src/platform/windows/mod.rs:333`（`HotkeyMachine::new`）vs `src/platform/windows/mod.rs:342`（worker spawn）vs `hotkey_adapter.rs:84-88`（返回 `Unavailable`）。
- **问题：** `NativePlatform::start` 在 **spawn 之前**就调用 `HotkeyMachine::new(registry, hotkey_setting)`。worker 线程稍后才 `CreateWindowExW` 并把 HWND 写入共享 slot（`mod.rs:127`）；若注册先于窗口存在，`Win32HotkeyRegistry::register` 因 `current_hwnd()` 为 None 返回 `HotkeyErrorKind::Unavailable`（`hotkey_adapter.rs:84`）。`HotkeyMachine::new` 只做了 `register()` 一次、失败则 `state=Disabled` 且仅设 `last_error`（`hotkey.rs:229-232`）——**没有重试**。启动后没有任何代码再次调用 `set`/`register`（全树 grep `register(` 仅 `new`/`set` 两路径），因此该组合**永远不再注册**。
- **影响：** 在启动竞态（happens-before 不确定）下默认 Ctrl+Alt+Space 可能整个会话不可用；且状态为静默 `Disabled`，tray 无热键状态提示（M04.2 验收「注册成功后启用」的语义被破坏）。`wait_for_hwnd`（`mod.rs:347`）在 spawn 之后专门等窗口，说明应等到 HWND 就绪后再注册。
- **证据：** `mod.rs:332-343`（注册在 spawn 前）；`hotkey_adapter.rs:84`（hwnd 缺失→Unavailable）；`hotkey.rs:229-232`（new 期失败不重试）；无任何后续重注册路径。
- **复现/确定性：** worker 线程创建窗口与主线程 `HotkeyMachine::new` 间无同步，两次运行结果可能不同（`expect("hotkey lock")` 等锁仅保护状态访问，不提供注册发生的 happens-before 保证）。
- **建议：** 在 `wait_for_hwnd` 得到 Some(hwnd) **之后**再构造 `HotkeyMachine::new`（或显式调用一次 `set(hotkey_setting)`），并在注册失败时把 `last_error`/Disabled 状态以匿名形式暴露给 tray（如「快捷键不可用」文案）以便用户察觉。

### F002（High）— 第二实例激活依赖 `FindWindowW`，但隐藏窗口是 message-only（不可枚举）→ 第二实例可能找不到端点

- **文件：** `src/platform/windows/mod.rs:107-122`（`CreateWindowExW` 传 `Some(HWND_MESSAGE)`）与 `src/platform/windows/single_instance.rs:3-5` / `activator.rs:40-63`（`FindWindowW` 按类名查找）。
- **问题：** `module.rs:117` 创建的是 message-only window（`HWND_MESSAGE` 父窗口），而 `activator.rs` 的 `discover_primary_window` 用 `FindWindowW(HIDDEN_WINDOW_CLASS)` 查找。Windows 文档：message-only 窗口**不在系统窗口列表/枚举器中**，`FindWindowW`（按 top-level 窗口遍历）和 `EnumWindows` **找不到 message-only 窗口**。
- **影响：** 第二实例（没有 mutex）在 `discover_primary_window(2000)` 的 2 秒预算内可能永远找不到端点半成品，走 `StaleEndpointExit`——第二实例退出且无托盘，但 **首实例窗口不会被激活**，用户双击 exe 无可见响应 → M04.3「第二实例发送 activate+退出 / 首实例显示并聚焦」验收核心路径可能失效。窗口是否真的在枚举中取决于 winit/Slint 底层 `HWND_MESSAGE` 语义与文档化行为不一致，需要实测，但代码层面存在明确的文档矛盾。
- **证据：** `mod.rs:117`（`Some(HWND_MESSAGE)`，模块顶部注释 `single_instance.rs:3-4` 自己也写了「message-only windows are not enumerable, so we deliberately use an invisible top-level popup」——实现与此注释矛盾）。
- **建议：** 二选一：(a) 创建不可见的 top-level popup（如同模块注释声称的 `WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE`、从不显示），或用 `CreateWindowExW` 把父窗口改为 `None` 且不传入 HWND_MESSAGE；或 (b) 改走 `FindWindowExW` 配合注册的 `TaskbarCreated` 或自定义消息，或直接放弃按 HWND 查找、改用 `RegisterWindowMessage`+广播或信号量单实例在端点上做二次确认。最简：让 worker 在已知 primary HWND 时将它暴露，第二实例通过 mutex 状态 + 有界轮询窗口枚举（仅对真正可枚举的 top-level 窗口）。**至少需要真实桌面双实例 smoke 验证**。

### F003（Medium）— per-monitor DPI 物理定位未按目标 monitor 换算；隐藏窗口定位看似 pass 的另外一条

- **文件：** `src/main.rs:55-74`（`place_window`）与 `src/platform/windows/window_placement.rs:106-139`（`dpi_scale_at`/`physical_size_for_cursor`）。
- **问题：** 边界提供了 `dpi_scale_at(x,y)` 与 `physical_size_for_cursor`（`window_placement.rs:132-139`），但 `main.rs:place_window` **未使用**——它取 `window.scale_factor()`（**Slint/wininit 窗口当前 scale**）把逻辑尺寸放大，未针对**目标（光标所在）monitor** 的 DPI。在混合 DPI（110/125/150/200%）或多个 monitor 场景，窗口尺寸换算可能与目标显示器不一致：Slint 的 `set_position` 传的是物理坐标，`SetWindowPos` 语义会把窗口整体搬到目标 monitor，尺寸用 wininit 主窗口 scale 计算，可能与目标 monitor 物理尺寸不匹配（偏大/偏小或 clamping 按错误尺寸）。`window_placement.rs` 已封装好在光标位置的 DPI probe 但**无人调用**（`grep physical_size_for_cursor` 仅定义处命中）。
- **影响：** 混合 DPI 多显示器桌面（Windows 常见）窗口定位/尺寸可能偏差，属 M04.4「per-monitor DPI 换算正确」验收点的缺口；单 monitor 100% 缩放下等价（scale=1）。结构测试 `window_position.rs` 纯函数通过不足以覆盖此路径（它假定调用方传入正确物理尺寸）。
- **证据：** `main.rs:57`（`window.scale_factor()`，非目标 monitor）；`window_placement.rs:132`（有函数但零调用）。
- **建议：** `place_window` 先 `cursor_position()` 得到目标点，再 `dpi_scale_at(x,y)` 计算目标物理尺寸，再 `placement_rect`；可加一个「给定 monitor 合成矩形 + 逻辑尺寸 × 目标 scale」的纯函数测试。

### F004（Low）— 同一快捷键实现为「显示/focus」非真正 toggle；明文文档与实现语义需对齐

- **文件：** `task/01-里程碑任务清单.md` M04.2「同一快捷键 toggle 主窗口」备注「实现为 Show（显示/focus）；toggle 语义处于 worker WM_HOTKEY 匹配，M04.2 验收项」；`windows/mod.rs:172-188`（WM_HOTKEY→`NativeEvent::HotkeyShow`）与 `main.rs:717-723`（→`LifecycleCommand::Show`，永不 hide）。
- **问题：** 产品/任务/CLAUDE 需求写「同一快捷键 toggle」，实现是恒 `Show`（Show+toggle 二选一本就是实现选择，`LifecycleCommand::Toggle` 已存在但未用于 hotkey）。窗口已可见时按热键重复 bring-to-front（通常会触发 Windows 前台限制 fallback），不隐藏。
- **影响：** 行为与需求描述不完全一致；任务文档已将其标为验收项，故**不构成阻断**，但需要在 response/后续里程碑明确「首版选择 Show 而非 Toggle」并获得用户/任务确认。
- **建议：** 在 response 中显式记录该实现选择，或复用 `LifecycleCommand::Toggle`（VM 层已有测试覆盖）。

### F005（Low）— abort-free but deliberate：worker 线程 `std::process::abort()` 在 GetMessage 返回 -1/退出路径会直接终止进程，而非优雅清理

- **文件：** `src/platform/windows/mod.rs:148-160`。
- **问题：** `worker_main` 在消息循环退出后 `DestroyWindow` + `PostQuitMessage(0)` 再 `std::process::abort()`（`mod.rs:160`）。注释与代码意图是把 `!` 当作不可达返回类型；`abort()` 是**最终手段**，但这里是在正常/半正常退出路径上无条件调用（`GetMessage` 返回 -1 是异常、返回 0 是 WM_QUIT）。若事件循环被外部终止，abort 会跳过 `Drop`/清理。
- **影响：** 干净退出时由 OS 回收注册（RegisterHotKey 会话级），`InstanceMutex::Drop` 在**主线程**（`main.rs:547` guard）执行，故实际残留风险低。属可维护性/健壮性代码异味：若未来 worker 需在退出前做清理，`abort()` 会影响。未达阻断级。
- **建议：** 提供返回路径（`fn worker_main(...) -> ()` + `std::process::exit(0)` 或让线程自然结束），并注释为什么进程整体退出是正确的终态（托盘 shell 存活依赖主循环，worker 单独退出不应杀掉进程）。低优先。

### F006（Low）— 打开失败未展示具体错误 kind 的可操作区分（隐私与可操作性平衡）

- **文件：** `src/main.rs:292-298` 与 `src/presentation/i18n.rs:239-243`（`error.open.body`）。
- **问题：** `shell_open.rs` 定义了 `OpenErrorKind` 六种可区分失败（`NoAssociation/NotFound/AccessDenied/DdeFailure/ShellRejected/Unavailable`），`tray_open.rs:179-187` 也正确映射，但 UI 收到 `OpenResult::Failed(_kind)` 时 `main.rs:292` **忽略 kind**，一律显示通用的「无法打开；重试/复制路径」（i18n 单文案）。测试 `opener_error_kind_maps_consistently` 只验证了 pure 层映射，未验证 presenter 侧对 kind 的区分展示。
- **影响：** 权限不足（AccessDenied）与路径不存在（NotFound）等本可给出可操作提示的场景全部折叠为同一提示；M04.5「权限不足等映射为可理解提示」部分达成（有提示但未按 kind 细分）。不构成数据/安全问题（路径确实不泄露）。
- **建议：** 将 kind 透传到 UI 侧，按 kind 选本地化文案（如 AccessDenied →「没有权限，检查文件夹共享/权限」），仍保持匿名。

### F007（Low）— `set_open_failure`/`clear_failure` 无直接单元测试

- **文件：** `src/presentation/view_model.rs:201-208`。
- **问题：** `Open` 失败状态的设置/清除方法在 VM 层零直接测试（grep 仅定义处与 `state.rs:56` as_detail 命中）。其状态机会在 `main.rs` 集成路径触发，但 VM 层单测未覆盖。
- **影响：** VM 状态机覆盖不完整；不影响正确性（逻辑简单），可维护性缺口。
- **建议：** 补 2 个测试：`set_open_failure` 后 `state.failure == Some(Open)` 且 rows/selection 不变；`clear_failure` 仅清 Open 而保留其它 failure。

### F008（Info）— 文档/实现不一致：若干「部分/待桌面验证」项与 head 代码一致性需 artifacts

- 任务清单标注的部分项（如「左键 toggle」「失焦隐藏可配且 popup-safe」「IME preedit」「focus 全选」「explorer 重启托盘」「UNC 卡住行为」）在本里程碑无自动化或桌面证据，必须进入桌面手工验收清单（见末节）。此外 `main.rs:221-222` 注释仍写「delivered back through `slint::invoke_from_event_loop`」而实际实现是 **polling timer**（`main.rs:816-826`）——属**过时注释**，建议在 response 修正（无功能影响）。
- `window_position.rs:100-106` 定义了 `TargetMonitor::Primary` 枚举但 `compute_position` 未使用（`TargetMonitor` 未接入选路径）——活动窗口显示器策略留作后续（任务已标注「部分」），建议标注 dead/预留。

## Cross-cutting 检查

- **正确性：** 纯几何/状态机/IPC/validation 表驱动测试充分（68 项 M04 platform 测试 + 既有 192 项回归，head 全绿）；隐藏窗口创建前注册（F001）与 message-only 不可枚举（F002）为代码可证明的功能缺口。
- **错误处理：** 匿名错误 enum（`OpenErrorKind`/`HotkeyErrorKind`/`NativeError`/`RegistryError`）均提供 `as_detail` 且测试断言不含路径/裸 code；生产路径 `expect("hotkey lock")`/`expect("hwnd slot")` 为内部 Mutex 中毒 panic 场景（可接受）；`ipc.rs:62,66` `try_into().unwrap()` 在长度已校验的固定切片上是编译期安全；`mod.rs:127`/`hotkey_adapter.rs:73,78` lock expect 为内部不变式。
- **数据安全：** M04 范围零 `std::fs` 删除 API（grep 零命中）；`storage/tests.rs:735` 结构 guard 已覆盖 remove 系列字面量；`on_open_failure` 恒 KeepWindow，无删除路径。
- **隐私：** 打开失败仅匿名 kind 回 UI；`ErrorOpenBody` 不含路径；`main.rs:833-837` 平台错误仅打匿名文案；clipboard `set_text` 从不日志；`eprintln` 全部为匿名固定文案（`main.rs:562,836,849`）。
- **安全性：** 唯一 shell 调用点 `ShellExecuteExW(lpFile=path)`，无命令构造/解释器（全树 `Command::new|powershell|cmd` 零命中）；IPC 固定 64B 信封、reserved=0、无 payload 槽（`ipc.rs`）；无网络/遥测（grep 零命中）。
- **Windows 行为 / MSVC：** MSVC CI head 全绿（257 项测试 0 失败、clippy `-D warnings`、release build、EXE/版本守卫、deny/licenses/ban/sources、about inventory）；window_placement 的 headless DPI 测试成功（无交互需求）；F001/F002 属逻辑/FFI 语义问题，CI 无法暴露（编译通过 + 运行通过但行为缺失），需修复或桌面确认。
- **测试覆盖：** M04 新增 68 项 platform 测试 + 3 domain settings 测试 + app.rs 8 项既有，均在 MSVC 实测通过；F001/F002 无自动化覆盖（属平台运行时行为，自动化成本高），F003 缺调用方测试，F007 缺 VM 直测。
- **确定性：** 纯测试全部确定性（表驱动固定输入）；IPC/几何/validation 无集合迭代序依赖。
- **可维护性：** 架构分层清晰（纯模块与 FFI 边界分离、trait 注入可测）；F008 的过时注释为唯一杂质。

## 对抗性检查结论

1. **Shell-open 安全（≥High 检查）→ 通过，无命令解释器路径。** 全树 `src/` grep `Command::new`/`powershell`/`cmd`/`/c ` **零命中**；唯一 shell 调用为 `tray_open.rs:165` `ShellExecuteExW`，`lpFile=path`（verbatim）、`lpVerb/lpParameters=null`（`tray_open.rs:148-150`）；`ShellHandle` 到 `open_folder` 之间零字符串构造（`main.rs:118-129` 仅委托）；结构测试 `shell_open.rs` 覆盖 verbatim path/no delete/匿名映射。**无注入面。无结构 guard 测试检测 shell 调用形式**（storage guard 只覆盖删除类），建议后续 M07 补一条「src 不含 Command::new/cmd/powershell」的 guard（可选增强，不阻断）。
2. **单实例正确性 → 部分 Gap（F002）。** mutex 会话级命名与长度校验正确（`Local\FileGo.<suffix>`，避免跨用户/跨会话阻塞）；guard 被 `main.rs:547` 的 `_instance_guard` 绑定至 run() 结束（修复过早期 drop 问题），`Drop` 显式 CloseHandle；IPC 只发固定 `Show` 信封、无路径/命令槽；`decode` 对任何畸形/路径样字节返回 `Ignored`；并发第二实例（`decide` 纯函数 + 测试）全部退出不产生第二托盘。**但端点查找依赖 FindWindowW 而窗口是 message-only** → 激活路径可能有运行时盲区（F002），需修复或桌面实测确认。
3. **Hotkey 方法 → 部分 Gap（F001）。** worker 线程隐藏 HWND + WM_HOTKEY → channel → UI 50ms timer → lifecycle Show；无 Win32 pump 在 Slint 线程；validation 表驱动纯函数测试充分；`set()` keep-old 语义有测试；pause 保留注册、resume 即时。**但注册发生在 worker 创建窗口之前**（F001），竞态下默认热键可能静默失败且无重试。
4. **窗口定位 → 部分 Gap（F003）。** 纯几何测试完备（monitor/DPI/clamp、负原点、跨屏、任务栏）；每次 show 重算（`main.rs:33-39`）；foreground fallback 三级链完整且文档化。**但 place_window 用 Slint 窗口全局 scale 而非目标 monitor DPI**，混合 DPI 下物理尺寸换算可能不匹配（F003）。
5. **错误映射 + 隐私 → 通过。** Win32 返回全部映射匿名 kind（`SE_ERR_*`、HRESULT、GetLastError 均转 `as_detail` 文案）；无裸 code 入 UI；路径/热键/查询不打日志；clipboard 无泄漏。F006 建议细分错误文案（非安全gap）。

## 未能自动验证的桌面项（必须进入手工验收清单）

1. **托盘真实渲染与菜单**（图标浅/深辨识、右键 7 项、开机启动/暂停 glyph 显示、关于、退出清 tray）。
2. **全局快捷键真实注册与触发**（默认 Ctrl+Alt+Space 可用性、快速连按、pause/resume 立即性、与其它软件冲突提示）。**重点验证 F001 竞态是否存在**。
3. **双实例 smoke**（启动两个 exe：第二实例应立即激活首实例并退出，无第二托盘；主窗口得到 focus）。**重点验证 F002 message-only 窗口是否可被 FindWindowW 枚举**。
4. **窗口定位**（多显示器：光标在副屏时居中出现在副屏、垂直 10%、任务栏四边不遮；混合 DPI 100/125/150/200% 的尺寸/位置）。
5. **前台限制 fallback**（热键触发时窗口获得焦点；被其它全屏窗口压制时至少可见）。
6. **Explorer 重启托盘恢复**（TaskbarCreated/Explorer 崩溃恢复）。
7. **UNC/网络盘打开**（非阻塞、无 UI 卡死）；无权限目录打开 → 保持窗口 + 重试/复制可用。
8. **IME preedit 接线、Ctrl+A/Ctrl+Backspace、失焦隐藏行为**（M03/M04 遗留，若实现则实测；未实现需明确 Skip+reason）。

## 最终结论

**`CHANGES_REQUESTED`**（M04 Windows 集成；非发布批准，依 CLAUDE.md §4.5 本结论为里程碑级）

- **范围与独立性：** `00f195e..828819b` 恰 22 文件（均 M04 范围；`src/app.rs` 未变、workflow 未变、无 M05/M06 掺入）；本 reviewer 未参与任何 M04 实现。
- **CI：** head `828819b` 的 Windows CI `35886960491`（**257 项测试 0 失败**，MSVC release EXE/版本守卫通过，cargo-deny/about 全绿）+ Search benchmark `35886960196`（`1 passed` + 6 行 BENCH，守卫通过）独立核验，head SHA 与 `git rev-parse HEAD` 一致。MSVC 门禁成立，但 **CI 无法暴露 F001/F002 的运行时行为缺口**。
- **阻断性 finding：** **F001（High，热键注册竞态）与 F002（High，message-only 窗口不可被 FindWindowW 枚举）必须关闭**——两者都能由 head 代码直接证明，影响 M04.2/M04.3 核心验收。
- **非阻断 finding：** F003（Medium，per-monitor DPI 未按目标 monitor 换算）建议本轮修复或明确定为已知限制并入 M07.4 加固；F004（Low，Show vs Toggle 语义对齐）、F005（Low，abort() 终态）、F006（Low，错误文案按 kind 细分）、F007（Low，VM Open 状态缺直测）、F008（Info，过时注释/预留枚举）纳入 response 处置。
- **后续义务：** response 须逐条给出 ACCEPTED/REJECTED 评估与修复计划；修复后重新推送、运行 MSVC CI 并定位新 run ID；随后由独立 reviewer 复审（r02）。桌面手工验收项（末节）在 M07/M08 强制。
