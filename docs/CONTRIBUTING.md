# 贡献指南 — ScoreLeap（谱跃）

## 行为准则

- 尊重所有贡献者；讨论聚焦技术与产品；隐私与安全话题保持谨慎。

## 开发流程（Issue 驱动，禁止直接提交 main）

1. 在 Issue 中认领任务（或新建 Issue，遵循模板）；
2. 检查依赖 Issue 是否完成；
3. 在 Issue 中发布实施计划，状态改为 `status:in-progress`；
4. 创建分支与 Worktree：

```bash
git worktree add ../scoreleap-worktrees/issue-12 -b feat/12-music-ir main
```

5. 实现 + 测试 + 更新文档；
6. 本地自审（fmt/clippy/test 全过）；
7. 提交（Conventional Commits，正文 `Refs #12`）；
8. 推送并创建 PR（正文包含 `Closes #12` 与检查清单）；
9. 等待独立审查通过 + CI 全绿；
10. 合并 → 关闭 Issue → 删除分支与 Worktree → 更新 Epic 任务列表。

## 目录所有权

见 `planning/repository-plan.md` 第 2 节。公共文件（根配置、公共类型、docs/adr）修改必须经 Agent A 协调，禁止跨目录私自修改。

## 质量门槛

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

- 所有 Bug 修复必须附带回归测试；
- 调度测试必须使用 `MockInputBackend` + `VirtualClock`，禁止依赖真实按键注入。

## 提交信息规范

| 前缀 | 用途 |
|---|---|
| `feat:` / `fix:` | 功能 / 缺陷 |
| `docs:` / `test:` | 文档 / 测试 |
| `refactor:` / `chore:` | 重构 / 杂项 |
| `ci:` / `build:` | CI / 构建 |

## 合规红线（违反即拒绝合并）

- 任何反作弊绕过、隐藏自动化、内存操作相关代码；
- 未审计许可证的依赖；
- 硬编码本机路径、提交密钥、提交模型权重；
- 无 Issue 的正式功能提交；
- CI 失败仍合并。

## 分支命名

`feat/<issue-number>-<short-name>` / `fix/…` / `docs/…` / `test/…` / `refactor/…`
