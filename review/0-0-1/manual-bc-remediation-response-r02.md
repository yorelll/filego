# FileGo 0.0.1 — B/C 手工验收整改实现响应 r02

- **版本 / topic / 轮次 / 日期**：`0.0.1` / `manual-bc-remediation` / `r02` / 2026-10-08。
- **角色**：implementation agent，未参与对本轮实现的独立评审，不批准发布。
- **对应评审**：`review/0-0-1/manual-bc-remediation-review-r01.md`，结论 `CHANGES_REQUESTED`。旧用户手工验收仅针对 `a16d28b015ac5816677f3bba961f8e89962fc119`，用户修改的 `manual-acceptance.md` 未暂存、未由本实现改写；FAIL/PARTIAL 不变，新候选须另行验收。
- **修复前 / 修复后**：`4c37bb05266e8bbc9b78b353d4df254ec4a48f73` → `486c1e4b6413c390f4e5752ae37d98cf28766e95`，对比 `git diff 4c37bb0..486c1e4`。代码范围：`src/main.rs`、`src/presentation/{i18n,mod,settings_controller,startup_registration}.rs`、`src/storage/{repository,repository_tests}.rs`、`ui/app-window.slint`。未修改 Cargo、workflows、真实目录、用户数据或旧验收结果。
- **本提交权威 MSVC CI**：[Windows CI run 37766128334](https://github.com/yorelll/filego/actions/runs/37766128334)，`headSha=486c1e4b6413c390f4e5752ae37d98cf28766e95`，`completed/success`，job `fmt, clippy, test, release, package`，artifact `FileGo-0.0.1-windows-x86_64-486c1e4b6413c390f4e5752ae37d98cf28766e95`。[Search benchmark run 37766128401](https://github.com/yorelll/filego/actions/runs/37766128401)，同一 head SHA，`completed/success`，job `release 10k benchmark`，artifact `search-benchmark-log-486c1e4b6413c390f4e5752ae37d98cf28766e95`。由指定完整路径 `D:\Program Files\GitHub CLI\gh.exe` 核对。开发 artifact 不是经独立 review 与新 SHA/hash 验收的 RC。

## 逐条评估和修复

| Finding | 结论 | 修改、证据与风险边界 |
|---|---|---|
| **H01 High**：损坏主配置不可恢复、告警清空 | **ACCEPTED** | 数据页新按钮调用 `repair_from_backup()`，保留损坏 main 证据并显式提升有效内部 `.bak`；命名快照走新 `DocumentRepository::restore_named_backup`：锁内重读 main，拒绝健康 main 被无意覆盖，写独立 corrupt evidence 后经同目录同步临时文件原子提升，并清 pending recovery。普通健康 main 的命名备份恢复失败时回滚工作副本。成功后 reload settings、manager 及 search。`sync_ui`/`sync_settings_ui` 使用一致的 settings 优先 notice，不再由空 manager notice 清除 `DataUnreadable`。真实 repository 测试覆盖坏 main+有效内部 backup 的既有修复、坏 main+无内部 backup 的命名快照恢复、健康 main 拒绝、权限拒绝和 promotion 故障原件/快照不变。数据页实际点击/视觉、无权限真实机器、K7/K8 仍须新 RC 手测。 |
| **H02 High**：HKCU 在无文档状态仍被写入、误报保存成功 | **ACCEPTED** | tray 与设置页两条回调都经 `SettingsWindowController::change_launch_at_login` → 可注入 `RunRegistration` 事务。无 loaded document 直接拒绝，pending recovery 在 adapter 拒绝，或在 repository 的 `save_at` 中失败且不写 OS；SettingsController 新 `Result` API 不再通过布尔快照判断保存成功。先保存 JSON 再写 HKCU，写入后复读核实；写入失败也当作可能部分生效，分别尝试恢复 OS 与 JSON，再复读 HKCU，任一失败匿名报错、不更新 tray glyph。成功才更新 glyph。fake registry 测试覆盖 enable/disable/幂等、无文档、写前读失败、磁盘保存失败、Win32 写入失败/部分写入、验证读失败、两端独立回滚及回滚失败；真实 repository pending recovery 测试证明不会调用 OS 写入。没有自动化读写用户真实 HKCU；J1–J6/C3.6 在新 RC 桌面核验 Run 内容、portable 路径/启停/重登。 |
| **H03 High**：40ms timer 返回后被 Drop 取消 | **ACCEPTED** | 两处短生命周期 `Timer::default().start(SingleShot)` 改为 Slint 1.18 `Timer::single_shot`；`single_shot_callback_survives_scheduling_function_return` 在实际 Slint event loop 中运行回调，配 2s watchdog 避免无限挂起。仍不能由 CI 证明 Windows 前台授权/白屏修复；C1/C3.1/C3.3/C5 须新 RC 多次桌面复测。 |
| **M01 Medium**：草稿跨页显示，固定嵌套滚动区捕获滚轮 | **ACCEPTED** | draft、offer、batch 均增加 page 1 可见性条件；用户切页时保留未保存草稿但不在“常规”等页渲染，返回 Folder 页继续编辑。移除 Folder 列表 400px 嵌套 ScrollView，统一由右侧外层 ScrollView 负责内容滚动。结构 guard 检查条件与滚动容器、Slint 编译期检查；静态断言不等于可点击 Save 或缩放实测，新 RC 在 760×520、125–200% DPI、滚轮/键盘/外侧 scrollbar 下复测。 |
| **M02 Medium**：缺少首次空备注磁盘 round-trip 和可注入启动项故障测试 | **ACCEPTED** | `first_run_empty_note_draft_saves_and_survives_restart` 在临时真实磁盘目录从 absent data dir 初始化，使用 `ManagementController<SharedStore>` 保存 name+path+空备注、销毁后重新打开 repository 断言记录与原目录仍在；registry seam 的上述 10 项测试与真实 recovered repo 的 pending-repair 故障覆盖纯逻辑；无 CI 会对用户 HKCU 作真实写/删。picker、窗口像素、焦点、Win32 HKCU round-trip 只能由用户新候选手工复测。 |

## 本地 GNU 快速反馈与提交状态

设置 `RUSTUP_AUTO_INSTALL=0`、`RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-gnu`；预检：Rust/Cargo `1.92.0`、target `x86_64-pc-windows-gnu`、`rustfmt`、`clippy` 均已安装。本地未自动安装/更新任何 toolchain 或组件。

- `cargo fmt --all -- --check`：PASS；`cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings`：PASS。
- `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu`：410 lib + 10 bin PASS，0 failed，1 既有专用 10k benchmark ignored。`cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu`：PASS；`git diff --check`：PASS。本地 GNU 产物仅开发检查，不用于 release/验收/哈希。
- 先前调试中 Clippy 报布尔断言风格错误，改为 `assert!` 后全绿；首次全量测试曾发现静态布局断言误把 Category 页的 400px 容器当 Folder 页，缩小 guard 范围后全绿。真实逻辑与 UI 仍需要独立复审。
- 代码提交 `486c1e4` 已推送，Windows CI 与 benchmark 均对**精确提交**成功。本文为纯文档审计证据，后续推送按 `paths-ignore` 不触发 CI。

## 待独立复审与人工验收

请求未参与实现的 reviewer 对 `4c37bb0..486c1e4` **实际代码、测试及精确 SHA 的两次 CI** 逐项复审 H01–H03/M01–M02；如仍有阻断问题，实施 agent 必须再修复并经新 CI/复审，不能自批。旧手工 B7 FAIL、C1/C2/C3 PARTIAL、C5 FAIL 与 D+ 未测均保持不变。只有审查通过、主 agent 创建并验证全新的 MSVC RC/EXE/ZIP/SHA-256 后，才可让项目用户复测 B/C、J1–J6、K7/K8 和继续 D+；CI 不证明真实 Win32 前台/Slint 绘制/GUI 可滚动。此 response **不是 `APPROVED_FOR_RELEASE`**，无 tag 或 GitHub Release 授权。
