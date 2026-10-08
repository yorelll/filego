# FileGo 0.0.1 — B/C 手工验收整改独立代码评审 r01

- **版本 / topic / 轮次 / 日期**：`0.0.1` / `manual-bc-remediation` / `r01` / 2026-10-08。
- **Reviewer**：独立 FileGo code-review agent；**未参与本轮所评代码的实现**，不兼任本轮 implementation agent，也不作发布批准。
- **旧桌面候选**：`a16d28b015ac5816677f3bba961f8e89962fc119`；项目用户在 `review/0-0-1/manual-acceptance.md` 记录的 B/C 结果仅针对这一候选，不自动转为新代码 PASS。
- **被评审实现提交**：`caa671efeb5b8e9036fca97486b3d2b076f76a33`；比较范围 `ed1b2327cf7a2aa378244bf63b90661c5f8ffc48..caa671efeb5b8e9036fca97486b3d2b076f76a33`（完整 `git diff`）；实现响应为后续纯文档提交 `bd4ecfdccc1152f07a6590df910caaa3e6b41612`。代码范围：`src/main.rs`、`src/platform/windows/data_location.rs`、`src/platform/windows/mod.rs`、`src/platform/windows/tray_open.rs`、`src/platform/windows/window_focus.rs`、`src/presentation/manager.rs`、`ui/app-window.slint`。额外核对 `src/storage/repository.rs`、`src/storage/io.rs`、`src/storage/backup.rs`、`src/presentation/settings_controller.rs`、`src/platform/windows/folder_picker.rs`、`src/app.rs` 与 Slint 1.18.0 上游 timer/ScrollView 行为；未修改任何被评代码。
- **工作树与原始证据**：检查时 HEAD `bd4ecfd`；`manual-acceptance.md` 是项目用户已修改、**未暂存**的文件，保留原样，本 review 不暂存、不覆盖、不转换为新候选结果。`git diff ed1b232..caa671e --check` 通过；七个改动文件无新增网络、遥测、全量目录扫描、真实目录删除调用、用户路径/搜索词日志或依赖/workflow 变动。
- **精确提交 CI**：[`Windows CI` run 37750361218](https://github.com/yorelll/filego/actions/runs/37750361218)，`headSha=caa671efeb5b8e9036fca97486b3d2b076f76a33`，`completed/success`，job `fmt, clippy, test, release, package`（MSVC fmt/Clippy/测试/Release EXE/版本/许可审计/portable development 打包上传均 success）；artifact `FileGo-0.0.1-windows-x86_64-caa671efeb5b8e9036fca97486b3d2b076f76a33`。[`Search benchmark` run 37750361105](https://github.com/yorelll/filego/actions/runs/37750361105)，**同一 headSha**，`completed/success`，job `release 10k benchmark`，artifact `search-benchmark-log-caa671efeb5b8e9036fca97486b3d2b076f76a33`。使用规定的绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查验；response 写作时 CI 尚在运行，但现已成功。CI 开发 artifact **不是**新 RC 的 EXE/ZIP/SHA-256 验收链，也不证明 GUI 像素/前台/HKCU 行为。

## 验收映射与可证事实

| 项目用户的旧候选结果 / 关联里程碑 | 本提交的代码证据 | 本轮判断 |
|---|---|---|
| B7 FAIL、C3.5 保存失败；M01/M05/M06/M07/M08 | `main.rs:2041-2090` 在真实 `NotFound` 后创建 known-folder 目录、复读、锁内 `save_if_current`；`data_location.rs:11-22` 从 `FOLDERID_LocalAppData` 取得路径，不退回 TEMP。`manager.rs:2230-2246` 证明内存 store 的空备注可保存。 | 首次正常启动的修复有实质依据，且现有 main/backup 不被初始化路径直接覆盖；**已有损坏 main + 可恢复 backup 的用户仍无法经 UI 修复**（H01）；旧 B7/C3.5 不得改填 PASS，必须实测首次创建、空备注保存并重启。 |
| C1 PARTIAL、C3.1 只能显示任务栏图标；M00/M03/M04/M08 | `window_focus.rs:66-121` 尝试 restore/raise/foreground；`main.rs:39-45,144-176` show 后请求 redraw 并安排 40ms 再激活。 | 原生首次激活路径确有改善，但延迟激活 timer 立即销毁（H03）；Windows 前台授权/Slint 焦点仍需桌面复测。 |
| C2 PARTIAL（无现象描述）；M04/M08 | 没有专门的双击行为修复或测试。 | **未解决/信息不足**；先向用户取得具体动作、闪烁/双击观察，再复测，不得声称通过。 |
| C3.2 cancel 仍打开设置；M05/M08 | `main.rs:237-241,1522-1537,3420-3428` 只有非空 `Picked` 导航显示，纯结果分类测试 `main.rs:4633-4643` 覆盖 Cancelled/Failed/空选择。 | 路由层修复有针对性；真实 COM picker 取消时的结果及已经打开的 SettingsWindow 行为仍需测试。 |
| C3.3 / C5 白屏和局部绘制；M03/M04/M06/M08 | 设置页同步模型、show 前后各 `request_redraw`（`main.rs:188-230`）；主窗 show 后 `request_redraw`。 | 即时 invalidate 是代码事实，**不能证明软件渲染已修复**；40ms 设置页重绘 timer 同样立即销毁（H03）；没有图像/交互测试。 |
| C3.4 表单不可滚、错误归属“常规”页；M05/M06/M08 | `ui/app-window.slint:1299-1309,1355-1443,1831-1945` 给右侧整体加 `ScrollView`，托盘成功选中切换 page 1。 | 结构上有外层 viewport 和 Folder page 的入口，但草稿仍位于**所有 page 条件块之外**，导航后会出现在常规等页（M01），嵌套固定 400px ScrollView/最小尺寸滚动需要实测。源码字符串顺序测试不是 viewport/点击验证。 |
| C3.6 登录启动反复点击无响应；M04/M06/M08 | `tray_open.rs:59-145` 修复悬空 `PCWSTR`、使用 `RegDeleteValueW`、读取并比对当前 EXE；`main.rs:193-200,3191-3218,3944-3977` 在设置打开时重读并尝试回滚。 | 关键 Win32 错误有实质修复，但 **没有载入文档时仍可写 HKCU 且显示成功**（H02）；纯字符串单测不测试 HKCU。J1–J6/C3.6 必须真实桌面验证。 |
| B6 PASS 但“UI太丑”、C3.7 About 外观；M06/M08 | `ui/app-window.slint:1802-1829` 新层次卡片；版本/arch/项目/MIT/隐私字段仍在。 | 主观视觉需用户复测，不将原 B6 PASS 外推为新候选验收；About 未声称已签名。 |
| 原 B1–B5、C4、C6–C10 PASS；M00–M08 其他要求 | 本次未改动发布工作流、原有生命周期/搜索/安全写入的主要实现；MSVC CI 与 release benchmark 同 SHA 成功。 | **只保留旧候选的原始结果**。D 及后续用户未执行项目仍未验证；新候选必须由主 agent 单独构建/校验/供用户验收。 |

## Findings（按严重级）

### H01 — High — 损坏主文件+备份恢复无法经 UI 完成，且读取失败告警被新的显示流程清空

- **位置**：`src/main.rs:188-205,198-200,1292-1326,2041-2090,3403-3415`；`src/storage/repository.rs:387-434,470-535,899-917`；`src/storage/backup.rs:127-159`。
- **复现/证据**：预置损坏 `data.json` 和有效 `data.json.bak`，`open_repository_at` 返回 `Recovered` + `Unreadable`，repository 标记 `pending_recovery`；普通保存必返回 `RecoveryRequired`。唯一能解除该状态的 `repair_from_backup` 没有在 `main.rs` 被调用。数据页 `list_backups` 只列 `backup-*.json`，不列 `data.json.bak`；`restore_backup` 即使有用户创建的命名备份，也直接 `set_data` 后 `save_at()`，被 pending recovery 拒绝。没有可编辑文档（坏 main、无有效 `.bak`）时 `restore_backup` 的 `set_data()` 又会返回 `NotFound`。另首次设置窗口打开执行 `sync_settings_ui()` 后立刻 `sync_ui()`：后者 `main.rs:1605-1612` 用 manager 空 notice 重写同一 `notice-text`，掩盖 `DataUnreadable`（也掩盖其他 settings notice）。
- **影响**：B7 修复的错误/备份分支仍不能由用户执行承诺的恢复动作；无法新增/保存，可能误以为无错误。旧数据仍留在磁盘，但可用性/配置恢复构成发布阻断。
- **建议**：在数据页提供**明确的、保留证据的**内部 `.bak` repair 操作，调用已有加锁 `repair_from_backup()`；对损坏 main + 命名备份单独设计先保留坏文件/备份再恢复，切勿通过普通 `save_at` 覆盖坏原件；在此之后刷新 manager/settings/search。让 DataUnreadable 与其他设置错误具有明确 UI notice 优先级且打开窗口不会被 manager 空 notice 擦除。增加坏 main + 有效 `.bak`、坏 main + 无 `.bak` + 命名备份、权限拒绝、故障时原文件/备份仍字节一致的**真实仓库与 UI 适配层**测试；新候选真实桌面复测 K7/K8/B7。

### H02 — High — 无已加载配置时启动项仍写 HKCU，设置可误报保存成功

- **位置**：`src/main.rs:3193-3218,3946-3977,860`；`src/presentation/settings_controller.rs:699-724`；`src/platform/windows/tray_open.rs:59-145`。
- **复现/证据**：坏/不可读 `data.json` 且无可恢复备份时 `repo.document() == None`。从 tray 点击 Launch at login：先写 HKCU，`if let Some(previous)` 直接跳过持久化/回滚，然后更新 ✓ glyph。设置页同一状态下点击开：先写 HKCU，`SettingsController::persist` 的 `set_settings` 返回 NotFound，previous 回退使用**已经变更为 true 的 snapshot**；调用方仅比较 `view.settings.launch_at_login == on`，将失败当成功并同步 glyph；下次启动数据配置依旧缺失。真实 Win32 写/删本身修复不能证明应用整体状态闭环。
- **影响**：读取失败时会留下意外的登录自启动注册；UI 将未持久化设置误报成功，用户难以停用，属于用户可观测副作用/配置一致性缺陷。
- **建议**：HKCU 写前检查文档可持久化、记录原值，失败时有可验证的状态和清晰错误；禁止 `None` 文档改变 Run，设置保存以明确 `Result`/成功状态判定，避免用新旧布尔相等判断成功；写入后/回滚后重新读取 HKCU，回滚失败需告警。给 tray 与设置的无文档、pending-recovery、磁盘保存失败和 HKCU 失败路径增加注入测试，并在用户桌面核验 HKCU Run 的实际值、EXE 路径、启用/禁用及注销后行为。

### H03 — High — 两个“下一事件循环再前台/重绘”一次性 timer 从未运行

- **位置**：`src/main.rs:157-177,220-231`；依赖源 `D:\Users\lawrence_lv\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\i-slint-core-1.18.0\timers.rs:42-45,79-113,148-156,253-264`。
- **复现/证据**：`bring_search_to_front` 和 `open_settings_window` 都创建局部 `let redraw = slint::Timer::default()`，`redraw.start(SingleShot, 40ms, ...)` 后函数结束，Timer 的 `Drop` 会移除/停止尚未触发的 timer。现存别的 repeating drain timer 在 `run()` 持有，不会延长这两个新 timer 的生命。Slint 提供**无需保存对象**的 `Timer::single_shot`。因此 response 关于“延迟 40ms 激活 / 下一回合设置重绘”的声明不成立；立即 show/raise/invalidate 仍确实会执行。
- **影响**：C1/C3.1 任务栏可见但不前台和 C3.3/C5 随机白屏可能继续复现，且缺失预期的首次布局后再尝试；CI 不运行交互 GUI，不能验证。
- **建议**：使用 `slint::Timer::single_shot` 或将 timer 存活至回调；加真实事件循环测试证明回调触发（而非仅对时间常量断言），再在新 MSVC RC 上逐次复现/复测托盘唤起、settings 白屏；考虑观测 `FocusResult` 而不将 `VisibleNotForeground` 误报成功，并遵守 Windows 前台限制。

### M01 — Medium（当前验收阻断）— 添加草稿仍是跨页全局视图，布局守卫不验证滚动/点击

- **位置**：`ui/app-window.slint:1303-1312,1355-1443,1831-1945`；`src/main.rs:1512-1519,3420-3428,3461-3485,4646-4669`。
- **复现/证据**：托盘添加成功时 page=1，但 `if (root.draft-visible)` 是 page 0–8 的兄弟（不是 page 1 的子树）；打开草稿后点左侧“常规”，既没有隐藏/阻止导航，也没有关闭草稿，草稿仍出现在常规页末尾。字符串 `find("ScrollView {") < find("// ===== add/edit dialog =====")` 只证明文本次序；无法验证实际滚动边界、鼠标事件或 Save 在 760×520 / 125–200% 时可到达。Folder 页面仍含嵌套固定 `height: 400px` 的 ScrollView，Fluent 子 scrollbar/scroll 事件也可能吃掉外层滚轮，需实测。
- **影响**：用户明确提出“添加不是常规设置”与 Save 无法触及；入口页切换只是部分修复，实际导航仍可重现错误归属；可达性、键盘/文本缩放尤其不确定。
- **建议**：把 draft/preview/offer 放在 folders page 之内（或在离开时保留草稿但切回/禁止跨页渲染），确定离开/未保存提醒；测试 page 切换的组件语义，并在最小窗口 + 长列表/标签、不同 DPI/键盘/滚轮及外侧滚动条上实际点击 Save 验证。

### M02 — Medium（证据缺口）— 新单测覆盖纯判断/内存路径，未证明真实首次写入后添加并跨重启保存或 HKCU round-trip

- **位置**：`src/main.rs:4592-4669`；`src/presentation/manager.rs:2230-2246`；`src/platform/windows/tray_open.rs:328-365`。
- **复现/证据**：`first_run_creates_a_valid_document_under_an_absent_data_directory` 确实用 tempdir/真实磁盘创建版本化文件；坏 main 测试确实比较原始字节，都是有用回归。空备注测试却使用 `MemStore`（只自增 revision），未从初始空的真实 DocumentRepository 走 picker→`SaveDraft`→关闭→重新 `load` 并确认文件记录/备注。tray cancel 测试只测 `picker_should_open_settings` 布尔判定，而非 show/page 真实回调；`launch_at_login_value_matches_only_this_executable` 只比较字符串及手动 UTF-16 转换，没开 HKCU 读写删。
- **影响**：B7 之后 C3.5 及 C3.6 的用户报错能绕过现有单测；不要把纯测试/编译当作 Windows 桌面结果。
- **建议**：补首次初始化→单条空备注保存→重启读取的 repository/controller 集成测试，以及可注入 OS/registry adapter 的成功、读/写/删失败、回滚测试；GUI/COM/渲染/Win32 foreground 必须由项目用户实测。这里不是否定现有测试，其范围应如实表述。

## 横向检查、风险与待验证项

- **正确性/错误处理与数据安全**：initial NotFound → known folder `create_dir_all` → 复查 → 加锁 `save_if_current(...,0)` 使正常首运行成立，不将坏 main 直接当新用户。repository 对不可读主文件返回 Io、对坏主文件返回 CorruptData/Recovered，已有备份/损坏原件的读路径没有在本 diff 中被自动清空。H01/H02 仍阻断配置恢复及持久副作用。新代码未提供真实文件夹删除能力；记录删除与真正磁盘文件保持分离。需明确首个保存后 `.bak` 尚不存在，第二次保存才产生前一版本备份；不应向用户保证首次启动就有 `.bak`。
- **隐私/安全/Windows**：Known Folder 用 OS API 获取当前用户 LocalAppData，避免环境变量与临时目录歧义；`CoTaskMemFree` 与 `PCWSTR` 所指 buffer 生命周期已检查。HKCU 操作为当前用户，不提权，关闭改用 `RegDeleteValueW`，注册值引号包围 EXE，未新增命令解释器/路径上传/遥测，也未新增路径/查询日志。真实路径较长、Run 值陈旧、读写失败/回滚和 Explorer/tray foreground 权限仍需要用户侧验证；不能声称重绘问题由 MinGW 引发或由 MSVC CI 修复。
- **测试/性能**：MSVC fmt/Clippy/测试/Release 构建、开发包和 10k release benchmark 同 SHA success；未改搜索热路径或依赖。这里没有重新执行本地 GNU，避免将已有实现 agent 的 GNU 证据当本 reviewer 的新运行。40ms 新 timer 生命周期是源码可证的失败；100ms 唤出、真实机 50ms 搜索、CPU/内存/冷启等数字仍未由用户测量。
- **UX / a11y / 可维护性**：About 保留 0.0.1/x86-64/MIT/项目链接与隐私信息，改用既有 theme tokens；外观主观意见待复测。C3.4 外层 ScrollView 可望改善可达性，但仅文本静态测试且有跨页展示问题；最小窗口、滚轮/滚动条/键盘、中文/英文、200% 文本缩放/高对比皆不能由托管 CI 推出 PASS。错误 notice 两个控制器写同一 Slint 属性，H01 所述覆盖应统一状态来源或明确优先级。

## 复审与手工门禁

实现 agent 应逐条回应 H01–H03、M01–M02，修复后提供实际新 commit 的 MSVC Windows CI/benchmark，独立 reviewer 复审完整 diff/测试/CI。然后主 agent 冻结**新**候选 commit，用 release-candidate workflow 生成新的未签名 EXE/ZIP/SHA-256、下载并校验哈希，由项目用户在**新 artifact** 上至少复测：B7（纯首次启动、现存正常文档、坏 main + 有效/无有效 backup、无权限）；C1/C3.1（隐藏区单击、菜单 Open、反复显示隐藏及前台），C2（先明确现象），C3.2（folder picker 取消/失败），C3.3/C5（多次打开/关闭设置、首帧/白屏），C3.4（760×520/不同 DPI 及中文/英文、外层滚动与 Save、切页后草稿归属），C3.5（不填备注保存、重启搜索/数据文件确认），C3.6 + J1–J6（HKCU Run 真实状态、关闭/重开/重登、中文/空格路径及失败回滚），B6/C3.7（About 外观），并续测当前未进行的 D+。旧用户结果不得编辑为新候选 PASS，`manual-acceptance.md` 当前未暂存，提交本 review 时须只暂存本 review，避免混入用户改动。

**最终结论：`CHANGES_REQUESTED`（仅本次整改代码审查，不是 `APPROVED_FOR_RELEASE`）。** 三项 High 故障与 C3.4 UI 归属仍需修复；旧候选用户 FAIL/PARTIAL、缺失新 RC 和真实桌面复测、缺少最终独立 release approval 的发布门禁均未满足，禁止 tag/GitHub Release。
