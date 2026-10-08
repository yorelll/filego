# FileGo 0.0.1 — B/C 手工验收整改独立代码复审 r03

- **版本 / topic / 轮次 / 日期**：`0.0.1` / `manual-bc-remediation` / `r03` / 2026-10-08。
- **Reviewer / 独立性**：独立 code-review agent；**未参与**被评 `caa671e`、`486c1e4`、`2b2a08c` 的实现；仅评审，不实现修复、不代项目用户填写验收、不批准发布。
- **被评代码及范围**：base `9921c7bb3442e8654b624f67e266df8dfb9389b1` → head `2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`。逐文件核对完整 `git diff 9921c7b..2b2a08c`（`git diff --check` 通过）：`src/main.rs`、`src/platform/windows/tray_open.rs`、`src/presentation/{i18n,startup_registration}.rs`、`src/storage/{location,repository,repository_tests}.rs`；并独立检查现行 `src/storage/{io,backup}.rs`、`src/presentation/{settings_controller,manager}.rs`、相关 UI 与 MSVC workflows。参考 `manual-bc-remediation-review-r02.md`、`manual-bc-remediation-response-r03.md`、本地 `task/03-发布手工验收清单.md` 与 `CLAUDE.md`。本地 HEAD `753172b` 只新增 response 文档，按文档-only `paths-ignore` 无新 CI；此代码复审的 CI 对应 `2b2a08c`，不冒充 HEAD 的新 run。Reviewer 未运行本地 GNU/真实桌面验收。
- **用户验收记录**：`review/0-0-1/manual-acceptance.md` 属项目 owner 的**未暂存改动**，未修改/暂存；候选仍是旧 `a16d28b`。B7/C5 FAIL，C1/C2/C3 部分通过（其结果词并非模板的标准 PASS），D+ 空白，owner 尚无针对本代码 SHA 的新 RC 手工结论。不得将本轮开发包认作已验收的 RC。
- **精确 SHA 远程证据**：经指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 独立查询，均 `headSha=2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`、`completed/success`：[`Windows CI` run 37774121293](https://github.com/yorelll/filego/actions/runs/37774121293)，job `fmt, clippy, test, release, package` 成功，Rust 1.92.0、显式 `x86_64-pc-windows-msvc` Clippy/test/Release build、421 lib 通过 + 1 ignored、10 bin 通过、MSVC Release EXE/版本/许可证/便携**开发**包检查成功，artifact `FileGo-0.0.1-windows-x86_64-2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`；[`Search benchmark` run 37774121336](https://github.com/yorelll/filego/actions/runs/37774121336)，job `release 10k benchmark` 成功，artifact `search-benchmark-log-2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`。Hosted MSVC Release medians：pinyin-heavy 64.91 ms、filtered-with-clone 57.86 ms、english-initials 52.50 ms、edit-distance 61.73 ms、multi-token 50.34 ms；都低于 CI 的宽松 500 ms 门槛，但多项超过产品 <50 ms 目标。未生成/核对同 SHA 正式 RC EXE/ZIP/SHA-256，不能宣称发布性能或 artifact 已验收。

## 需求 / 旧 finding 对照

| 要求 / 旧 ID | 独立核验 | 结论 |
|---|---|---|
| **BC-R02-H01 High**；B7、K7/K8：已载入健康主文件后在运行中变坏、有效内部 `.bak` + 选定命名备份，不抹坏证据 | `src/main.rs:1339-1359` 改为只传选定的平面备份名；`src/storage/repository.rs:550-615` 在单一写锁内读、校验选定快照及主文件，先同步保存坏原件 `data.json.corrupt-*`，提升时不覆盖 `.bak` 或命名备份。健康主文件仅在已观察到的 revision **及原始 bytes 均相同**时允许显式替换，且先存 `data.json.pre-restore-*`；已载入后主文件缺失拒绝，未加载且主文件健康拒绝。`repository_tests.rs:1536-1779` 的实盘测试证实读权限拒绝、命名备份失效、健康 revision/同 revision bytes 冲突、丢失主文件及 3/4/5 三个**实际提升**故障均保留来源和证据。 | **原问题的“命名备份恢复入口”已关闭**；但同一坏主文件可经**普通 UI 保存**覆盖有效 `.bak` 且不留 evidence（`BC-R03-H01`）。因此 B7/K7/K8 的完整数据安全硬门禁**仍未关闭，High 阻断**。锁仅保护协作写者；不声称防无锁并发者在检查与 rename 之间改文件。 |
| **BC-R02-M01 Medium**；C3.6/J1–J6：已有不同 portable EXE Run 值不能因开/关或失败回滚被误删 | `src/platform/windows/tray_open.rs:54-163,171-246` 读取 HKCU 固定值的原始类型 + bytes，区分不存在/存在，限制超长值，编码本 EXE 的有引号 NUL 结尾 UTF-16 `REG_SZ`，失败可按原类型/bytes 精确写回并重读。`src/presentation/startup_registration.rs:46-113` 对启用**和禁用**都先拒绝外来值，拒绝前无 JSON/HKCU 写；其余路径先存 JSON，OS 写后严格核验，失败两侧回滚、不假报成功。`startup_registration.rs:280-495` 的 fake 覆盖 foreign type/bytes、own/absent、部分写、复读失败和回滚失败；`src/main.rs:228-248,753-775,3446-3460,4009-4015` 两个切换入口同走事务。 | **代码层面关闭旧 finding**；与另一进程同时无锁写 HKCU 不具备原子 compare-and-swap，不能断言绝对竞态隔离。fake 未写真实 HKCU，也没有以真实旧 EXE `REG_SZ` 的原始值做 Windows 登录测试；owner 须实测旧 portable、Unicode/空格、开/关、重登、故障/权限。 |
| r02 H03/M01/M02：前台/白屏、设置滚动/表单、空备注实盘保存 | 此次七文件 diff 没有更改 Slint 结构、前台 API 或原来的计时器/保存测试，MSVC 编译及单测只证明相关代码仍可构建；`repair_from_backup` 的 `HadNoCorruptMain` UI 现同步 settings/manager/search (`main.rs:1376-1383`)。 | 不能把 request-redraw/Timer、源码 shape guard 或 headless 测试说成真实首帧、焦点、滚轮及缩放已 PASS；另见 `BC-R03-M01` 的无 pending 修复假成功。 |

## Findings（按严重级）

### BC-R03-H01 — High，阻断 — 普通 UI 保存仍可覆盖唯一有效内部备份并抹掉坏主文件证据

- **位置**：`src/storage/repository.rs:980-999` (`save_at`) → `:309-352` (`save_locked`、`replace_backup_with_current_main`)；生产入口 `src/presentation/settings_controller.rs:716-743,368-370`，`src/presentation/manager.rs:376-390,1400-1419`；相关测试仅覆盖显式恢复 `src/storage/repository_tests.rs:1624-1656`。
- **复现/证据**：正常启动或 `repo.save()` 后内存持有健康 document，磁盘曾有一份有效且与内存版本不同的 `data.json.bak`；应用运行期间另一不遵守 `data.json.lock` 的程序/磁盘故障把 `data.json` 改成不可解码字节；不按“恢复备份”，而是正常 UI 改一个设置、添加/编辑记录、切换收藏等。`SettingsSharedStore` / `SharedStore` 均直达 `save_at()`；`pending_recovery` 仍为 `false`（只在 `load()` 发现坏主文件时设置），`save_at()` 不检查真实主文件的可解码性、revision 或已观察原始 bytes。锁内 `save_locked` 先写新临时文件，`exists(main)` 为 true，`replace_backup_with_current_main()` 将**损坏字节**同步写入并 rename 覆盖原有效 `.bak`，最后以过期内存的新版本 rename 覆盖损坏主文件，未产生 `data.json.corrupt-*`。即使最后的主文件 rename 出错，`.bak` 也已被坏字节覆盖；任意健康外部新 revision/同 revision 不同 bytes 同理可被普通保存无冲突地覆盖。此可由真实 GUI 的普通用户操作触发，**不是只通过直接调用仓库 API 的理论路径**；`response-r03` 明确承认普通 `save_at` 未修。
- **影响**：把仍可恢复的内部前一修订和坏主原件同时静默丢掉；与项目“坏原件保留/备份可恢复”、K7/K8 及 r02 High 的绝对数据安全要求冲突。命名备份分支修好不能豁免所有普通保存的证据擦除，禁止据此批准整改或发布。
- **建议**：在 `save_at` 的**同一写锁内、任何 temp/backup 替换前**读取并分类实际主文件：已载入健康状态下损坏则拒绝普通保存、保持 `.bak` 与坏主原样（可引导明确修复），健康但 revision 或原始 bytes 不匹配、已载入后主文件缺失则拒绝/要求冲突处理；`save` 的正常路径也应遵守坏原件/备份不覆盖不变量，同时保持首次无主文件安全创建。增加真实仓库测试：load → 建立不同的有效 `.bak` → 无锁把主文件写坏 → `save_at`（模拟普通 settings/manager 操作）→ 检查返回错误、主文件仍为坏原字节、`.bak` 逐字有效且没有假成功；另测 rename/backup 故障以及健康外部 revision 和相同 revision 的 bytes 修改。实施后按该代码 SHA 重跑 MSVC CI/benchmark 并独立复审。

### BC-R03-M01 — Medium — 非 pending 状态点击“修复内部备份”可宣告成功却不检查/修复运行中损坏

- **位置**：`src/storage/repository.rs:476-479,490-542`；`src/main.rs:1366-1388`，UI 按钮 `ui/app-window.slint:1772`。
- **复现/证据**：同样先健康加载且已有有效 `.bak`，运行中磁盘主文件损坏，点 Data 页“修复内部备份”。`pending_recovery == false`，`repair_from_backup` 在锁/复读**之前**直接返回 `HadNoCorruptMain`，不提升备份也不生成证据；新 UI 分支无条件刷新并显示 `BackupRestoreApplied`。再次启动才可能由 `load` 识别坏主、进入 pending。即使主文件仍健康、没有需要修复的内容，按钮也显示“已恢复”。旧 r02 要求的“外部已修复主文件”成功同步仅当 **此前 pending=true 且锁内实际复读到健康主文件**时有确切依据；非 pending 的相同枚举值不具此含义。新测试未覆盖此 UI/仓库分支。
- **影响**：B7/K7/K8 用户可能以为恢复已完成，留下坏数据直到重启；继续普通 UI 保存还可能触发 `BC-R03-H01` 的证据丢失。属于误导性恢复反馈，需在候选验收前修复。
- **建议**：区分“实际修复 / 外部已修复 / 无需或无法修复”结果；点击修复时安全地检查当前磁盘状态并在需要时走相同的锁内证据保护修复流程，或明确告知必须重启/重新加载且绝不报告应用成功。补充健康加载后磁盘损坏与无 pending 的仓库及 UI 映射测试；仍须 owner 实测。

## 横向审查 / 验证边界

- **正确性、错误处理及数据安全**：新命名恢复锁内校验选定快照、健康主文件字节冲突、坏文件 evidence 和故障矩阵是实质改进；真实 `FaultyFileOps` 在本路径中 0=lock、1/2=evidence write/sync、3/4=promotion temp write/sync、5=rename，测试确实命中提升而非旧 r02 的误注释位置；主文件缺失且已有健康 load 明确拒绝。`repair_from_backup` 的 pending=true、外部已修复分支更新仓库且 UI 刷新正确；不能推广到非 pending 早退。两项新 finding 使完整数据安全要求尚未达标。新增恢复路径仅操作配置主文件/备份/evidence/temp；真实文件夹删除逻辑仍是 `remove_record` 内存记录过滤，`clear_all_records` 仅清记录，未见本次 diff 引入实际目录删除或递归扫描；真实 canary 仍须人工检查。
- **Windows / 隐私 / 安全性**：HKCU 的 subkey/value 固定、两次查询的 8192-byte 上限及类型+原始 bytes 快照保留；`PCWSTR` 的键名宽字符缓冲在所有相应 Win32 调用期间存活，读成功/错误分支关闭 key，开/关错误匿名且无直接将路径/搜索词/配置内容写日志；`RunValueSnapshot` 持有敏感 bytes 但目前未被格式化到日志。Win32 注册表读写权限、登录行为、双 portable 同时竞争及真实软件渲染都不能由 mock/MSVC 构建证明。若非协作进程恰在注册表快照与本应用写入之间改值，当前 Win32 调用没有 CAS，可能抢占其新值；同样 FileGo 数据锁只防协作写者。此竞态作为边界记录，不得称原子保护所有外来写者；**事先存在**的不同 portable 值在本次两入口的启用/禁用均拒绝修改。
- **测试、性能、可访问性、可维护性**：MSVC CI 全绿是当前 SHA 的自动门禁，不是桌面 PASS；本 reviewer 未运行额外 GNU，也未触碰用户 HKCU。测试新增真实磁盘 named-restore 及 registry fake，但缺普通保存磁盘运行中损坏、非 pending 修复和真实 Windows HKCU/COM/focus/帧/滚动。性能日志多组中位数超产品目标，CI 用 500 ms 回归阈值不能据此验收真实机器 <50 ms；对搜索以外的 UI/恢复运行时性能没有测量。恢复 enum `HadNoCorruptMain` 混合“已外部修复”与“未经检查无需修复”，增加错误处理/维护歧义。高 DPI、键盘/Tab、外层滚动条、中文 IME、屏幕阅读器、高对比需人工逐项验证；没有证据推断无障碍 PASS。无新增 Cargo 依赖、网络/遥测/路径上传或发布 workflow 变更。

## 交接与真实桌面复测

1. Implementation agent 分别回应 `BC-R03-H01`/`BC-R03-M01`，修代码及真实仓库失败测试；推送后取得**新代码 SHA** 的 Windows MSVC CI 和 10k benchmark 成功 run，交独立 reviewer 复核实际 diff、run 及 response。保留 owner 未暂存验收记录，不混入审计提交。
2. 主 agent 冻结**新**候选 SHA，以发布候选 workflow 生成未签名 portable EXE、ZIP、SHA-256，下载核对来源、版本、名称、哈希；当前同 SHA `Windows CI` artifact 仅**开发包**，旧 `a16d28b` RC 验收不可转用。
3. 请 owner 用该新候选在真实 Windows 桌面复测 B7/K7/K8：正常健康载入后让主文件损坏并保留不同的有效 `.bak` + 命名快照，分别尝试普通保存、内部修复、命名恢复、权限/锁/提升失败，重启对照原始坏 bytes/evidence/`.bak`/快照/数据；试健康主文件外部改成新 revision、相同 revision 不同 bytes 与缺失主文件，不要把单测当 owner PASS。C3.6/J1–J6 检查旧 portable Run 值在启用与禁用后都未改变、精确回滚/匿名错误、中文与空格 EXE、注销/重登。C1/C3.1/C3.3/C5 在 tray 隐藏区和菜单多次检查**真正前台与焦点、首帧/重复打开的白屏/部分绘制**，而非仅任务栏按钮；C2 请 owner 补双击异常步骤；C3.2 测 picker Cancel；C3.4 测 760×520、长列表、125/150/200% DPI/字体缩放、滚轮/scrollbar/Tab 可达 Save 和草稿页归属；C3.5 测空备注新增及重启持久性，B6/C3.7 请 owner 复看 About。继续 D+、Explorer 重启、热键、IME、多屏/路径异常、真实目录 canary、真实机器 10k 性能及资源测量；所有未执行项目显式标注而非推断通过。
4. 这只是 B/C **代码整改复审**。同一最终候选 commit、成功 CI、核哈希的 RC artifact、owner 手工结论与最终独立 `APPROVED_FOR_RELEASE` 都尚未齐备；禁止 tag 或 GitHub Release。

**最终结论：`CHANGES_REQUESTED`（整改里程碑；不是发布批准）。** `BC-R02-H01` 的具名恢复入口已关闭但整体无证据丢失要求被 `BC-R03-H01` 阻断；`BC-R02-M01` 的事先存在不同 portable 注册值处理已在代码层面关闭，真实 HKCU/登录仍待手测；`BC-R03-M01` 为另一个待修恢复反馈问题。本 review 文档应独立纳入 Git 审计，不得同时暂存项目 owner 的 `manual-acceptance.md`。
