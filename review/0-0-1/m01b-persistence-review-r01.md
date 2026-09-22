# Review: M01-B 安全持久化（Round 01）

## 元数据

- **版本：** `0.0.1`
- **Topic：** `m01b-persistence`
- **轮次：** `r01`
- **日期：** 2026-09-21
- **Reviewer：** 独立 code-review agent（M01-B r01）
- **独立性声明：** 本 reviewer 未参与 `dce6121..72c623b` 中持久化实现、测试、提交、推送或 CI 执行；本次仅作只读审查，未编辑源代码、未运行 cargo、未提交或推送。除本 review 审计文档外，没有写入其他文件。
- **Base SHA：** `dce6121aa35fe5a91738ab795d38822d66bc97b3`
- **Head SHA：** `72c623b7207905ea9e7101bf783f89132d407cb4`
- **比较范围：** `dce6121..72c623b`
- **审查文件：** `Cargo.toml`、`Cargo.lock`、`src/storage/mod.rs`、`src/storage/location.rs`、`src/storage/io.rs`、`src/storage/repository.rs`、`src/storage/repository_tests.rs`、`src/storage/tests.rs`。
- **审查方法：** 完整阅读上述当前代码与范围 diff；核对 M01.4 任务条目、既有 M01-A r02 审计记录及 Windows CI workflow；通过指定绝对路径 `D:\Program Files\GitHub CLI\gh.exe` 查询指定 CI run。未以 response 代替代码核验。

## CI 证据

| Run | Head SHA | Workflow / job | 结果 | 核验内容 |
|---|---|---|---|---|
| [35695909584](https://github.com/yorelll/filego/actions/runs/35695909584) | `72c623b7207905ea9e7101bf783f89132d407cb4` | `Windows CI` / `fmt, clippy, test, release, package` | `success`（completed） | run 与本次 head 精确一致；job 的 format、`clippy -D warnings`、MSVC test、MSVC release build、EXE/version 检查、license audit、portable artifact 打包/上传步骤均为 success。 |

- workflow 将 `TARGET` 明确设为 `x86_64-pc-windows-msvc`，并在 clippy、test、release build 和 EXE 路径中保持一致（`.github/workflows/ci.yml:22,83-96`）。
- CI 证明该提交在 MSVC 门禁中可构建和测试通过；它**不**证明以下竞争、断电窗口、损坏主文件后再次保存等未覆盖场景安全。

## M01.4 与删除不变量验收映射

| 验收点 | 实际证据 | 结论 |
|---|---|---|
| 可测试注入数据目录、main/backup/temp 位于同目录 | `DocumentPaths::from_base_dir` 保存注入路径，三个派生路径见 `location.rs:21-67`；`paths_resolve_same_dir_with_expected_names` 见 `repository_tests.rs:130-162`。 | **满足注入/同卷前提。** 生产 `%LOCALAPPDATA%\FileGo` 解析及父目录创建在本范围内不存在；`location.rs:3-7` 明确将其推迟到 M03/M05，故不能把 M01.4 的端到端目录解析标记为已自动验证。 |
| temp → 写入 → flush/sync → backup → replace | `save` 的调用顺序在 `repository.rs:131-152`；真实 temp 写入的 `write_all`、`flush`、`sync_all` 在 `io.rs:43-49`；同目录命名在 `location.rs:9-10`。 | **隔离单写入流程基本存在，但安全保证受 F001、F003、F005 限制。** |
| 故障注入 | `FaultyFileOps` 覆盖 temp write/sync、backup copy、rename（`io.rs:164-220`）；相应测试在 `repository_tests.rs:349-476`。 | **部分满足。** 已覆盖注入点在调用前直接失败；没有覆盖真实 backup 目标被截断后才发生的 copy 失败，也没有覆盖并发/损坏后保存。 |
| 半写 JSON 与遗留 temp | temp 同步成功后才 `rename`，隔离保存不会把半写 temp 提升为 main。 | **部分满足。** `cleanup_stale_temps` 无法区分旧 temp 与其他活跃 writer 的 temp（F003），故“清理策略安全”不成立。 |
| 主配置损坏时恢复且保留证据 | `load` 能从有效 `.bak` 返回 `Recovered`，且仅 load 后确实保留损坏 main；测试在 `repository_tests.rs:257-285`。 | **不满足完整要求。** 紧随普通 `save` 或 `save_if_current` 可覆盖损坏证据并毁掉唯一有效 `.bak`（F001、F002）。 |
| 并发/陈旧 revision 写入拒绝或序列化 | 有顺序执行的 revision 测试（`repository_tests.rs:501-575`）。 | **不满足。** 读 revision 与写入之间无原子 CAS/互斥；普通 `save` 无保护；cleanup 可删除并发 writer 的 temp（F003）。 |
| reset 先备份、恢复动作明确 | 任务表本身列为 M01-B 未包含。 | **不在本 slice 内；仍为未完成验收项。** |
| “删除”只删除快捷记录，绝不删除真实文件夹 | `remove_folder` 仅对内存 `Vec` 执行 `retain`（`repository.rs:277-290`）；实目录保留的端到端测试在 `repository_tests.rs:596-633`。递归源码 guard 禁止 storage/domain 的目录删除，只有 `io.rs:69` 能删除应用自己的 temp 文件。 | **本范围满足。** `remove_file` 的 temp 清理能力应仅限受并发保护的应用临时文件（F003），但不会删除目录。 |

## Mandatory adversarial safety analysis

### 1. `Recovered` 后普通 `save` 是否毁掉唯一有效 recovery copy / 损坏证据？

**结论：确认（Confirmed），High。**

1. `load` 在损坏 main、有效 backup 时返回 `LoadOutcome::Recovered`，并把 backup 的 document 置入内存（`repository.rs:191-229`，尤其 `211-215`）。
2. 随后普通 `save(recovered_document)` 并不识别恢复状态或重新验证 main；它先写入完整 temp（`131-138`），接着因为损坏的 main 仍存在，在 `141-144` 执行 `copy(main, backup)`。
3. 因此有效 `.bak` 会被损坏 main 的字节覆盖；接着 `147` 将新 temp rename 到 main。成功时原始损坏 main 证据被覆盖；若在 `143` 成功后、`147` 前崩溃，则 main 与 `.bak` 都不可解码，而 `load` 根本不检查 temp（`191-241`）。这会破坏唯一已知可恢复副本。
4. 现有 `load_corrupt_main_with_valid_backup_recovers_and_preserves_main` 只断言 **load 后** main 字节不变（`repository_tests.rs:257-285`）；没有“Recovered 后普通 save”、copy 后 rename 前故障、或保存后原证据/backup 语义的测试。

最小安全修复是：持久化恢复状态（至少标记“main 损坏且 backup 已用于恢复”），并让普通 `save`/`save_if_current` 返回 `CorruptData` 而不触碰 main、backup，直到调用一个显式的恢复/修复 API。该 API 必须先把损坏 main 保存为独立且 durable 的证据文件，保留有效 backup，之后才允许提交修复后的 main。还应增加上述三个回归测试。

### 2. `save_if_current` 面对不可解码 main 是否会接受 expected revision 并覆盖？

**结论：确认（Confirmed），High。**

`on_disk_revision` 对解码失败返回 `Ok(None)`（`repository.rs:267-275`，尤其 `274`）。`save_if_current` 再把 `None` 伪造成调用方提供的 `expected_revision`（`254-257`），所以任意传入 expected 值都会通过 `259-263` 的比较并进入普通 `save`。它随后具备第 1 项完全相同的覆盖行为；有有效 backup 时还会先把损坏 main 复制到 `.bak`。

最小修复是将“main 不存在”“I/O 不可访问”“main 存在但不可解码”建模为不同结果：只有明确 `NotFound` 才可走新建策略；不可解码应返回 `CorruptData`，非 `NotFound` I/O 应返回 `Io`。`save_if_current` 不得用 expected revision 填补 corrupt/unreadable 状态，并应有直接回归测试，断言 main 与 backup 字节均保持不变。

### 3. `cleanup_stale_temps` 是否可能删除并发 writer 的 temp？

**结论：确认（Confirmed），并违反并发/陈旧 revision 要求（High，见 F003）。**

`cleanup_stale_temps` 在每个成功 save 后列举 base directory（`repository.rs:154,165-177`），并对所有名称以 `data.json.tmp.` 开头的文件调用 remove（`174-176`）。没有文件锁、所有权记录、年龄阈值或活动 writer 协议。另一个 process 已经完成 temp 的 durable 写入、尚未 rename 时，当前 writer 可将其 temp 删除；后者的 rename 随后失败。更糟的是，两个 writer 都能在 revision check 后继续各自的普通 `save`，使 check 与 rename 之间发生 TOCTOU 覆盖。现有 temp 测试只放置两个静态“stale”文件（`repository_tests.rs:640-668`），没有 barrier 驱动的并发 interleave 测试。

## Findings（按严重级别排序）

### M01B-R01-F001 — High — Recovered 后保存破坏恢复副本和损坏证据

- **文件/行：** `src/storage/repository.rs:131-158,191-241`；缺失测试在 `src/storage/repository_tests.rs:257-285` 之后。
- **问题：** 已从有效 `.bak` 恢复的 document 进入普通 `save` 时，代码无恢复状态保护，反而在 rename 前执行 `copy(corrupt main, data.json.bak)`。
- **影响：** 有效 backup 被损坏字节覆盖；成功时 corrupt main 证据也消失；copy 与 rename 之间崩溃时，没有 main/backup 可解码而 temp 又不参与 recovery。这违反 M01 目标“不能破坏唯一有效配置”及 M01.4“不自动覆盖损坏证据”。
- **证据/复现：** 见上方 Mandatory item 1；现有测试只覆盖 load 本身，并未续接 save。
- **建议：** 默认拒绝在 corrupt-main recovery 状态的保存；提供明确、可审计的恢复 API，并先耐久保存损坏证据、保留有效 backup。补充普通 save、`save_if_current`、rename 前中断三类字节级回归测试。

### M01B-R01-F002 — High — `save_if_current` 把损坏 main 当作 expected revision

- **文件/行：** `src/storage/repository.rs:249-275`；相关现有测试仅为有效 main 的 `repository_tests.rs:501-589`。
- **问题：** 不可解码 main 在 `on_disk_revision` 中变成 `None`，随后 `save_if_current` 将它替换为 `expected_revision`，比较必然成功并执行写入。
- **影响：** API 名称承诺的 revision guard 在数据最需要保护时失效；可覆盖损坏主配置，并可按 F001 路径破坏有效 `.bak`。这不是“拒绝或序列化陈旧写入”。
- **证据/复现：** 见 Mandatory item 2；不存在 corrupt-main + `save_if_current` 的拒绝和字节保留测试。
- **建议：** 让 revision 查询区分 missing、corrupt 和 I/O；corrupt 返回 `RepositoryError::CorruptData`，unreadable 返回 `Io`，绝不可回退为 expected revision。只有经明确设计的新建文档路径才允许 missing 状态。

### M01B-R01-F003 — High — revision guard 非原子，且 cleanup 可删除活跃 writer 的 temp

- **文件/行：** `src/storage/repository.rs:131-158,165-177,249-275`；`src/storage/location.rs:31-36`；测试缺口 `src/storage/repository_tests.rs:538-575,640-668`。
- **问题：** `save_if_current` 的读 revision 与随后 `save` 的 backup/rename 之间没有 inter-process lock 或 compare-and-swap；公开的普通 `save` 完全无 revision 检查。每次成功后 cleanup 又无条件删除所有相同前缀 temp，无法辨别 crashed writer 与仍在写入的 writer。
- **影响：** 两个实例可都在同一旧 revision 上通过检查并先后 rename，后一实例静默覆盖前者；或一个实例删除另一个已 sync、尚未 rename 的 temp，使后者以非语义化 `Io` 失败。两者均不符合“拒绝或序列化，不能默默覆盖新状态”和“遗留 temp 清理策略安全”。
- **证据/复现：** 现有“external writer”测试将 external save 完整结束后才执行 B 的 check，未覆盖 check→rename 交错；stale-temp 测试只处理预先存在的静态文件。按上述源码顺序在两个 writer 之间插入 barrier 即可复现。
- **建议：** 为同一 base directory 使用跨进程排他写锁，且锁覆盖 revision 读取、corrupt 状态检查、backup、rename 和 cleanup；在锁内完成真正的 CAS 语义。cleanup 只能在该锁持有时运行，或先禁用自动删除并采用可证明非活跃的恢复策略。补充两进程/可控 FileOps barrier 测试：双 `save_if_current` 只允许一个成功，另一方得到 `ConcurrentModification`；以及 writer A cleanup 不能删除 writer B 的 temp。

### M01B-R01-F004 — High — 不可访问的文件被误判为 missing/corrupt

- **文件/行：** `src/storage/io.rs:26-28,64-65`；`src/storage/repository.rs:193-225,233-240,267-275`。
- **问题：** `FileOps::exists` 是无错误通道的 bool，真实实现使用 `Path::exists()`；main read 的任意错误在 load 中都被当作“未解码”，backup 的任意 read 错误也被吞为 `None`。若两者不可读，`load` 最终返回 `NotFound`（`218-225`），而不是 `Io`。`on_disk_revision` 同样把所有非 NotFound I/O 伪装为 `None`。
- **影响：** 临时共享冲突、访问被拒绝、离线/不可访问存储会被误认为无数据或损坏数据，违反“不可访问不等于无效或应删除”的项目约束；后续调用方可能走错误的恢复/新建/覆盖决策。
- **证据/复现：** `try_recover_backup` 仅将显式 `NotFound` 单列，其他错误仍以 `Ok(None)` 返回（`235-240`）；现有测试没有 AccessDenied、sharing violation 或 base directory 不可访问的 case。
- **建议：** 将存在性检查改为 `io::Result<bool>`（或直接以打开/metadata 的精确错误驱动）；只对明确 `NotFound` 采取 missing 分支，其他 I/O 返回匿名 `RepositoryError::Io`。main 读取错误不应自动从 backup “恢复”。加入注入式权限/读取失败测试，断言不会返回 `NotFound`/`Recovered`，且不会修改任何文件。

### M01B-R01-F005 — Medium — backup 覆盖不是故障安全操作，测试未模拟部分 copy 失败

- **文件/行：** `src/storage/io.rs:52-54`；`src/storage/repository.rs:140-147`；`src/storage/repository_tests.rs:410-443`。
- **问题：** `fs::copy(main, backup)` 直接以 live `data.json.bak` 为目标；真实 copy 在目标打开/截断之后仍可失败，届时旧 backup 已可能被部分内容破坏。现有 `FaultyFileOps::copy` 在调用真实 copy 前直接返回错误（`io.rs:187-192`），所以 `save_fault_backup_copy_error_preserves_main_and_backup` 不能证明实际部分 copy failure 会保留旧 backup。
- **影响：** 虽然当前 main 在该错误后通常仍存在，但之前的可恢复版本可能丢失，和模块文档“failed step never deletes ... backup”（`io.rs:3-7`）不一致；F001 的 recovery 场景会进一步放大影响。
- **证据/复现：** 测试预置 `{"older"}` 后仅触发“写入前失败”，并非截断/部分写入后的失败。
- **建议：** 先将 backup 内容写到独立 backup-temp，`write_all`/flush/sync 成功后再原子 replace `.bak`，并为其加入 write/sync/replace fault points；发生错误时保持旧 `.bak` 字节不变。该流程还应明确处理并测试 crash 边界。

## Cross-cutting 检查

- **正确性：** 单 writer 的正常 save/load、Unicode round-trip、previous-main backup 与 invalid-data no-write 均有合理测试。F001–F004 使损坏和多 writer 情形的正确性不成立。
- **错误处理：** `RepositoryError::Display` 是固定匿名文本（`repository.rs:47-85`），不会回显路径、JSON 或文件夹名，这是正确的。反之 F004 中对 I/O 的错误分类被吞掉，调用方无法正确处理不可访问状态。
- **数据安全：** temp 在 rename 前同步是正确基础；但恢复后保存、corrupt revision fallback、非原子并发和 live-backup copy 造成高风险数据安全缺口。post-rename `sync_path` 错误在 `repository.rs:149-152` 被明确忽略，且当前 fault matrix 未覆盖该 durability 边界；不得将当前 CI 成功表述为断电耐久性证明。
- **隐私：** 持久化边界不记录用户路径或 JSON；error 转换丢弃底层细节，符合避免路径泄露的要求。测试 fixture 包含敏感路径/标记，`repository_error_display_contains_no_stored_content`（`repository_tests.rs:675-683`）提供有限但相关的检查。
- **安全性：** 未引入网络、遥测或真实目录删除 API。temp 清理只会调用 `remove_file`，不会删除目录；但应按 F003 改为仅处理可证明归属且非活跃的 temp。
- **Windows 行为：** 同目录命名满足同卷 rename 前提，CI 已在 Windows/MSVC 运行基本路径。尚未验证 Windows sharing violation、杀毒/索引器占用、并发 process、断电/崩溃及首次 `%LOCALAPPDATA%\FileGo` 目录创建。`std::fs::rename` 遇目标被占用的失败已能返回 `Io`，但没有用户可恢复流程测试。
- **测试覆盖：** fault matrix 覆盖 temp write/sync、backup-copy-before-touch、rename-before-touch；正常 recovery 和删除不变量已有测试。缺少 F001/F002 的腐损后保存链、F003 的 barrier 并发、F004 的 I/O 分类、F005 的部分 backup copy、post-rename sync failure 和真实 crash/restart 测试。
- **性能：** 正常 document 规模下编码和同目录操作开销可接受；不过每次成功保存都 `read_dir` 并遍历整个数据目录（`repository.rs:165-177`），同时该扫描正是并发危险来源。修复应优先安全性，再评估目录规模。
- **可访问性：** 本变更无 UI、键盘或可访问性树改动；无新增 UI a11y 结论。
- **可维护性：** `FileOps` seam、路径对象与命名清晰，故障注入结构便于扩展；但 cleanup 绕开该 seam 使用直接 `std::fs::read_dir`，且 bool `exists` 丢失错误信息，使关键安全语义难以测试和维护。
- **依赖与许可证：** 新增 `tempfile = "3.20"` 是 dev-dependency（`Cargo.toml:35-36`），lock 固定实际解析为 `3.27.0`（`Cargo.lock:4221-4231`），不会进入 production runtime dependency graph。许可证策略允许 MIT 与 Apache-2.0（`deny.toml:11-25`），且本 head 的 CI license audit 成功；未发现与 MIT 发布约束冲突的证据。manifest 使用 semver 范围而非 `=`，但受已提交 lockfile 与 `--locked` CI 约束，不单列 finding。

## 未能自动验证的项

1. 修复后需在真实 Windows 上验证跨进程排他锁、崩溃后 lock 释放、病毒扫描/同步软件/文件占用下的错误分支，以及不会删除另一进程 temp。
2. 需要故障注入或受控关机实验验证 backup-temp、main rename、post-rename durability 的实际 crash 边界；普通单元测试不能证明断电后文件系统元数据持久性。
3. `%LOCALAPPDATA%\FileGo` 的 production 解析、首次目录创建、权限错误 UI 提示和恢复动作尚未接入本 slice，须在其实现里另行测试和复审。
4. 本变更无 GUI；托盘、IME、DPI、多显示器等桌面验收不在本次持久化代码范围。

## 最终结论

`CHANGES_REQUESTED`

存在 4 个 High finding，其中 F001、F002 直接违反损坏配置证据/唯一有效恢复副本保护，F003 直接违反并发/陈旧 revision 约束，F004 违反不可访问文件的安全处理原则；F005 仍需在 backup 写入路径关闭。修复后应：补齐针对每项 finding 的测试、在实际 `72c623b` 后的新修复 commit 上重新通过 MSVC CI、由独立 reviewer 进行 r02 复审。本文不构成里程碑或发布批准。
