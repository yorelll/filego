# FileGo 0.0.1 — Release Candidate 已知问题清单（RC known issues）

> 版本：0.0.1
> 主题：`m07-known-issues`
> 日期：2026-09-21
> 目的：M07.6 要求把「所有已记录延期项」集中列举并标注状态；区分「已关闭」「记录为已知限制（可随 0.0.1 发布，不构成数据安全/正确性缺陷，评审接受）」「MVP 缺失（不得改写为 known issue 直接发布）」。MVP 缺失被单列，绝不混入 0.0.1 known issues。

## 状态约定

- `CLOSED`：本 M07 轮已修复（含代码/测试/CI 证据）。
- `DISPOSED`：评审已明确接受为已知限制或非缺陷，不阻塞 0.0.1；记录理由。
- `DEFERRED_IN_REVIEW`：独立评审责令延期并明确接受，需在 release-review 前关闭或由 reviewer 再次明确接受。
- `MVP_MISSING`：本属 0.0.1 之外的功能/MVP 缺失，**不得**以 known issue 名义放行发布。

## 已关闭（本 M07 轮）

| ID / 源 | 状态 | 说明 |
|---|---|---|
| M04-N001（Low/Info：托盘未显示快捷键不可用） | `CLOSED` | 托盘新增禁用提示行 `hotkey-unavailable-hint`（`tray_hotkey_hint`，main.rs），启动和设置保存后同步刷新；i18n 双语；行 `enabled:false` 不可激活。见 M07.1。 |
| M06-I1（before-import 快照秒级 stamp 碰撞） | `CLOSED` | `backup::unique_stamp` 对同秒同前缀生成 `-2/-3` 后缀；测试 `unique_stamp_disambiguates_same_second_backups`。见 M07.1。 |
| M06-I2（保存失败留下冗余快照） | `CLOSED` | 若 `save_at` 失败，`backup::remove_failed_import_snapshot` 只清理本次刚创建的 `backup-before-import-*.json`；manual backup 不可作为目标，缺失清理幂等。回归测试 `failed_import_snapshot_cleanup_removes_only_the_named_backup`。同时回滚 repository working copy，避免失败导入残留内存状态。 |
| M06-I3（冲突时 Apply 仍可点） | `CLOSED` | 预览 conflict > 0 时 Apply 按钮 `enabled: false`（ui/app-window.slint），前置拦截 all-or-nothing 拒绝。 |
| M05-L2（undo 横幅 Cancel 语义） | `CLOSED` | 新增 `contextmenu.undo_dismiss`（"保持已移除"/"Keep removed"）替代含混的 Cancel。 |
| M05-L3（空路径手动添加的提示） | `CLOSED` | 空路径草稿显示 "请输入文件夹路径"/"Enter a folder path"（`Msg::NoticeEnterPath`）而非通用的 "无效路径"。 |
| M05-OBS-01（context-action 动作码耦合） | `CLOSED` | `RowAction::from_context_action` 命名常量 + 单测镜像 MenuRow 顺序。 |
| M05-OBS-02（action_toggle_enabled 死键） | `CLOSED` | 删除死 i18n 键 + slint 属性 + setter。 |
| M01B-N001-followup（repair_from_backup 不加锁） | `CLOSED` | `repair_from_backup` 全程持有 `WriteLock`（repository.rs）；回归测试 `repair_from_backup_contends_with_a_concurrent_writer_via_the_lock`。发布前必须关闭的项现已关闭。 |
| M07.1 配置读取失败静默 | `CLOSED` | `open_repository` 返回 `StartupDataStatus`，数据未读时 Data 页显示匿名双语通知 `settings.notice.data_unreadable`；恢复动作（打开数据位置/恢复备份/重置）原本已可达。损坏文件保留，不自动覆盖。 |
| M07.1 panic hook 脱敏（redaction） | `CLOSED` | `install_redacted_panic_hook` 只写匿名行到数据目录 `panic.log`（`diagnostics::log_panic`，256KB 上限 + `.1` 轮转）；payload/用户路径/query 一律不落盘；release 无 console spam。 |
| M07.5 导入资源限制 | `CLOSED` | `MAX_IMPORT_BYTES=8MiB`、`MAX_IMPORT_RECORDS=100k`；解析前的线性 count probe（不做 O(n²) 全量解码）；超限返回 `ImportTooLarge`，不 mutation；i18n + 通知。3 项测试。 |
| M07.5 shell/command 源守卫 | `CLOSED` | `storage/tests.rs` 递归扫描禁止 `std::process::Command`/`powershell`/`cmd.exe`/`CreateProcess` 等；ShellExecuteExW 边界必须 `lpVerb=null`/`lpParameters=null`。 |
| M07.5 GH Actions 供应链 | `CLOSED` | `workflows_pin_actions_and_use_minimal_permissions` + `release_workflow_never_runs_untrusted_code_with_write`：actions 全 full-SHA pin、`permissions: contents: read`、dispatch-only、无 write scope、无 release job。 |
| M07.2 快速连续保存/并发 | `CLOSED` | `rapid_successive_saves_all_persist_in_order`（100 连写）+ `concurrent_burst_saves_never_torn_main`（4 线程×20）+ `rapid_toggling_persists_every_command_no_lost_update`。 |
| M07.4 125/150% 几何 | `CLOSED` | `window_position.rs` + `window_placement.rs` 新增 100/125/150/200% 纯几何测试。真实桌面仍手工。 |

## 记录为已知限制（0.0.1 可接受，评审已明示/本清单记录）

| ID / 项 | 说明 / 依据 |
|---|---|
| M02-F005 多音字首读音 | `keys.rs` 文档化；确定性取首读音，非常用读音拼音搜索可能不命中。r02 CLOSED（文档）。对应桌面验收项。 |
| M02-F003 重复检测 O(n²)（`duplicate_folders` 全库定位） | 保留（仅显式「定位重复」按钮触发 O(n²) 全库扫描，不在搜索热路径；搜索核心本身无路径比较）。性能数字为本地 debug 快速反馈（10k 时逐键 `is_duplicate_path` ~77µs debug / release ~个位数 µs；全库扫描 debug ~772ms / release ~80ms），**非 MSVC 权威、非 Git 可复算产品断言；为（目标/待真实机器测量），权威数字由 M07.3/桌面性能验收实测后补入 `review/0-0-1/manual-acceptance.md`**。 |
| M02 review F005（MSVC>50ms 基线） | 归 M07.3 真实机器复核；CI 基准只做回归门禁（宽松 500ms，本机 hosted 数字非产品断言），权威 50ms 由真实桌面测量。**尚未实测量化，列为（目标/待真实机器测量）**。 |
| native 失焦钩子（hide_on_focus_loss） | M06.2 已接 接入设置；真实 native 失焦行为留桌面手工验收（M08）。 |
| Explorer 重启 tray 恢复 / 显示器变化 / 睡眠唤醒 / 网络断连 | M04/M07 桌面手工验收项（task/03 C8、H5/H6、L9）。 |
| 混合 DPI / 文本缩放 / 高对比 / 减少动画 / 屏幕阅读器 | M07.4 桌面手工项；screen reader 受 Slint 1.18 限制，如实披露、不虚假宣称（task/03 I7/I9/I11）。 |
| Microsoft 拼音候选放置/composition | 桌面手工验收（task/03 G1-G3）；presenter 层 IME gate 已实现并有状态机测试。 |
| OS 拖拽（drag-drop） | M05 记录为 seam：Slint 1.18 不透传 DroppedFile；剪贴板粘贴可用。桌面手工项。 |
| 线程模型 | 单 worker 线程（filego-native-worker）+ 每条 shell open 一个临时线程；无 per-show 线程生成；`Rc` 不强捕获进线程。 |
| 空闲 CPU/内存 | 无 timer busy-loop 是 **静态代码结构结论**（3 个 50ms drain timer 仅 `try_recv`，空 channel 立即返回），**不得声称「实测 CPU 0%」**；内存 `<50MB` 为**优化目标（待真实机器测量）**，由 M07.3/桌面性能验收实测，权威记录计划补入 `review/0-0-1/manual-acceptance.md` —— 此前不作为已达标事实。 |

## MVP 缺失（**不得**标为 known issue 放行发布；仅列清单）

- 搜索历史与智能频率排序（0.0.1 history off，UI 诚实标注）。
- 批量编辑/删除/分组移动增强、递归/后台目录扫描。
- 终端/编辑器/自定义动作系统、URL Scheme、SendTo、标签层级/自动规则。
- 启动时全量路径检查、路径相似自动匹配。
- 加密备份、同步、自动更新、ARM64、安装器与签名。
- `M07.3` 中的 hotkey→visible ≤100ms 与 cold-start ≤1s 仅是「测量目标」，未在本机定制测量前不作为已达标事实。

## 说明

- 以上 CLOSED 项均随 MSVC CI run 执行。权威 run ID 见 `review/0-0-1/m07-hardening-response-r01.md`（Windows CI `35950631364` + Search benchmark `35950631427`，head `367bb8c`）；CI 全绿前不作为已验证。
