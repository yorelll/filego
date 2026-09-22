# Response: M01-B 安全持久化（Round 01）

## Metadata

- **版本:** `0.0.1`
- **里程碑/topic:** `m01b-persistence`
- **轮次:** `r01`
- **Implementation agent:** M01-B 修复 implementation agent（仅实现与补测并触发 CI；未参与 `m01b-persistence-review-r01` 评审，代码修复期未做评审）
- **对应 review 文档:** [`m01b-persistence-review-r01.md`](m01b-persistence-review-r01.md)
- **评审前 commit SHA（head）:** `72c623b7207905ea9e7101bf783f89132d407cb4`
- **评审/比较范围:** `dce6121..72c623b`
- **修复后 commit SHA:** `03179339a5e01f48dc73e54ffe97dcfff8ca8f7c`（消息 `fix: harden persistence recovery, revision and backup fault-safety`）
- **Response 日期:** 2026-09-21

## Summary

| Finding | Assessment | Status |
|---|---|---|
| F001 (High) | `ACCEPTED` | 已实现 pending-recovery 状态 + 显式 `repair_from_backup` + 证据文件；普通 `save`/`save_if_current` 在待修复期拒绝并保护字节 |
| F002 (High) | `ACCEPTED` | `on_disk_revision` 显式区分 Missing / Corrupt / Ok；corrupt 返回 `CorruptData`，绝不伪造 expected revision |
| F003 (High) | `ACCEPTED` | 新增 `data.json.lock` 写锁（create-new 语义），覆盖 check→backup→rename→cleanup；cleanup 仅在持锁时运行且只删 `data.json.tmp.*` |
| F004 (High) | `ACCEPTED` | `FileOps::exists` 改为 `io::Result<bool>`；load / backup 恢复 / revision 查询只对显式 `NotFound` 走 missing 分支，其余 I/O 返回 `Io` |
| F005 (Medium) | `ACCEPTED` | backup 改为 backup-temp 写入 + flush/sync + 原子 rename，写入/同步/替换均提供 fault point |

无 `REJECTED` / `PARTIALLY_ACCEPTED` 条目。全部 5 个 finding 接受并已实现；所有既有不变量与测试保持通过，并按 review 要求新增/改写回归测试。

## 变更范围

- `src/storage/io.rs` — `FileOps` 契约调整（详见 F004/F005）；新增 `WriteLock`（详见 F003）；`FaultyFileOps` 故障矩阵扩展 `LockAcquire` / `BackupWrite` / `BackupSync` / `BackupReplace`，原 `BackupCopy` 取消。
- `src/storage/location.rs` — 新增 `BACKUP_TEMP_FILE_PREFIX`、`CORRUPT_EVIDENCE_PREFIX`、`LOCK_FILE_NAME` 常量及 `backup_temp_document_path`、`corrupt_evidence_path`、`lock_file_path` 派生函数。
- `src/storage/repository.rs` — 实现 F001–F005（详见各 finding 小节）；模块文档新增恢复契约与并发契约。
- `src/storage/repository_tests.rs` — 新增回归测试（见各小节）；更新故障矩阵测试的 step 索引。

未改动 `codec.rs`/`schema.rs`/`domain`/`Cargo.toml`/`Cargo.lock`/UI/workflow。

## Finding Responses

### `F001` — `ACCEPTED`（High）

- **评估:** 属实。`load` 返回 `Recovered` 后，普通 `save` 会先把损坏 main 复制为 `.bak`，再覆盖 main，使唯一有效恢复副本失效并销毁损坏证据。
- **修改（repository.rs）:**
  - 新增状态字段 `pending_recovery: bool`。`load` 在「main 缺失/损坏 + backup 可解码」时置真（`Recovered`），返回文档同时标记待修复。
  - `save` 与 `save_if_current` 入口先调用 `reject_if_pending_recovery()`：待修复期返回新增错误 `RepositoryError::RecoveryRequired`，不触碰 main 与 backup 任何字节。
  - 新增显式恢复 API `repair_from_backup(&mut self) -> Result<RepairOutcome, RepositoryError>`：
    1. 重读 main：若现在能解码（外部已修复），清除 pending 并刷新 `latest_loaded_revision`，返回 `HadNoCorruptMain`；
    2. 否则 main 损坏或缺失：必须能解码 backup，否则返回 `CorruptData` 且 main/backup 均原样保留；
    3. main 存在且损坏时，先把损坏字节写入新的独立证据文件 `data.json.corrupt-<uuid>`（经 `write_flush_sync`，durable）；
    4. 将 backup 字节经「新 temp 写入 → 原子 rename → 尽力 sync」提升到 main；
    5. 清除 pending，内存文档与 `latest_loaded_revision` 更新为恢复文档，此后普通 `save` 恢复可用。
  - 返回类型 `RepairOutcome::{Repaired{evidence: Option<PathBuf>}, HadNoCorruptMain}`，evidence 暴露证据文件路径供调用方审计。
  - 模块文档新增「恢复契约」说明。
- **新增测试（repository_tests.rs）:**
  - `recovered_pending_save_is_rejected_and_preserves_main_and_backup` — load 后普通 save → `RecoveryRequired`，main 与 backup 字节均不变。
  - `recovered_pending_save_if_current_is_rejected_and_preserves_main_and_backup` — 同上对 `save_if_current`。
  - `repair_from_backup_preserves_evidence_and_repairs_main` — 修复后 main 可解码为恢复文档；损坏字节原样存在于证据文件；backup 仍可解码且字节不变；pending 清除；`latest_loaded_revision` 更新；随后 save 成功且证据文件不被清理。
  - `repair_from_backup_with_missing_main_recreates_main` — 仅 backup 时 main 从 backup 重建，无证据文件，pending 清除后 save 可用。
  - `repair_from_backup_rejects_absent_or_invalid_backup_and_preserves_both` — pending 状态下 backup 被外部改成无效/删除后修复返回 `CorruptData`，main/backup 原样保留，无证据文件写出。

### `F002` — `ACCEPTED`（High）

- **评估:** 属实。`on_disk_revision` 对解码失败返回 `Ok(None)`，`save_if_current` 用 `expected_revision` 填 None，任意 expected 都通过并执行覆盖。
- **修改（repository.rs）:** `on_disk_revision` 改为返回显式分类枚举 `OnDiskRevision::{Missing, Corrupt, Ok(u64)}`：
  - 只读显式 `NotFound` → `Missing`（新建路径）；
  - main 存在但解码失败 → `Corrupt`；
  - 其他非 `NotFound` 读错误 → `RepositoryError::Io`。
  - `save_if_current` 对 `Missing` 视为无基线（允许显式 bootstrap 新建），对 `Corrupt` 直接返回 `CorruptData`（绝不回退到 `expected_revision`），对可读但不等于 expected 返回 `ConcurrentModification`。
- **新增测试:**
  - `save_if_current_with_corrupt_main_returns_corrupt_data_and_preserves_both` — 未 load（无 pending）情况下 corrupt main + 有效 backup：`save_if_current(doc, 任意 expected)` → `CorruptData`，main 与 backup 字节均保留。
  - 既有 `revision_guard_is_not_enforced_before_any_load_or_save`（fresh-dir 新建）与 `revision_guard_rejects_stale_expected_and_accepts_matching`（可读 main 的 stale/matching）保持通过，语义等价。

### `F003` — `ACCEPTED`（High）

- **评估:** 属实。`save_if_current` 的 check 与 rename 之间无互斥；`cleanup_stale_temps` 无条件删除同前缀 temp，可删除并发 writer 的 temp。
- **修改（repository.rs + io.rs + location.rs）:**
  - 新增 per-directory 写锁：`data.json.lock`，以 `FileOps::create_new`（`File::create_new`）获得独占语义，`WriteLock` RAII 持有；`AlreadyExists` → `RepositoryError::ConcurrentModification`（文档化语义：另一 writer 持有或遗留 stale lock 都拒绝写入，绝不静默破坏锁）。drop 时尽力删除锁文件。
  - `save` 与 `save_if_current` 在编码成功后 `acquire_write_lock()`，锁覆盖整个 revision check → backup → rename → cleanup。两个进程的 `save_if_current` 不可能都通过 check-then-rename：后到者拿到 `ConcurrentModification`。
  - `cleanup_stale_temps` 仅在持锁期间运行：锁成立时任何 `data.json.tmp.*` 都属陈旧；只枚举并删除 `data.json.tmp.*` 前缀，绝不触碰 `data.json.corrupt-*`、`data.json.bak.tmp.*` 与 `.lock`。
  - 自动过期不做（会静默破坏互斥）；stale-lock 处理明确记录为 0.0.1 的手工/下一 slice 事项。
  - 模块文档新增「并发契约」。
- **新增测试:**
  - `two_writers_contending_save_if_current_only_one_wins` — 两个真实线程上的 repository 经屏障（Barrier）同时 `save_if_current`，恰一个成功、一个 `ConcurrentModification`（repository 为 `Send`，保障双实例跨线程互斥等价于跨进程锁语义）。
  - `cleanup_does_not_delete_interleaved_writers_temp` — A 的 cleanup 只删持锁时已存在的 stale temp；B 在 A 释放锁之后产生的 temp 不被 A 清理，B 随后 save 成功。
  - `cleanup_never_removes_evidence_or_backup_temps_or_lock` — save 后证据文件、backup temp 幸存，锁文件被释放。

### `F004` — `ACCEPTED`（High）

- **评估:** 属实。`FileOps::exists` 无错误通道；main/backup 的任意读错误被当作「未解码/无数据」，不可访问被误判为 missing/corrupt。
- **修改（io.rs + repository.rs）:**
  - `FileOps::exists` 签名改为 `fn exists(&mut self, path: &Path) -> io::Result<bool>`；`FsFileOps` 用 `fs::metadata(...).map(|_| true).or_else(仅 NotFound→false / 其他→Err)`。
  - `load` 的 main read 只对 `NotFound` 视为缺失；其他错误返回 `RepositoryError::Io`，不进入 backup 恢复，也不判定为 missing。
  - `try_recover_backup` 只对 `NotFound` 返回 `Ok(None)`；其他读错误返回 `Io`。
  - `on_disk_revision`（见 F002）只对 `NotFound` 返回 `Missing`，其余读错误返回 `Io`。
- **新增测试（注入 `ReadFaultFileOps` / `ExistsFaultFileOps` seam，按文件名注入非 `NotFound` 错误）:**
  - `load_main_read_access_denied_returns_io_and_touches_nothing` — main 读返回 `PermissionDenied` → `Io`，不落 backup；文件不被修改。
  - `load_backup_read_access_denied_returns_io` — backup 读返回 `PermissionDenied` → `Io`（不是「无 backup」→ `CorruptData`/`NotFound`）；main 字节保留。
  - `save_if_current_main_read_access_denied_returns_io` — revision 查询读返回 `PermissionDenied` → `Io`，不当作 missing baseline。
  - `save_main_exists_error_returns_io_and_preserves_previous_main` — `exists()` 返回错误 → `Io`，main 保留。

### `F005` — `ACCEPTED`（Medium）

- **评估:** 属实。`fs::copy(main, backup)` 直接目标 live `.bak`，copy 在截断后才失败时旧 backup 已被破坏；原 `FaultyFileOps::copy` 在触碰目标前失败，无法证明部分 copy failure 的安全。
- **修改（repository.rs + io.rs）:** 删除 `FileOps::copy`；备份改为故障安全三明治：
  1. 读当前 main 字节；
  2. 写入独立 backup-temp `data.json.bak.tmp.<token>`（`write_all` + flush + `sync_all`）；
  3. 原子 `rename` 到 `data.json.bak`。
  任一步失败时旧 `.bak` 字节保持原样。`FaultyFileOps` 增加 `BackupWrite` / `BackupSync` / `BackupReplace` 故障点（按 `data.json.bak.tmp.*` 文件名路由），删除已无意义的 `BackupCopy`。
- **新增测试:**
  - `save_fault_backup_staging_preserves_previous_backup` — 对 `BackupWrite`(step4)、`BackupSync`(step5)、`BackupReplace`(step6) 三个故障点各跑一遍：save 返回 `Io`，预先存在的 `.bak` 字节 byte-identical，main 不动。
  - 既有 `save_fault_rename_error_preserves_main_and_backup` 更新为新的 main-rename step 索引（step7）；新增 `save_fault_backup_staging_preserves_previous_backup` 一并覆盖 backup-rename 故障（step6）。

## 本地 GNU 验证证据（快速反馈，非 MSVC 权威）

- **Toolchain:** `1.92.0-x86_64-pc-windows-gnu`（Rust 1.92.0, cargo 1.92.0）；预检确认已安装 `x86_64-pc-windows-gnu`、`rustfmt`、`clippy`；未通过 rustup 下载任何组件。
- **记录前禁止自下载:** `RUSTUP_AUTO_INSTALL=0`、`RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-gnu`。
- **结果:**
  - `cargo fmt --all` 且 `cargo fmt --all -- --check` — PASS；
  - `cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings` — PASS；
  - `cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu` — PASS，lib 69 + main 1 = 70 项通过，0 失败；
  - `cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` — PASS。
- **已知 GNU linker 漂移:** `cannot find -lshlwapi` 出现（GNU 工具链自身问题，非本次代码引入）；按已有每会话（不提交）工作区设置 `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker=D:\mingw64\bin\x86_64-w64-mingw32-gcc.exe"` 与 `PATH` 中的 `D:\mingw64\bin` 后通过。本机 GNU 通过不替代远程 MSVC CI。
- 本地构建产物仅用于开发检查，不作为 release artifact 或发布哈希来源（CLAUDE.md §3.1）。

## CI 证据（远程 MSVC）

| 修复 commit SHA | Workflow | run ID/URL | 结果 | 说明 |
|---|---|---|---|---|
| `03179339a5e01f48dc73e54ffe97dcfff8ca8f7c`（+ response doc `4c8a232`） | [Windows CI](https://github.com/yorelll/filego/actions/runs/35726133222) | `35726133222` | in_progress（监控中） | run head `4c8a23235b0d05a591008d9b9f801691487dbe1c`，包含修复 commit；本文档记录提交时刻状态，待 run 结束后由 implementation agent / reviewer 复核结论 |

> 说明：run 从 `D:\Program Files\GitHub CLI\gh.exe run list` 获取。文档记录的是提交时刻的 in_progress 状态；未确认 success 前不宣称完成、不作为发布依据（CLAUDE.md §3.4）。

## 未解决事项与待人工验证项

1. **跨进程写锁为 0.0.1 尽力而为实现:** 锁文件为 create-new 语义；真实双进程并发下互斥成立，但「崩溃后 stale lock 恢复」不在本 slice（自动过期会静默破坏互斥，故刻意不做）。遗留 stale lock 时后续写入得到 `ConcurrentModification`，由用户手工删除锁文件恢复——这是下一 slice/文档化的运维步骤。
2. **真实 Windows 验证项（托管 CI 无法可靠覆盖）:** 跨进程排他锁在真实 Windows 上的行为、崩溃后锁释放、杀毒/同步软件/共享冲突下的错误分支、以及不会删除另一进程 temp。
3. **断电/崩溃边界:** backup-temp、main rename、post-rename durability 的真实 crash 窗口需故障注入/受控关机实验；普通单元测试不能证明断电后文件系统元数据持久性。Review 已指出 `save_locked` 中 post-rename `sync_path` 错误故意忽略（非致命、避免伪失败），本 slice 保持该语义并在响应中记录。
4. **`%LOCALAPPDATA%\FileGo` production 解析/首次目录创建/权限错误 UI 提示/恢复动作** 属 M03/M05 范围，未在本 slice 接入（location.rs 仍为注入 base_dir）。
5. **无 GUI/托盘/IME/DPI 等多显示器桌面验收项** 超出本次持久化代码范围。

## r02 复审请求

已完整修复 `m01b-persistence-review-r01` 的 F001–F005（全部 ACCEPTED），新增/更新 16 项持久化回归测试并更新模块文档契约。请独立 reviewer 针对本修复 commit 进行 r02 复审：复核代码 diff、新增测试、GNU 验证记录与远程 MSVC CI run（推送后回填），确认 F001–F005 关闭后再评估里程碑/发布结论。

请求原 reviewer（或另一名独立、未参与本实现与评审的 code-review agent）执行 r02 复审。
