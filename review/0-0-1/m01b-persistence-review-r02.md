# Review: M01-B 安全持久化（Round 02）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m01b-persistence`
- **轮次：** `r02`
- **日期：** 2026-09-22
- **Reviewer：** 独立 code-review agent（M01-B r02 复审）
- **独立性声明：** 本 reviewer 未参与 M01-B 初始实现（`dce6121..72c623b`）、r01 修复（`0317933`）或 r01 response（`4c8a232`/`f501442`）的任何代码、测试、提交、推送、CI 监控或 response 撰写；本次仅作只读复审，未编辑源代码、未运行 cargo、未提交或推送。除本 review 审计文档外，没有写入其他文件。
- **Base SHA（r01 head）：** `72c623b7207905ea9e7101bf783f89132d407cb4`
- **修复 commit SHA：** `03179339a5e01f48dc73e54ffe97dcfff8ca8f7c`
- **Head SHA（本复审对象）：** `f5014427bc5bacdbe440c7e69c46c54a9b8ea5b5`
- **比较范围：** `72c623b..f501442`（代码变更集中于 `0317933`；`4c8a232`/`f501442` 为纯文档提交，见 CI 证据节）
- **审查文件（代码）：** `src/storage/io.rs`、`src/storage/location.rs`、`src/storage/repository.rs`、`src/storage/repository_tests.rs`、`src/storage/tests.rs`（既有删除禁止扫描）。
- **审查文件（审计）：** `review/0-0-1/m01b-persistence-review-r01.md`、`review/0-0-1/m01b-persistence-response-r01.md`。
- **审查方法：** 完整阅读 head 上 `repository.rs`/`io.rs`/`location.rs`/`repository_tests.rs` 当前代码；核对 `72c623b..0317933`、`72c623b..f501442` 的 diff 与统计；独立核对 r01 的每项 finding 在代码中的实际实现与测试字节级断言；通过指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询 r01 修复后的 CI run 及 job 日志；未以 response 摘要代替代码核验。

## CI 证据

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35726458488](https://github.com/yorelll/filego/actions/runs/35726458488) | `f5014427bc5bacdbe440c7e69c46c54a9b8ea5b5` | `Windows CI` / `fmt, clippy, test, release, package` | `success`（completed） | 单一 job 全步骤 success：`Check formatting`、`Run Clippy with warnings denied`、`Run tests`、`Build release executable`、`Verify release outputs and version helper`、`Audit advisories…`、`Generate third-party license inventory`、`Build portable development artifact`、`Upload portable development artifact`。仓库的 MSVC 门禁 workflow（`.github/workflows/ci.yml:22,39,83-96`）显式使用 `x86_64-pc-windows-msvc`。job 日志实测 test step：`test result: ok. 69 passed; 0 failed`（lib）+ `1 passed`（main）= **70 项通过，0 失败**，与 response 本地 GNU 记录的口径（lib 69 + main 1）一致。 |
| [35726133222](https://github.com/yorelll/filego/actions/runs/35726133222) | `4c8a23235b0d05a591008d9b9f801691487dbe1c` | `Windows CI` | `cancelled`（completed） | 该 run 在 response 文档中被记录为提交时刻的 in_progress 状态（诚实透明，未宣称成功）；随后被 head `f501442` 的 run `35726458488` 取代而取消。因 `f501442` 相对 `4c8a232` 仅修改 response 文档（`git diff 4c8a232 f501442 --stat` 仅 1 文件 `review/0-0-1/m01b-persistence-response-r01.md`），源码与构建输入完全一致，故被取消 run 的取消无信息损失：它以相同代码着色，权威结论以下方 `35726458488` 的 `success` 为准。**两 run 均如实记录，无掩盖。** |
| [35695909584](https://github.com/yorelll/filego/actions/runs/35695909584) | `72c623b7207905ea9e7101bf783f89132d407cb4` | `Windows CI` | `success` | r01 已记录；保留用于 r02 对照，不作为本次修复的证据。 |

- **CI 结论：** 修复后的 head `f501442` 通过 MSVC 门禁（fmt / clippy `-D warnings` / test 70 项 / release build / EXE 与版本核对 / license audit / portable artifact 打包上传）。此证明本 head 在该环境可构建、可测试、可打包；不证明断电元数据可持续性、真实双进程竞争、真实 Windows 句柄拒绝/杀毒占用等未覆盖场景。
- **备注（记录透明性）：** response 文档 `m01b-persistence-response-r01.md` 的 CI 表格最终回填的是被取消的 run `35726133222`（其本来状态为 in_progress；后由 implementation agent 在 `f501442` 提交的该字段仍指向该 run）。真正结论性 run `35726458488` 未在 response 表格中单独记录。这是**文档回填不完整**，非证据造假——两 run 均来自 `gh` 查询且 head SHA 可核验；本 review 已如实补录权威 run。不构成新的代码 finding。

## 变更范围核验（无 scope creep）

`git diff 72c623b..0317933 --stat`：

| 文件 | 变更 |
|---|---|
| `src/storage/io.rs` | +171/-（重写，含 `WriteLock`、`exists` 契约、`fault` 模块） |
| `src/storage/location.rs` | +44（`BACKUP_TEMP_FILE_PREFIX`/`CORRUPT_EVIDENCE_PREFIX`/`LOCK_FILE_NAME` 及派生函数） |
| `src/storage/repository.rs` | +400 |
| `src/storage/repository_tests.rs` | +813 |

`Cargo.toml`、`Cargo.lock`、`codec.rs`、`schema.rs`、`domain/*`、UI、`.github/workflows/ci.yml` 均**未变**（`git diff 72c623b..f501442` 额外仅含两份 review 文档）。与 response 宣称范围一致，**无 scope creep**。

## 测试计数核验（对 response 声称的复核）

- head 上 `src/storage/repository_tests.rs` 含 **33** 个 `#[test]`；r01 base `72c623b` 为 20 → 净增 **+13**。
- 修复 diff 新增 14 个测试函数、移除 1 个旧测试（`save_fault_backup_copy_error_preserves_main_and_backup` 被 `save_fault_backup_staging_preserves_previous_backup` 取代；旧 `save_fault_rename_error_preserves_main_and_backup` 更新 step 索引），净 +13。新增 14 个测试函数逐一存在且命名与 response 陈述一致（见 F001–F005 各节证据）。
- response 的「新增/更新 16 项」应按「新增 14 + 替换/更新 2」理解，与实际的 14 新增 / 1 替换 / 1 更新相符；表述略宽，属范围口径差异，不构成数据不实。
- `#[allow(...)]` 在 `src/storage/` 下**零出现**（grep 确认）。
- 错误类型 `RepositoryError` 保持匿名 Display（`repository.rs:112-142`），不携带路径/JSON/文件夹内容；`repository_error_display_contains_no_stored_content` 测试覆盖。
- 删除不变量：`src/storage/tests.rs` 的递归扫描测试 `source_under_storage_and_domain_has_no_fs_delete_api_calls` 与 `domain_does_not_expose_real_directory_delete_api` 全局禁止 `remove_dir`/`remove_dir_all`/`delete_directory` 且限制 `remove_file` 只出现于 `io.rs`；`remove_folder` 仅对内存 `Vec::retain`。无真实目录删除面。

## r01 → r02 逐 finding 处置

### M01B-R01-F001 — 状态：**CLOSED**

- **代码证据（`repository.rs`）：**
  - `pending_recovery: bool` 字段（`177`），`load` 在 main 缺失/损坏 + backup 可解码时置真并返回 `LoadOutcome::Recovered`（`405-416`）。
  - `save`（`243-248`）与 `save_if_current`（`259-266`）**入口最先**调用 `reject_if_pending_recovery()`（`215-221`），待修复期返回 `RepositoryError::RecoveryRequired`，**先于** `codec::encode` 与 `acquire_write_lock`，任何写路径（encode 触碰磁盘或 lock/rename）都不会执行。
  - `repair_from_backup`（`464-520`）：(a) 非 pending → `HadNoCorruptMain`（`465-467`）；(b) 重读 main，现在可解码 → 清 pending + 刷新 revision，`HadNoCorruptMain`（`478-485`）；(c) main 仍损坏/缺失时，backup 必须可解码，否则 `CorruptData` 且 main/backup 原样保留（`489-498`）；(d) main 存在且损坏 → **先**把损坏字节写入独立证据文件 `data.json.corrupt-<uuid>`（`write_flush_sync`＝write_all+flush+sync_all，durable，`502-510`），**然后**经 temp 写入 → rename → best-effort sync 提升 backup 到 main（`514`，`promote_bytes_to_main` `524-532`）；(e) 清 pending + 更新文档/revision（`516-519`）。
- **测试证据（字节级，`repository_tests.rs`）：** `recovered_pending_save_is_rejected_and_preserves_main_and_backup`（`385-412`，断言 main==corrupt_bytes、backup==原编码字节、revision 仍为 6）；`recovered_pending_save_if_current_is_rejected_and_preserves_main_and_backup`（`417-439`）；`repair_from_backup_preserves_evidence_and_repairs_main`（`444-510`，断言证据文件同目录、名 `data.json.corrupt-`、字节等于 corrupt_bytes、main/backup 解码为恢复文档且 backup 字节不变、pending 清除、后续 save 成功、证据未被清理）；`repair_from_backup_with_missing_main_recreates_main`（`516-543`，无证据文件、main 重建、save 复用）；`repair_from_backup_rejects_absent_or_invalid_backup_and_preserves_both`（`550-643`，拒绝后双文件原样、无证据写出、pending 保持）。
- **结论：** r01 与之相关的全部验收点（不自动覆盖损坏证据、不破坏唯一有效副本）已闭环。**CLOSED。**

### M01B-R01-F002 — 状态：**CLOSED**

- **代码证据：** `on_disk_revision` 返回显式枚举 `OnDiskRevision::{Missing, Corrupt, Ok(u64)}`（`282-295`，`592-597`）：仅 `NotFound` → `Missing`；可读不可解码 → `Corrupt`；其他读错误 → `RepositoryError::Io`。`save_if_current` 在锁内对 `Missing` 放行（bootstrap 新建），`Corrupt` → `CorruptData`（`268-269`，**绝不伪造 expected revision**），可读但不等于 expected → `ConcurrentModification`（`270-272`）。
- **测试证据：** `save_if_current_with_corrupt_main_returns_corrupt_data_and_preserves_both`（`649-680`，未 load、corrupt main + 有效 backup，任意 expected 均 `CorruptData`，main 与 backup 字节均保留）。既有 revision 测试（`1085-1173`）保持。
- **锁内性：** `on_disk_revision()` 在 `acquire_write_lock()` 之后调用（`266-267`），状态检查确在锁内执行。
- **结论：CLOSED。**

### M01B-R01-F003 — 状态：**CLOSED（含并发契约的界定说明）**

- **代码证据：** `FileOps::create_new`（`io.rs:84-86`）→ `WriteLock`（`io.rs:97-126`），RAII drop 尽力删除锁文件；`acquire_write_lock`（`repository.rs:227-236`）把 `AlreadyExists` 映射为 `ConcurrentModification`，其余 → `Io`。`save`/`save_if_current` 均在编码后、任何磁盘写前获取锁并跨 `on_disk_revision` → backup → rename → cleanup 持锁（`246-247`、`266-275`）。
- **锁内状态检查已核实：** `save_if_current` 的 encode 在锁外（`265`），但 encode 是纯函数、不触盘；revision/CAS 检查（`269-274`）在锁内。故不存在「check 与 rename 之间被插入」的 TOCTOU。
- **cleanup 安全性：** `cleanup_stale_temps`（`351-364`）只在持锁时运行，仅匹配 `TEMP_FILE_PREFIX`（`data.json.tmp.`）；`data.json.corrupt-`、`data.json.bak.tmp.`（前缀 `data.json.bak.`，与 `data.json.tmp.` 在第 10 字符不同）、`data.json.lock` 均不会被匹配到。cleanup 仍使用真实 `std::fs::read_dir`（`repository.rs:352`）而非 `FileOps` seam——该 seam 绕行只影响故障注入粒度，不改变生产逻辑正确性，且让锁成为唯一的并发判定依据（归属证明），符合 F003 意图。
- **测试证据：** `two_writers_contending_save_if_current_only_one_wins`（`1334-1394`，两个真实线程上的 repository 经 `Barrier` 同时 `save_if_current`，断言恰 1 成功 / 1 `ConcurrentModification`，main 为二选一完整文档，无遗留锁文件）；`cleanup_does_not_delete_interleaved_writers_temp`（`1289-1327`，B 的 temp 在 A 释放锁后才产生，A 的 cleanup 不可见它，B 随后 save 成功）；`cleanup_never_removes_evidence_or_backup_temps_or_lock`（`1259-1280`，证据/backup-temp 幸存，锁释放）。
- **残余风险（诚实记录，已由 response 声明界定）：** 锁为 create-new 语义，崩溃后遗留 stale lock 会持续拒绝写入（`ConcurrentModification`），自动过期会破坏互斥故刻意不做；此为主观接受的运维项，已列入未解决事项。
- **结论：CLOSED。**

### M01B-R01-F004 — 状态：**CLOSED**

- **代码证据：** `FileOps::exists` 签名改为 `fn exists(...) -> io::Result<bool>`（`io.rs:37`），真实实现 `FsFileOps::exists` 用 `fs::metadata`：`NotFound → Ok(false)`、其他错误 → `Err`（`io.rs:74-82`）。`load` 的 main read：仅 `NotFound → None`，其他 → `RepositoryError::Io` 直接返回、**不**进入 backup 恢复（`repository.rs:383-387`）；`try_recover_backup`：仅 `NotFound → Ok(None)`，其他 → `Io`（`436-444`）；`on_disk_revision`：仅 `NotFound → Missing`，其他 → `Io`（`286-290`）。
- **测试证据（注入 `ReadFaultFileOps`/`ExistsFaultFileOps`，按文件名注入 `PermissionDenied`）：** `load_main_read_access_denied_returns_io_and_touches_nothing`（`746-782`，有有效 backup 也不回退、无文档被 seed、文件未被改）；`load_backup_read_access_denied_returns_io`（`786-810`，不可访问 backup ≠ 「无 backup」，main 保留）；`save_if_current_main_read_access_denied_returns_io`（`814-830`，不可读 main 不作 missing baseline）；`save_main_exists_error_returns_io_and_preserves_previous_main`（`891-914`，`exists` 错误 → `Io`，main 保留）。
- **结论：CLOSED。**

### M01B-R01-F005 — 状态：**CLOSED**

- **代码证据：** `FileOps::copy` 已删除（grep 仅剩测试注释提及 `fs::copy`）。backup 改为故障安全三明治 `replace_backup_with_current_main`（`repository.rs:335-342`）：读当前 main 字节 → 写独立 backup-temp（`write_flush_sync`=write_all+flush+sync_all）→ 原子 rename 到 `.bak`。任一失败时旧 `.bak` 字节不变；`FaultyFileOps` 提供 `BackupWrite`/`BackupSync`/`BackupReplace` 故障点（`io.rs:158-160,222-227,230-265,268-278`），其中 `BackupWrite` 诚实模拟「目标已截断 + 半写入 + 报错」的部分写失败（`io.rs:251-264`），正对 r01 F005 的点。
- **测试证据：** `save_fault_backup_staging_preserves_previous_backup`（`984-1027`），对 `BackupWrite`(step4)/`BackupSync`(step5)/`BackupReplace`(step6) 各跑一遍：save → `Io`，预先存在的 `.bak` 字节 byte-identical（`b"{\"older\"}"` 原样），main 不动。
- **结论：CLOSED。**

## 新发现（r02 新增）

### M01B-R02-N001 — Medium — `repair_from_backup` 不获取写锁，修复可与非并发契约互斥的保存竞争

- **文件/行：** `src/storage/repository.rs:464-520`（`repair_from_backup`）、`promote_bytes_to_main` `524-532`；与之对照的并发契约文档 `repository.rs:48-59`。
- **问题：** 本 slice 声称「每个写入由 `data.json.lock` 串行化」（`repository.rs:49-55`）。`save`/`save_if_current` 在锁内写；但 `repair_from_backup` 作为同样执行磁盘写入（写证据文件 + `promote_bytes_to_main` 写 main）的路径，**未获取 `WriteLock`**。其运行序列「读 corrupt main → 读 backup → 写证据 → promote 到 main」与另一个 repository 实例（后台线程或另一进程）的一次 `save` 无任何互斥。
- **影响：** 若后台 writer B（持有锁的 `save`）在 A 已读 backup、尚未 promote 之间把更新的 main 提交上去，A 随后会把（较旧的）backup 字节 promote 到 main，**静默把更新状态回退为旧版本**；B 的更新只存在于已被 A 覆盖的 main 中，backup 仍是修复前旧数据，B 的新版本即丢失。双 `repair` 竞争时则可能发生 last-wins，但因两边 promote 同一个 backup，结果一致可接受。该窗口不删除真实目录、不产生不可解码文件（main 仍为合法 JSON，只是回退版本），且证据文件与 backup 均存活。
- **可达性评估：** 0.0.1 的单进程/单 repository 实例模型下，同一实例的 `pending_recovery` 门（`reject_if_pending_recovery`）已拒绝同一实例的并发 save；本仓库的并发支持模型（`two_writers…` 测试、`Send` 上的真实线程）论证的是**两个实例/两线程**共存。在此双实例模型下，repair 确实可与 save 竞争，属于并发契约声明与实际实现之间的一个真实不符点。修复代价小：`repair_from_backup` 在通过 `pending_recovery` 检查后获取 `WriteLock`，并在锁内重读 main/backup 完成全部判定（与 `save_if_current` 的锁内检查同构）。
- **处置：** **ACCEPTED（Medium，延期）**。理由：(1) 0.0.1 明确单进程/单实例，该竞争在文档化部署形态下不可达；(2) 即使发生，结果是「回退到旧有效版本」而非损坏/不可解码，且证据与 backup 均保留，可再次 `repair_from_backup` 恢复，不构成数据不可恢复；(3) 与 stale-lock 处理同属「多 writer 完整互斥」的下一 slice 事项，本 slice 已把跨进程完整互斥列为未解决事项。按 CLAUDE.md §4.5，Medium 延期由本 reviewer **明确接受**并记录影响与后续任务：**后续任务 M01B-N001-followup = 在 `repair_from_backup` 中获取 `WriteLock` 并把 main/backup 的读与判定纳入锁内；STALE_LOCK 恢复一并处理；增加「repair 竞争 save（barrier 双线程）」回归测试。** 此项不阻断 MISS 里程碑批准，但在任何 release-review 前必须关闭。

<aside>

补：上述竞争与 r01 F003 的旧 cleanup 竞争不同——F003 是在「两个普通写入」之间，本 slice 已用锁串行化；N001 是「修复」这一个新 API 脱离锁。二者不重叠。

</aside>

## 需求/验收标准映射（r02）

| M01.4 验收点 | 证据（head） | 结论 |
|---|---|---|
| 主配置损坏时恢复且保留证据、不自动覆盖 | `pending_recovery` + `repair_from_backup` + 证据文件（`repository.rs:177,464-520`）；`load` 保留 corrupt main（`405-416`）；字节级测试（`repository_tests.rs:385-643`） | **满足** |
| concurrent/陈旧 revision 拒绝或串行化 | 锁覆盖 check→backup→rename→cleanup（`246-247,266-275`）；`two_writers…` barrier 测试（`1334-1394`） | **满足（单实例/尽力而为跨进程；stale-lock 与 repair-竞争见 N001 与未解决事项）** |
| 故障注入覆盖主要失败点 | `FaultyFileOps` 含 LockAcquire/TempWrite/TempSync/MainExists/BackupWrite/BackupSync/BackupReplace/ReplaceRename/StaleTempRemove（`io.rs:148-168`）；矩阵测试（`repository_tests.rs:920-1079`） | **满足** |
| 半写 JSON 与遗留 temp 处理安全 | temp 同步后 rename（`309-316`）；cleanup 仅持锁且只删 `data.json.tmp.*`（`351-364`） | **满足** |
| 不可访问 ≠ 无效/应删除 | `exists`/`read` 错误分类（`io.rs:74-82`；`repository.rs:286-290,383-387,436-444`）；注入测试（`repository_tests.rs:747-914`） | **满足** |
| 「删除」只删记录、不删真实目录 | `remove_folder` 内存 retain（`537-547`）；递归源扫描（`tests.rs`） | **满足** |
| reset 先备份/恢复动作 | 不在本 slice（response 与 r01 一致） | 后续里程碑项 |
| `%LOCALAPPDATA%\FileGo` 生产解析/首次建目录/UI 恢复 | 不在本 slice（`location.rs` 仍为注入 base_dir） | 后续 M03/M05 |

## Cross-cutting 检查

- **正确性：** 正常 save/load、Unicode round-trip、previous-main backup、invalid-data 不落盘均保持；F001/F002 的损坏链、F003 双写互斥、F004 错误分类、F005 部分写失败均有字节级/barrier 级测试。
- **错误处理：** `RepositoryError` 六种变体匿名 Display（`112-142`）；`Io` 拆解底层细节；`sync_path` 后的 best-effort 错误被有意忽略（`321`、`530`），模块文档注明「不把逻辑成功变为伪失败」，可接受。
- **数据安全：** 恢复契约（pending 期拒绝一切触碰 main/backup 的写）+ 证据文件先于 promote 落盘 + backup 故障安全三明治，形成对 r01 三处数据风险的完整闭环。post-rename `sync_path` 的断电窗口仍不能由单元测试证明（见未自动验证项 2）。
- **隐私：** 持久化边界不记录用户路径/JSON；Display 固定文案；测试 fixture 含敏感标记且 `repository_error_display_contains_no_stored_content` 覆盖。符合约束。
- **安全性：** 无网络/遥测/自动更新；cleanup 只 `remove_file` 且限 `data.json.tmp.*`；删除不变量由源扫描测试强制。两项安全相关残余均已在类型/文档层面声明并列入未解决事项（stale-lock 手动移除、跨进程完整互斥下一 slice，N001）。
- **Windows 行为：** 同目录命名满足同卷 rename；`File::create_new` 独占语义在 Windows 有效；CI 已在 MSVC 运行且 70 项测试通过。共享冲突、杀毒占用、真实双进程、崩溃后锁释放仍属未自动验证项。
- **测试覆盖：** `repository_tests.rs` 33 项（20→33，净 +13），含全部 finding 的回归与字节断言；无 `#[allow]`。CI job 日志确认 70 项全部通过（非仅编译）。
- **性能：** 每次 save 的 `read_dir` 遍历与锁读写为常数级目录开销；`FileOps` 请求均在锁内串行，无额外复杂度；未引入网络或遥测路径。
- **可访问性：** 无 UI 改动，无新增 a11y 结论。
- **可维护性：** `FileOps` seam + `fault` 模块 + 显式命名常量（`location.rs:14-30`）使并发、恢复、backup 各语义可独立测试；cleanup 的 `read_dir` 绕行 seam（`repository.rs:352`）是已知折衷，不改变生产正确性。
- **依赖与许可证：** 依赖树未因修复变化（Cargo.toml/Cargo.lock 未动）；CI license audit 与 THIRD_PARTY_LICENSES.html 生成 step success。MIT 约束无冲突证据。

## 未能自动验证的项

1. 修复后需在真实 Windows 上验证：跨进程排他锁行为、崩溃后锁释放与 stale-lock 手工恢复、杀毒/同步软件/共享冲突下的错误分支、不会删除另一进程 temp、后台 repair 与 save 竞争（N001 的后续任务）。
2. 断电/崩溃边界：backup-temp、main rename、post-rename durability 的真实 crash 窗口需故障注入/受控关机实验；单元测试（含 CI 的 70 项通过）不证明断电文件系统元数据持久性。
3. `%LOCALAPPDATA%\FileGo` production 解析、首次目录创建、权限错误 UI 提示与恢复动作属 M03/M05。
4. 无 GUI；托盘、IME、DPI、多显示器等桌面验收不在本持久化代码范围。

## 最终结论

`APPROVED_FOR_MILESTONE`

r01 的 4 个 High（F001–F004）与 1 个 Medium（F005）经独立逐条代码与测试核验，全部 **CLOSED**；修复在 `0317933` 落实，head `f501442` 的 MSVC CI（run `35726458488`）全绿（fmt / clippy `-D warnings` / 70 项测试 / release build / 打包），被取消的 run `35726133222` 已如实记录（代码与 head 相同，无证据损失）。

新增 **1 个 Medium finding（M01B-R02-N001）**：`repair_from_backup` 未获取写锁，可在双实例/后台线程共存模型下与一次 `save` 竞争并把 main 静默回退到旧 backup 版本。本 reviewer **明确接受其延期**至后续 slice（理由与影响如上），后续任务 `M01B-N001-followup` 必须在本里程碑关闭后、任何 release-review 前完成并重新 CI + 复审；在发布硬门禁语境下 N001 不得被视为已豁免。

本文不构成发布批准；`APPROVED_FOR_RELEASE` 需要后续 Release Candidate 的独立发布评审、真实桌面手工验收与对应响应闭环。
