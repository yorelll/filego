# RC `0.0.1` 候选修复与产物清单（M08.1 阶段）

## 背景

M08.1 RC 冻结期间发现发布阻断项：最初候选 EXE 虽经 `strip = "symbols"`，仍残留 **PE Debug Directory**（CodeView PDB 记录 + VCFeature/POGO）与 **523 条构建机绝对源码路径**（`C:\Users\runneradmin\.cargo\...`、`D:\a\filego\filego\...`），违反 "no debug symbols / no absolute host paths" 的 RC 产物标准。ZIP 本身不含 .pdb，但 EXE 内嵌调试记录与路径不可接受。

## 修复工作流提交（feature/m00-foundation）

| Commit | 内容 |
|---|---|
| `315b075` | release 文档（README/release-notes/manual-acceptance 模板）+ 工作流加 remap-path-prefix + 初始 PDB 守卫（过严，随后修正） |
| `7f71481` | 允许 target 内瞬时 PDB（仅禁止打包），修正守卫语义 |
| `840fed4` | llvm-tools-preview 组件 + `--remap-path-prefix=$USERPROFILE=/build-user` + 动态定位 `llvm-objcopy/llvm-objdump` + `Strip and verify` 步骤 |
| `b8e9d0a` | **PE 调试目录清理器**：解析节表，清零每个 IMAGE_DEBUG_DIRECTORY 与指向载荷、清目录索引 6、重写并复验（llvm-objcopy 只删调试节、不删 directory——真实缺陷，已修而非掩盖） |
| `a16d28b` | `--version` smoke 改用 .NET ProcessStartInfo（避免 pwsh 对 GUI 子系统 exe 关管道导致 `os error 232`） |
| `c0b0130` | manual-acceptance.md 仅填候选身份表（SHA/run/artifact/hash，artifact 核验事实，**不含验收结果**） |

## 权威 RC run

- **Run** `35981840362` — workflow `Build release candidate` — **success**
- **候选 head** `a16d28b015ac5816677f3bba961f8e89962fc119`（base `b503c0c` M07 批准头；`b503c0c..a16d28b` 仅 release 文档 + release.yml 工作流修复，无产品功能/源码变更）
- 工作流内自检输出：`PE debug directory is absent (RVA=0, Size=0)`、`No forbidden build-host markers`、`Stripped portable EXE version: FileGo 0.0.1`

## 产物清单与哈希（本地独立复算=CI SHA256SUMS）

| 文件 | SHA-256 |
|---|---|
| `FileGo-0.0.1-windows-x86_64.exe` | `e0b50242ebe5c7eb9af7638d5ac9605f1cc95852a6a750446b231e462fba6eca` |
| `FileGo-0.0.1-windows-x86_64.zip` | `7c36af3fe845eba7f263a2eb5437028904a2564049d7c92e3e63912fd6bd2b28` |

- ZIP 内容恰为 5 项：`FileGo.exe`、`LICENSE`、`README.md`、`THIRD_PARTY_LICENSES.html`、`licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`。无 .pdb/.ilk/.exp/.lib/config/token/用户数据/内部文档。
- ZIP 内 `FileGo.exe` 与独立 EXE 字节一致。
- 独立 PE 解析：`PE32+ 0x20b, debug RVA=0, Size=0`；禁止路径字节扫描（ASCII+UTF8）干净：`C:\Users\runneradmin`、`D:\a\filego`、`D:\work\tools\quickfolder`、`filego.pdb`、`VCFeature`、`POGO`、`/filego/`、`/workspace/`、`/cargo/registry/src`、`/build-user/` 均零命中。
- `--version` → `FileGo 0.0.1`。

## 失败 run（守卫/清理器确实生效的证据）

- `35977084271` — 失败于"PE debug directory 保留"（证明守卫有效，非纸面）
- `35979002478` — 通过 PE/路径检查，仅 `--version` 管道问题（随后 `a16d28b` 修复）

## 设计说明

- `target/.../filego.pdb` 保留为瞬时构建产物（允许）；**只有** portable staging / ZIP / EXE 保证无 PDB、无调试目录、无主机路径。
- 未创建 tag、GitHub Release、发布；release workflow 无写权限；manual-acceptance 未预填验收结果。

## 下一步门禁（未完成项）

1. 用户真实桌面手工验收（M08.3，结果由用户给出，不回填替标）。
2. 独立 release review 取得 `APPROVED_FOR_RELEASE`。
3. 用户授权后创建 tag `v0.0.1` 与 GitHub Release（M08.5）。
