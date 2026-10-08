# 0.0.1 B/C 手工验收问题整改 — 实现响应 r01

- **角色**：implementation agent；不参与本轮变更的独立评审与发布批准。
- **原候选**：`a16d28b015ac5816677f3bba961f8e89962fc119`，Build release candidate run `35981840362`。用户原始 B/C 结果见 `manual-acceptance.md`，原件和所有结果行均未由实现 agent 修改或提交。
- **整改基础**：`ed1b2327cf7a2aa378244bf63b90661c5f8ffc48`。
- **修复代码提交**：`caa671e`（`fix: repair first-run storage and B/C desktop flows`）。
- **权威 MSVC CI**：Windows CI run [`37750361218`](https://github.com/yorelll/filego/actions/runs/37750361218)，Search benchmark run [`37750361105`](https://github.com/yorelll/filego/actions/runs/37750361105)；均对应代码提交 `caa671e`。benchmark **success**，Windows CI 截至本文写入时仍 **in_progress（未验证完成）**；不以旧 run 冒充新提交证据。
- **新 RC**：尚未构建；原候选 `a16d28b` 的 artifact 不可冒充新整改候选。仅在独立评审批准后由主 agent 启动新 RC 构建及新哈希验收。

| 项目（用户结果） | 根因与改动 | 自动验证 / 尚需用户验证 |
|---|---|---|
| B7 FAIL：首次无数据目录，误报数据无法读取 | `open_repository_at` 将真 `NotFound` 与损坏/不可读区分，首次先创建数据目录、重查并原子写入空版本化文档；恢复自 backup 时保留 `Unreadable` 提示，不自动覆盖损坏证据。数据目录从 Windows `FOLDERID_LocalAppData` 读取，无 TEMP/工作目录回退。 | `first_run_creates_a_valid_document_under_an_absent_data_directory`、`corrupt_existing_data_is_preserved_and_not_treated_as_a_fresh_run`；B7 必须在新候选上复测。 |
| C1 PARTIAL / C3.1 托盘及菜单只能产生任务栏按钮 | `window_focus` 在托盘激活时恢复 iconified 的 HWND，并无条件提升 native HWND z-order、再走前台限制回退；AppWindow 显示后请求 redraw，延迟一次 40ms 激活以跨越首帧布局。 | 逻辑仍依赖真实 Windows 前台授权与托盘事件；C1、C3.1 必须桌面复测，未宣称 PASS。C2 缺具体异常描述，维持用户的 PARTIAL，待具体现象。 |
| C3.2 取消 folder picker 仍打开设置窗口 | picker 只有 `Picked(nonempty)` 才转 `OpenPickPaths`、切到文件夹页并显示 SettingsWindow；`Cancelled`/`Failed`/空集合不导航。 | `tray_add_picker_cancel_does_not_open_settings`；用户复测真实 native dialog。 |
| C3.3、C5 概率白屏/局部绘制，resize 后才恢复 | 设置窗口在 show 前同步全部模型，并于同步后、show 后和下一个事件循环回合分别 `request_redraw`；主搜索窗口首帧与后续激活同样显式重绘。 | Slint/MSVC 实际绘制问题需在新候选桌面复测；静态/编译测试不等于像素验证，不推断为 MinGW 库造成。 |
| C3.4 小窗口无法滚至添加表单；表单看似在常规页 | SettingsWindow 整个右内容区改为可滚动 `ScrollView`，包含管理页、草稿和 Save；托盘 Add 成功时显式切到文件夹页而非保留常规页。 | `settings_slint_keeps_draft_and_page_within_scrollable_content` 静态布局 guard + Slint 编译；用户复测 760×520/较小桌面。 |
| C3.5 正常目录不填可选备注无法保存 | 同一首次初始化失败导致 repo 无已加载文档，`put_folder` 返回 `NotFound`，控制器仅显示 `SaveFailed` 且草稿仍 `unsaved`。首次创建有效 repo 后不再该错误；并加 name+path+空 note 保存成功回归。 | `plain_folder_with_blank_optional_note_saves_and_closes_the_draft` + B7 回归；用户须在新候选复测实际 picker→保存→重启。真实盘/权限故障仍显示匿名保存失败，不伪报成功。 |
| C3.6 登录启动勾选反复点击无响应 | 原 `pcwstr(Vec<u16>)` 返回指向已释放 buffer 的 PCWSTR；且原用 `RegSetKeyValueW(..., None, 0)` 删除 value 语义错误。改为 Win32 调用期间持有两个 UTF-16 key/value buffer，关闭时显式 `RegOpenKeyExW`+`RegDeleteValueW`；状态读取比较实际 HKCU Run 内容与当前 EXE 路径，并在设置每次打开时重读；写入或持久化失败回滚 UI/注册状态，不假成功。 | `launch_at_login_value_matches_only_this_executable` + GNU/MSVC 通过；HKCU 行为仅真实桌面可验证，保留 C3.6 复测。原有已知的 HKCU Run 值不在本轮自动删除。 |
| B6 PASS 但整体 UI 与 C3.7 About 过于粗糙 | About 页面增加清晰标题、版本/架构层级和分组卡片，仍只使用现有 theme token 与 zh/en 文案；不新增功能。 | UI 感受是用户主观反馈，须用户复测；不得将“更好看”代填 PASS。 |

## 本地 GNU 验证

按仓库 `CLAUDE.md` §3.1 预检：`RUSTUP_AUTO_INSTALL=0`，Rust/Cargo `1.92.0-x86_64-pc-windows-gnu`，已安装 GNU target、rustfmt、clippy。MinGW 的 windres/链接器需会话级 PATH + `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS`（**未提交配置**）。

- `cargo fmt --all -- --check`：PASS；`git diff --check`：PASS。
- `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings`：PASS。
- `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu`：**396 lib + 8 bin 通过，0 failed，1 ignored**（原有 10k release 专用基准）。
- `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu`：PASS；本地 GNU 仅快速反馈，不能替代 MSVC CI 或成为发布 artifact。

## 待独立评审 / 待实际桌面复测

此响应不是 `APPROVED_FOR_RELEASE`；请独立 code-review agent 阅读 `git diff ed1b232..<fix-head>`、实际源码和针对本提交的 MSVC CI，再决定是否允许构建**新** RC。用户先前 B/C 结果仍是原候选 `a16d28b` 的事实，绝不继承为修复后的 PASS。优先复测 B7、C1/C2、C3.1–C3.7、C5；若无数据初始化/保存仍失败，D 及后续仍 BLOCKED。设置白屏/托盘前台行为依赖真实 Windows 桌面，托管 CI 无法证明其已修复。新候选的 exe/zip/hash 和候选身份由主 agent 在独立评审后生成并记录；不得提前覆盖用户当前交付物。
