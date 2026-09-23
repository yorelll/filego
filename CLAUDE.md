# FileGo Agent 开发规则

本文件约束所有在本仓库中工作的主 agent 与子 agent。除非用户明确修改规则，否则不得跳过。产品范围与默认行为见 `README.md`；详细实施清单见本地 `task/` 目录。

## 1. 固定项目约束

- 项目是面向 Windows 10 22H2 / Windows 11 的 Rust + Slint 原生桌面应用。
- 首个目标版本是 `0.0.1`，目标架构为 Windows x86-64。
- 用户数据使用本地 JSON 文件；必须采用安全写入、备份和可恢复策略。
- 发布物为未签名的 portable `.exe` 与 `.zip`，并提供 SHA-256；不得暗示其已签名。
- 项目许可证为 MIT；新增依赖必须兼容 MIT 分发，并在发布物中提供第三方许可证清单。
- 不扫描磁盘，不读取文件夹内部文件，不上传路径或搜索内容，不默认启用遥测。
- “删除”始终只删除快捷记录，绝不删除真实文件夹或其中内容。
- `task/` 是本地工作计划目录，必须保持不被 Git 追踪。实现前先阅读其中任务文档并更新本地状态，但不得强制添加到 Git。
- `review/` 和其中的 review/response/re-review 文档属于发布审计证据，必须被 Git 追踪。

## 2. Agent 职责与交接

每项工作必须明确角色：

- **implementation agent**：实现或修改功能、补充测试、触发 CI、回应评审意见。
- **code-review agent**：只评审，不得兼任同一轮所评代码的 implementation agent。
- 同一个 agent 不得既实现某项变更，又批准该项变更发布。即使会话压缩、重启或重新命名，也不能视为身份隔离。
- implementation agent 必须提供变更范围、测试证据、已知限制和待人工验证项，供 reviewer 独立检查。
- reviewer 不得只阅读 response 后直接批准；必须重新查看相应代码、diff、测试和 CI 证据。

## 3. 强制要求一：本地 GNU 快速验证与远程 MSVC 发布验证

### 3.1 本地 Rust 验证（已授权的 MinGW64 GNU 工具链）

本机已配置 Rust `1.92.0-x86_64-pc-windows-gnu`（MinGW64）。在工具链、target、`rustfmt` 与 `clippy` 均已安装时，agent **可以**在本地执行 Rust 语法、静态检查、测试和 GNU Release 构建，以缩短反馈周期。

每次本地 Rust 验证前必须先禁止 Rustup 自动下载，并记录预检结果：

```powershell
$env:RUSTUP_AUTO_INSTALL = '0'
$env:RUSTUP_TOOLCHAIN = '1.92.0-x86_64-pc-windows-gnu'
rustc --version
cargo --version
rustup target list --installed
rustup component list --installed
```

只有预检确认已安装 `x86_64-pc-windows-gnu`、`rustfmt` 和 `clippy` 时，才可执行下列本地命令：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings
cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu
cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu
```

规则：

- agent 不得通过 `rustup`、安装包管理器或其他方式自动安装、更新、切换或下载 Rust toolchain、target 或 component；预检失败时应停止本地 Rust 验证并报告缺失项。
- 本地 GNU 结果是快速反馈证据，必须记录 toolchain、target、命令、commit/工作树状态和结果。
- GNU 与 MSVC ABI、linker、Windows SDK 和依赖行为不同；本地通过绝不替代远程 MSVC CI，也不得据此宣称发布物已验证。
- 本地构建产物只用于开发检查，不得作为 release artifact、Actions artifact 或发布哈希来源。

### 3.2 GitHub Actions 必备 MSVC 门禁

远程 Windows CI 和所有候选/发布构建必须使用 `x86_64-pc-windows-msvc` toolchain/target。workflow 中 Rust 检查、测试、Release build、EXE 路径和打包步骤必须显式保持 MSVC target 一致；主 agent 在每次推送、workflow 修改和 release-candidate 前都必须检查该约束。

项目建立后必须维护 Windows CI。至少包括：

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-msvc -- -D warnings`
3. `cargo test --workspace --all-features --locked --target x86_64-pc-windows-msvc`
4. `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-msvc`
5. 对 MSVC Release EXE 做可执行文件存在性和打包检查

发布 workflow 还必须：

- 只从通过 MSVC 质量门禁的目标提交构建 x86-64 Release 产物；
- 生成 portable EXE、ZIP 和 SHA-256 校验文件；
- 上传 Actions artifact；
- 发布前验证版本号、tag 与产物名称一致；
- 不在缺少 review 批准或手工验收结论时创建 GitHub Release。

### 3.3 使用指定 GitHub CLI

所有 GitHub CLI 调用必须使用完整路径：

```powershell
& "D:\Program Files\GitHub CLI\gh.exe" <args>
```

禁止假定 `gh` 已在 `PATH` 中。常用监控流程：

```powershell
& "D:\Program Files\GitHub CLI\gh.exe" workflow list
& "D:\Program Files\GitHub CLI\gh.exe" run list --branch <branch> --limit 10
& "D:\Program Files\GitHub CLI\gh.exe" run watch <run-id> --exit-status
& "D:\Program Files\GitHub CLI\gh.exe" run view <run-id> --log-failed
```

必要时下载产物：

```powershell
& "D:\Program Files\GitHub CLI\gh.exe" run download <run-id> --dir <directory>
```

### 3.4 CI 操作纪律

- CI 必须由已推送到 GitHub 的提交触发；仅本地未提交/未推送的代码不能形成有效 CI 证据。
- **纯文档推送不触发 CI（节省时间）**：`ci.yml` 与 `benchmark.yml` 的 `push` 触发器配置了 `paths-ignore`（`**/*.md`、`docs/**`、`review/**`）。仅改动这些文档文件（如 review/response 文档、README、docs）的推送**不会**触发 CI，agent 不得等待其 run、也不得以“无 run”标记阻塞或引入额外提交。仍需保证：
  - 文档本身属于审计证据，仍必须提交 Git；其正确性由 reviewer 在 re-review 时核验，不依赖托管的构建门禁。
  - **任何代码、Cargo、`src/`、`.github/workflows/` 或构建输入变更都必须触发并监控 CI**（workflow 文件不在 `paths-ignore` 内，改动会触发真实 run，符合 §3.4 “workflow 自身变更也必须通过一次实际运行验证”）。
  - 若文档变更与代码变更混在同一提交，则该提交会因代码路径而触发 CI（`paths-ignore` 只匹配“全部变更均为文档”的提交），属正常。
  - 评审证据记录时：文档-only 提交记录为“无 CI run（纯文档，按规则跳过）”，不得伪装成有 run；代码提交按下方要求正常记录 run。
- 推送、创建 tag、创建 release 等外部操作遵循用户授权；没有授权时，implementation agent 应停在“需要推送验证”并明确说明。
- 每次实现或修复代码后，定位该提交对应的 run ID，并用上述绝对路径监控到结束；纯文档提交不等待 CI。
- 若 CI 失败，必须查看失败日志、修复根因、再次触发并获得新的成功 run；不得只重跑以掩盖不稳定问题。
- 报告 CI 时至少记录：commit SHA、workflow、run ID/URL、结论、关键 job、产物名称。不得把其他提交或旧 run 的成功冒充当前变更证据。
- workflow 自身变更也必须通过一次实际运行验证。
- 若 GitHub 不可用或 workflow 未运行，状态只能是“未验证”，不得标记完成或允许发布。

## 4. 强制要求二：独立 Code Review 闭环

### 4.1 触发时点

以下时点必须启动独立 code-review agent：

- 每个实施里程碑完成、相关 CI 通过后；
- Release Candidate 冻结后、创建 `0.0.1` tag 或 GitHub Release 前；
- reviewer 要求修复后，必须由 reviewer 再评估；
- 任何影响数据安全、真实文件夹删除语义、单实例、全局快捷键、启动项、配置恢复或发布 workflow 的重大修复后。

可以多轮 review → response → re-review，直到 reviewer 明确同意 release。

### 4.2 文档目录和命名

版本 `0.0.1` 的所有记录放在：

```text
review/0-0-1/
```

每个里程碑或候选版本使用独立主题 slug，按轮次保存，不覆盖旧文档：

```text
review/0-0-1/<topic>-review-r01.md
review/0-0-1/<topic>-response-r01.md
review/0-0-1/<topic>-review-r02.md
review/0-0-1/<topic>-response-r02.md
```

`r02` review 是对 `r01` response/修复的复审。若 `r02` 仍有问题，则产生 `response-r02` 和后续轮次。文档必须提交 Git，作为审计记录。

### 4.3 Review 文档必填内容

每份 review 至少包含：

- 标题、版本、topic、轮次、日期；
- reviewer agent 标识/角色声明，以及“未参与所评代码实现”的声明；
- 被评审 commit SHA、比较范围（base/head）和文件范围；
- 对应 CI run ID/URL 与结果；
- 需求/验收标准映射；
- 按严重级排序的 findings：唯一 ID、严重级、文件与行号、问题、影响、复现/证据、建议；
- 对正确性、错误处理、数据安全、隐私、安全性、Windows 行为、测试覆盖、性能、可访问性和可维护性的检查；
- 未能自动验证的 GUI/平台项；
- 最终结论：`CHANGES_REQUESTED`、`APPROVED_FOR_MILESTONE` 或 `APPROVED_FOR_RELEASE`。

Release Candidate 的最终一轮必须明确写出 `APPROVED_FOR_RELEASE`；“看起来没问题”“LGTM”或里程碑批准不能代替发布批准。

### 4.4 Response 文档必填内容

implementation agent 必须逐条回应，至少包含：

- 对应 review 文件、实现 agent、修复前后 commit SHA；
- 每个 finding ID 的评估结论：`ACCEPTED`、`PARTIALLY_ACCEPTED` 或 `REJECTED`；
- 判断理由和证据；
- 若接受：具体修改、文件/测试、验证方式及新 CI run；
- 若拒绝或部分接受：详细技术理由、风险分析和替代保障；
- 未解决事项和待人工验证项；
- 请求 reviewer 复审的明确说明。

不得因为 reviewer 的意见“可能不重要”而省略。确认存在且应修复的问题必须由 implementation agent 修改；不修改必须给出足以复核的详细理由。

### 4.5 复审与发布硬门禁

- 必须由原 reviewer 或另一名同样独立、未参与实现的 code-review agent 复审 response、代码 diff、新测试和新 CI。
- 所有阻断性 finding 必须关闭；被拒绝的 finding 必须由 reviewer 明确接受其理由。
- `Critical`/`High` finding 未关闭时禁止发布；`Medium` 若延期，必须由 reviewer 明确接受并记录影响与后续任务；任何可能删除真实数据、破坏配置或错误发布产物的问题都不得延期。
- 最终 release review 必须针对实际候选 commit，且该 commit 与最终成功 CI、手工验收产物及待打 tag 的 commit 完全一致。
- release review 批准后若代码、依赖、UI、workflow 或产物内容发生变化，批准自动失效，必须重新 CI 和复审。纯文档变化只有 reviewer 明确判断不影响发布证据时才可豁免。
- 未获得独立 reviewer 的 `APPROVED_FOR_RELEASE`，禁止创建 tag 和 GitHub Release。

## 5. 里程碑工作流

每个里程碑严格按以下顺序执行：

1. 阅读 `task/` 中对应任务、产品约束和已有 review。
2. 明确验收标准、依赖、风险和测试计划。
3. implementation agent 实现最小完整切片并补充测试。
4. 检查 diff，确认没有真实目录删除、敏感信息、构建产物或用户数据进入 Git。
5. 提交并在获得外部操作授权后推送分支。
6. 用指定 `gh.exe` 监控该 commit 的 GitHub Actions；失败则修复并重新验证。
7. CI 全绿后，由独立 code-review agent 创建 review 文档。
8. implementation agent 创建 response 文档并按需修改。
9. 重复 CI、review 和 response，直到里程碑批准。
10. 更新本地 task 状态；不得把 `task/` 强制加入 Git。

## 6. Release `0.0.1` 额外流程

1. 冻结候选 commit 和版本号，运行完整 CI 与 Release artifact 构建。
2. 下载并记录候选 EXE/ZIP/SHA-256 的 artifact 来源与哈希。
3. 向用户提供真实 Windows 桌面手工验收清单；用户负责验证托盘、快捷键、中文 IME、多显示器/DPI、Explorer 重启、开机启动和路径异常等无法由托管 CI 可靠覆盖的项目。
4. 将用户给出的验收结果记录到 `review/0-0-1/manual-acceptance.md`；任何必测项未执行或失败都必须显式标记，失败项阻止发布。
5. 启动独立 release code-review agent，评审实际候选 commit、CI、产物和手工验收证据。
6. 通过 response/re-review 闭环取得 `APPROVED_FOR_RELEASE`。
7. 再确认 tag 指向已批准的候选 commit，才可在用户授权后创建 tag/GitHub Release。
8. Release Notes 必须声明支持的平台、数据位置、升级/回滚方式、未签名及 SmartScreen 提示、SHA-256 和已知限制。

## 7. 工程和测试原则

- 保持核心逻辑与 Slint UI、Windows API 适配层解耦，使搜索、排序、筛选、数据迁移与安全写入可单元测试。
- Windows API 调用集中在边界模块，检查每个返回值并转换为可理解错误，不向 UI 泄漏裸错误码。
- 路径按 Windows 语义处理；支持 Unicode、空格、长路径、UNC、映射盘和暂时不可访问路径。不得通过字符串小写化草率判断路径等价。
- 不可访问不等于无效或应删除；任何状态检查不得阻塞输入线程。
- JSON schema 必须有显式版本。保存采用同目录临时文件、flush/sync、原子替换和备份；读取失败保留损坏文件，并提供恢复路径。
- 导入默认先解析、校验和预览，再由用户选择合并/覆盖；覆盖前备份。不得反序列化后直接破坏当前数据。
- 日志不得记录完整用户路径、搜索词或配置内容；错误信息要可操作但保护隐私。
- 所有用户可见文本应支持简体中文和英文；不得把业务文案散落硬编码在逻辑层。
- 对搜索排名使用确定性 tie-breaker；10,000 条记录基准必须在 Release CI 或专用基准中验证，不能用 Debug 结果代替。
- 不引入网络、遥测、自动更新或超出 MVP 的功能，除非用户明确同意。

## 8. 完成定义

任务只有同时满足以下条件才可标记完成：

- 需求和验收标准已实现；
- 自动测试已覆盖关键正常路径与失败路径；
- 当前 commit 的 GitHub Actions 全绿且证据已记录；
- 对应独立 code review 已闭环并批准；
- 文档和已知限制已更新；
- 需要真实桌面验证的项目已清楚列出。

版本只有在上述条件、用户手工验收、最终 `APPROVED_FOR_RELEASE` 全部满足时才允许发布。
