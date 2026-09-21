# QuickFolder

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/v1.18.0/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)

QuickFolder 是面向 Windows 的轻量级文件夹快捷启动工具。它常驻系统托盘；用户通过全局快捷键唤出搜索窗口，输入名称、路径、分类或标签，并用键盘快速打开已添加的文件夹。

> 当前状态：`0.0.1` 开发中。M00 工程与 CI 骨架已经建立。Windows CI run `35562858225`（head `08bfb527`）已通过格式、Clippy、测试、Release 构建与打包；真实 Windows 桌面行为和 release-candidate workflow 仍待验证。

## 产品目标

- 快速唤出、立即搜索、回车打开，优先支持纯键盘操作。
- 管理由用户主动添加的本地目录、映射盘、UNC 网络路径和移动磁盘目录。
- 不扫描磁盘、不索引普通文件、不上传用户路径或搜索内容。
- 空闲时接近零 CPU 和持续磁盘活动，保持较低常驻内存。
- 提供简体中文与英文界面，并适配深浅主题、高 DPI 和多显示器。

## `0.0.1` 范围

首版计划包含：

- 系统托盘、单实例运行和可配置的全局快捷键。
- 顶部居中的搜索窗口，以及名称、路径、分类和标签搜索。
- 多关键字及连续模糊匹配、组合筛选、键盘选择和回车打开。
- 文件夹记录的添加、编辑、移除、分类、标签和置顶管理。
- 不可访问路径提示；保留暂时离线的网络盘或移动磁盘记录。
- 设置窗口、开机启动、浅色/深色/跟随系统主题。
- 本地 JSON 数据存储，以及配置导入和导出。
- 中文输入、高 DPI 和多显示器支持。

首版不包含全盘索引、普通文件搜索、云同步、账号、插件系统、内置终端或完整文件管理能力。

## 技术与平台

- 语言：Rust
- UI：Slint
- 目标系统：Windows 10 22H2、Windows 11
- 首版架构：Windows x86-64
- 发布物：未签名的便携版 `.exe` 与 `.zip`，附 SHA-256 校验值

## 默认交互

- 全局快捷键：`Ctrl + Alt + Space`
- 主题：跟随系统
- 最大搜索结果：8 条
- 主窗口：当前鼠标所在显示器顶部居中
- 打开文件夹后隐藏窗口并清空搜索内容
- 空搜索框显示置顶项和最近使用项

删除操作只会移除 QuickFolder 中的快捷记录，绝不会删除磁盘中的真实文件夹。

## 开发与验证

本项目的开发流程和强制质量门禁见 [`CLAUDE.md`](CLAUDE.md)。本地环境不提供 Rust 工具链，因此 Rust 编译、格式检查、静态检查和测试均以 GitHub Actions 的 Windows CI 结果为准。

交互类能力（托盘、全局快捷键、中文输入、多显示器等）需使用 CI 生成的便携版在真实 Windows 桌面上完成发布前手工验收。

当前 M00 只提供架构验证壳：静默托盘启动、最小搜索窗口、显示/隐藏及退出。搜索、持久化、快捷键、单实例和完整设置尚未实现，不能视为可发布版本。

### 获取开发构建

代码推送后，GitHub Actions 的 `Windows CI` workflow 会执行格式、Clippy、测试、Release 构建、依赖审计和打包。成功 run 的 artifact 名为 `QuickFolder-0.0.1-windows-x86_64-<commit>`。本地没有 Rust 工具链，不提供本地构建结果。

首次 CI 是特殊 bootstrap：如果仓库尚无 `Cargo.lock`，workflow 会生成并上传 `cargo-lock-<commit>`，随后故意失败。维护者必须下载、检查并将该 lockfile 提交；之后 CI 才允许继续。这避免长期构建静默更新依赖。

### 架构记录

关键技术决策见 [`docs/adr/`](docs/adr/README.md)，依赖与许可证门禁见 [`docs/dependency-policy.md`](docs/dependency-policy.md)。

## 数据与隐私

用户数据默认仅保存在本机，不扫描文件夹内容，不要求登录，不启用遥测。应用提供主动导入、导出和清理能力。

## 许可证

QuickFolder 项目源代码使用 [MIT License](LICENSE)。Slint 采用其桌面 Royalty-free 2.0 许可选项，因此应用保留可访问的 “Made with Slint” attribution；各依赖仍适用各自许可证。正式发布物必须包含 CI 生成并经审查的第三方许可证清单，以及仓库中固定保存的 Slint 许可证文本。
