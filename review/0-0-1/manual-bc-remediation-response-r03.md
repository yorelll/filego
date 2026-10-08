# FileGo 0.0.1 — B/C 手工验收整改 implementation response r03

- **版本 / topic / 轮次 / 日期**：`0.0.1` / `manual-bc-remediation` / `r03` / 2026-10-08。
- **角色**：implementation agent；本 agent 仅实现、测试并回复，不评审或批准自己修改的代码，也不填写用户验收结论。
- **对应 review**：`review/0-0-1/manual-bc-remediation-review-r02.md`，结论 `CHANGES_REQUESTED`。
- **修复前 / 修复后代码 SHA**：`9921c7bb3442e8654b624f67e266df8dfb9389b1` → `2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`（比较 `9921c7b..2b2a08c`）。该修复包含 `src/main.rs`、`src/platform/windows/tray_open.rs`、`src/presentation/{i18n,startup_registration}.rs` 和 `src/storage/{location,repository,repository_tests}.rs`，未改 Cargo / CI workflow。
- **用户验收原始记录**：`review/0-0-1/manual-acceptance.md` 的用户修改保留在本地未暂存、未提交；旧候选 B7/C5 FAIL、C1/C2/C3 部分通过以及 D+ 未测的状态**完全不变**。本轮开发包不能冒充新 RC，也没有 tag / GitHub Release。

## 对 r02 findings 的逐项回应

### BC-R02-H01 — High — `ACCEPTED`

**理由**：此前 UI 按加载时的内存健康状态选择 `set_data → save_at`。若加载后磁盘主文件损坏，普通保存会先把坏主文件写进有效的内部 `.bak` 并覆盖原件；这是确定性证据丢失路径。

**修改**：

1. `src/main.rs` 的设置页恢复入口现在只向仓库传入列表选中的平面 `backup-*.json` 文件名；不再在 UI 中按 `document().is_some()` 选择普通保存，也不在加锁之前解码实际要提升的快照。恢复成功后同步设置、管理器和搜索；内部备份恢复的 `HadNoCorruptMain` 分支也刷新 UI 并报告已应用，而不是误报失败。
2. `src/storage/repository.rs` 在同一个写锁下按平面文件名 allowlist 重新读取并验证选中备份、重读磁盘主文件，对健康主文件比较实际 revision **及先前读/写的精确原始 bytes**；拒绝未观察到的健康修改（包括同 revision）、以及加载后健康主文件消失的冲突。腐坏主文件原始 bytes 先同步写入唯一的 `data.json.corrupt-*`；健康主文件显式替换前也先同步保存在 `data.json.pre-restore-*`。提升使用同目录同步临时文件及原子重命名，不覆盖原有效内部 `.bak` 或用户选中快照；读/写故障返回错误，不报成功。修复后刷新仓库内存及观察状态。
3. `src/storage/repository_tests.rs` 针对**正常载入之后**主文件被外部损坏、同时存在有效内部 `.bak` 测实际命名恢复及各来源逐字不变；对步骤 3/4/5（提升临时写 / 同步 / 重命名）分别注入故障，确认已同步的坏主文件证据、原坏主、有效内部 `.bak`、选中命名备份仍完整。另覆盖未载入健康主文件冲突、外部更新 revision / 相同 revision 改 bytes、文件名越界 / 选定快照丢失或失效、健康主文件显式替换前的持久原版副本、加载后主文件消失的冲突。

**边界**：锁只约束 FileGo 仓库协作写者；外部不遵守 `data.json.lock` 的程序若恰在重读与重命名之间无锁改文件，无法由当前锁协议提供系统级独占。需用户真实桌面复测 B7/K7/K8：从已载入健康主文件在运行中变坏、保留内部和命名备份、恢复后检查坏原件 evidence 与文件实际字节；也测权限/锁冲突和重启持久性。普通 `save_at()` 在磁盘被非协作程序改坏后仍可能先复制坏主文件进 `.bak`；本修复针对 r02 明确指出的**命名备份恢复入口**，不宣称覆盖所有并发外部破坏场景，请 reviewer 特别复核该范围。

### BC-R02-M01 — Medium — `ACCEPTED`

**理由**：之前 `bool` 快照把缺失 Run 值和另一个 portable EXE 的 Run 值混为 `false`。启用失败后的 `false` 回滚会删除另一个 EXE 的值。

**修改**：

1. `src/platform/windows/tray_open.rs` 从当前用户的 Run 键读取原类型及至多 8192 原始 bytes（长度两次查询、错误/超限拒绝，不记录或显示路径），明确区分值缺失和存在；本进程 expected value 用 REG_SZ + 带终止符的原始 UTF-16 编码。恢复函数精确写回原类型和 bytes / 原不存在则删除，并由事务重读校验；EXE 路径截断拒绝当作完整路径。
2. `src/presentation/startup_registration.rs` 的可注入事务 seam 改为 `Option<Snapshot>` 和 `expected_value`。启用与禁用均**拒绝**已存在且不等于本进程注册 bytes 的值，不读写 JSON、也不修改 HKCU；提示中/英文说明可能需要在另一 FileGo 副本先关闭登录启动。非外来值先保存 JSON 后写 HKCU、重读严格比较；失败时尝试精确恢复原 snapshot 及 JSON，重读核验，无法完成返回 `RollbackFailed` 而非成功。`src/main.rs` 的生产 adapter 接上新 seam。模拟测试覆盖外来类型/bytes 启用和禁用无任何写入、部分写、读回失败、原有自家值恢复、回滚失败、缺数据与 pending repair。这些自动测试**从未修改实际 HKCU**。

**策略取舍**：固定键名已注册另一个 portable EXE 时不自动迁移、不替换、不删除；本轮未设计跨进程原子 compare-and-swap（其他不协作注册表写者可以在读与写之间改变值），需要用户真实桌面验证 J1–J6/C3.6（包含旧 portable EXE、中文/空格路径、登录后效果）。同类型同 bytes 被判断为自家值；原类型或非规范尾随 bytes 有差异时安全地视为 foreign 而拒绝。

## 自动验证 / CI 与未解决事项

- 本地 GNU 快速反馈：`RUSTUP_AUTO_INSTALL=0`，`RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-gnu`；预检 `rustc 1.92.0`、`cargo 1.92.0`、`x86_64-pc-windows-gnu` target、rustfmt、clippy 均已安装。工作树当时有本 agent 的未提交代码及用户未暂存验收记录；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --all-features --locked --target x86_64-pc-windows-gnu -- -D warnings`、`cargo test --workspace --all-features --locked --target x86_64-pc-windows-gnu`（421 lib，其中 420 通过、1 项设计为 ignored；10 bin 全通过；doc tests 0）、`cargo build --workspace --all-features --release --locked --target x86_64-pc-windows-gnu` 及 `git diff --check` 全部通过。GNU Release 构建只作本地开发验证，未用于发布物或哈希。提交后又单独通过了新加的 loaded-main promotion 故障测试、fmt 和 Clippy；推送时仅 code files 在 commit 中。
- 当前代码 commit `2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6` 已推送；[Windows CI run 37774121293](https://github.com/yorelll/filego/actions/runs/37774121293) 与 [Search benchmark run 37774121336](https://github.com/yorelll/filego/actions/runs/37774121336) 的 `headSha` **均精确等于该 SHA**，`completed/success`。前者 MSVC `x86_64-pc-windows-msvc` job `fmt, clippy, test, release, package` 全成功，artifact `FileGo-0.0.1-windows-x86_64-2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6` 为**开发包**；后者 job `release 10k benchmark` 成功，artifact `search-benchmark-log-2b2a08ccfd1fc5b824384a0752f12b7e78a1e9b6`。Hosted runner pinyin-heavy median 64.91ms、filtered-with-clone 57.86ms、multi-token 50.34ms，部分超过 50ms 产品目标但均通过 500ms CI 宽松回归门槛，不能推断用户机器性能已达标。
- **未自动验证**：真实前台焦点、初帧白屏/部分绘制、DPI/键盘/IME、托盘单/双击、Explorer 恢复、HKCU 真实登录行为、真实 ACL、用户新候选 EXE/ZIP/SHA 校验。旧用户 FAIL/PARTIAL 必须使用未来单独冻结并核哈希的新 MSVC RC 实测；C2 缺详细双击症状，不能声称已修复。M08 保持 `IN_PROGRESS`，release review 未批准。

**复审请求**：请独立、未参与本轮实现的 code-review agent 重新审查 `9921c7b..2b2a08c` 的实际 diff、上述故障测试及精确 SHA 的两项 MSVC run，并逐项判断 BC-R02-H01 / BC-R02-M01 是否关闭；若不成立继续要求实现修复，不要以本 response 单独代替代码检查，更不要批准发布或改写用户验收。
