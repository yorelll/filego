# Code Review Records

此目录保存纳入 Git 的里程碑与发布评审证据。完整规则见仓库根目录的 [`CLAUDE.md`](../CLAUDE.md)。

- 每个版本使用去掉点号的目录，例如 `0.0.1` 对应 `0-0-1/`。
- 实现者与评审者必须是不同的 agent。
- 文档按主题和轮次追加，禁止覆盖历史记录：`<topic>-review-rNN.md`、`<topic>-response-rNN.md`。
- 每次修复后必须重新运行 GitHub Actions，并由独立 reviewer 复审。
- 只有针对实际候选 commit 的最终评审明确给出 `APPROVED_FOR_RELEASE`，且手工验收通过，才允许发布。

当前版本入口：[`0-0-1/`](0-0-1/README.md)。
