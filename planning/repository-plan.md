# Repository Plan — ScoreLeap（谱跃）

> 本文档固化多 Agent 协作模型、目录所有权、分支策略与公共文件修改规则。
> 所有正式开发必须通过 Issue 驱动，遵循本文件约束。

## 1. 项目元信息

| 项 | 值 |
|---|---|
| 中文名 | 谱跃 |
| 英文名 | ScoreLeap |
| GitHub | `superdaobo/scoreleap`（公开） |
| License | GPL-3.0 |
| Tauri identifier | `com.superdaobo.scoreleap` |
| Android namespace | `com.superdaobo.scoreleap` |
| 默认分支 | `main` |

## 2. 多 Agent 角色与职责

| Agent | 角色 | 核心职责 | 目录所有权 |
|---|---|---|---|
| A | 产品与总体架构 | PRD、架构、ADR、公共接口、Issue 依赖、最终集成 | 根配置、`docs/`、`docs/adr/`、公共类型 |
| B | 音乐核心 | Music IR、MIDI 解析、Tempo Map、转调/音域/和弦、时间轴编译 | `crates/scoreleap-music-ir/`、`scoreleap-midi/`、`scoreleap-arranger/`、`scoreleap-sequence/`、`fixtures/midi/`、`fixtures/sequences/` |
| C | Windows 平台 | SendInput、扫描码映射、窗口检测、精确调度、全局快捷键、紧急停止 | `crates/scoreleap-scheduler/`、`plugins/tauri-plugin-scoreleap-input/src/desktop.rs`、Windows 专属代码 |
| D | Android 平台 | Tauri Android 插件、Kotlin、AccessibilityService、GestureDescription、Foreground Service、校准 | `plugins/tauri-plugin-scoreleap-input/android/`、`src/mobile.rs`、Android Manifest/Service/Instrumented Tests |
| E | 前端与质量 | Vue 3、Pinia、页面、钢琴卷帘、Vitest、Playwright、CI、文档 | `apps/scoreleap/src/`、前端测试、`README.md`、用户文档 |
| F | 独立审查 | 审查 PR、架构边界、安全、许可证、测试覆盖、验收标准 | 只读为主；必要修复必须单独开 Review Fix Issue |

## 3. 协作约束

1. 每个 Agent 必须有清晰的目录所有权（见上表）。
2. 不允许多个 Agent 同时修改相同公共文件。
3. 公共类型、Cargo Workspace、pnpm workspace、根配置由 Agent A 管理。
4. 跨模块接口必须先写 ADR 或接口说明，再开始并行实现。
5. 每个 Agent 使用独立分支和 Git Worktree。
6. 不允许直接向 main 提交。
7. 同一个 PR 的实现者和审查者不得是同一个 Agent。
8. 合并前必须由主 Agent（Agent A）统一运行完整测试。
9. 出现接口冲突时，暂停相关实现，由 Agent A 统一裁决。
10. 不得通过覆盖、强制推送或删除他人分支来解决冲突。

## 4. 分支与 Worktree 策略

- 分支命名：`feat/<issue-number>-<short-name>`、`fix/…`、`docs/…`、`test/…`、`refactor/…`
- Worktree 示例：

```bash
git worktree add ../scoreleap-worktrees/issue-12 -b feat/12-music-ir main
```

- 每个 Worktree 只能处理一个主要 Issue。
- 一个 PR 只对应一个 Issue；禁止超大 PR 实现整个版本。

## 5. 公共文件修改规则

1. 修改公共配置前在 Issue 中声明。
2. 由 Agent A 分配修改权。
3. 同一时间只有一个 Agent 修改公共文件。
4. 公共接口变更必须附带迁移说明。
5. 公共接口变更后通知所有受影响 Agent。
6. 不允许子 Agent 私自修改其他 Agent 的目录。

## 6. 提交与 PR 规范

- 提交信息使用 Conventional Commits：`feat:` / `fix:` / `docs:` / `test:` / `refactor:` / `chore:` / `ci:` / `build:`。
- 提交正文引用 Issue：`Refs #12`。
- PR 标题示例：`feat(core): implement Music IR`。
- PR 正文必须包含：`Closes #<n>`、变更内容、设计说明、测试结果、截图/录屏、风险、回滚方式、检查清单。

## 7. 禁止事项

- 一个 PR 实现多个无关 Issue；
- 直接向 main 推送；
- 未测试就提交；CI 失败仍然合并；
- 强制推送覆盖他人工作；
- 将临时文件和模型权重提交进 Git；
- 在代码里硬编码本机路径；
- 在仓库中保存密钥；
- 无 Issue 的正式功能提交。

## 8. 项目核心原则（所有 Agent 必须遵守）

1. 项目名称、核心 crate、公共接口中不得硬编码 Identity V。
2. 《第五人格》只是第一个 Game Profile（`game-profiles/identity-v/`）。
3. MIDI 演奏 MVP 不依赖 AI。
4. 音频转 MIDI 和 AI 模型在后续版本实现（v0.4 / v0.5）。
5. 所有转换尽量在用户本地执行。
6. 不上传用户的 MIDI、音频或曲谱，除非用户明确选择在线服务。
7. 不实现进程注入、内存修改、驱动注入、反检测、反作弊绕过或隐藏自动化行为。
8. 不用于排位、竞技操作或获得竞技优势。
9. 必须提供醒目的第三方自动化风险提示。
10. 必须提供紧急停止、释放所有按键和手势取消机制。
