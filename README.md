# ScoreLeap 谱跃

> 把 MIDI 曲谱编译成游戏乐器可演奏的时间轴，在自由演奏场景用键盘替你完成演奏。
> ScoreLeap compiles MIDI scores into a playable timeline for in-game instruments —
> an honest, visible, and controllable way to play music in free-play mode.

> **⚠️ 重要风险提示（必读）**
>
> 本工具通过模拟键盘输入辅助演奏，**第三方自动化工具可能违反游戏用户协议，存在账号封禁风险**。
> 请仅用于**自由演奏 / 个人空间**场景，本工具**不提供任何反检测能力**（不随机化、不隐藏行为、不伪装进程），
> **使用后果由您自行承担**。每次演奏前请再次确认您所在游戏的最新用户协议。
>
> **⚠️ IMPORTANT**: Automating input may violate game terms of service and **can lead to account bans**.
> Use ScoreLeap only in free-play / private spaces. This tool provides **no anti-detection features**.
> Use at your own risk.

---

## 架构

```
┌──────────────────────────────────────────────┐
│ 前端 apps/scoreleap（Vue 3 + TypeScript）      │
│  导入 → 轨道选择 → 编排参数 → 钢琴卷帘预览 → 演奏 │
└──────────────────────┬───────────────────────┘
                       │ Tauri IPC（invoke / events）
┌──────────────────────┴───────────────────────┐
│ Rust 核心（平台无关 crates）                    │
│  scoreleap-midi ──► scoreleap-music-ir        │
│  scoreleap-arranger（转调/音域/量化/和弦简化）   │
│  scoreleap-sequence（CompiledSequence）        │
│  scoreleap-scheduler（精确调度 + 紧急停止）      │
└──────────────────────┬───────────────────────┘
                       │ tauri-plugin-scoreleap-input
                       ▼
              Windows SendInput → 游戏自由演奏页面（用户主动授权）
```

数据流：`.mid` → 解析（`scoreleap-midi`）→ 编排（`scoreleap-arranger`）→
`CompiledSequence` → `scoreleap-scheduler` 按时间轴发送按键事件。
详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 功能（v0.1.0 — Windows MIDI MVP）

- **MIDI 导入与解析**：格式 0/1/2、running status、NoteOn velocity=0、tempo map（整数微秒时间轴）
- **轨道选择**：多轨 MIDI 可选任意轨道组合
- **编排管线**：自动转调（适配音域）、音域折叠、节奏量化、复音限制与和弦简化
- **钢琴卷帘预览**：演奏前可视化检查每个音符与按键
- **倒计时演奏**：3 秒倒计时后开始，全程状态可见
- **紧急停止**：任意时刻 `Ctrl+Alt+F9` 全局快捷键立即释放全部按键
- **乐器 Profile 系统**：36 键游戏乐器布局（`game-profiles/identity-v`），可校准
- **纯本地处理**：无网络、无遥测

## 安装

从 [GitHub Releases](https://github.com/superdaobo/scoreleap/releases) 下载
Windows NSIS 安装包（`ScoreLeap_0.1.0_x64-setup.exe`）。

- **SmartScreen 提示**：安装包未做代码签名时，Windows SmartScreen 会提示
  “Windows 已保护你的电脑 / 未知发布者”。这是未签名软件的常规提示，并非病毒。
  请先按下方方法校验 SHA256，确认无误后点击「更多信息 → 仍要运行」。
- **SHA256 校验**（与 Release 页公布的哈希比对）：

```powershell
Get-FileHash -Algorithm SHA256 -Path .\ScoreLeap_0.1.0_x64-setup.exe
```

- **系统要求**：Windows 10/11 x64（要求系统级键盘输入权限，运行于用户会话）。

## 快速开始

1. **导入 MIDI**：点击「导入」，选择本地 `.mid` 文件（文件仅在本机解析，不上传）
2. **选择轨道**：勾选要演奏的轨道（旋律 / 和弦等）
3. **设置编排参数**：移调、音域策略、量化、最大复音
4. **预览**：钢琴卷帘确认音符与按键映射是否符合预期
5. **风险确认**：每次演奏前确认风险提示（含游戏协议自查链接）
6. **倒计时演奏**：3 秒倒计时后自动开始演奏
7. **紧急停止**：随时按 `Ctrl+Alt+F9` 立即停止并释放所有按键

> 提示：演奏前请将游戏切至自由演奏模式并把乐器音域调整到与 Profile 一致。

## 重要风险提示

- **封号风险**：自动化输入在任何游戏中都可能被检测为异常行为；即使仅在自由演奏模式使用，
  仍可能违反游戏用户协议并导致账号处罚（确定性风险）。**使用后果由您自行承担**。
- **仅限自由演奏/个人空间**：本工具面向自由演奏与练习场景，不用于竞技、代打或任何牟利用途。
- **不提供反检测能力**：项目明确**不实现**输入随机化、行为隐藏、进程伪装等任何规避检测功能——
  诚实、可见、可控是本工具的立场。
- **协议自查**：使用前请查阅您所用游戏的最新用户协议（例如网易游戏用户协议：https://id5.163.com ）。
- 完整风险分析见 [docs/RISK_AND_COMPLIANCE.md](docs/RISK_AND_COMPLIANCE.md)。

## 隐私声明

- **本地处理**：MIDI 文件解析、编排、演奏全部在本地完成，文件**不上传**任何服务器；
- **无遥测**：不收集使用数据、不埋点、无账号体系；
- **日志**：仅本地调试日志（可关闭），不包含文件内容。
- Privacy: All processing is local — your files are never uploaded and no telemetry is collected.

## 开发指南

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 启动 Tauri 开发（前端 + Rust）
pnpm lint             # ESLint
pnpm typecheck        # TypeScript 类型检查
pnpm test             # 前端单元测试
cargo test --workspace        # Rust 全工作区测试
cargo clippy --workspace --all-targets --all-features -- -D warnings
node scripts/gen-fixtures.mjs # 重新生成 MIDI 测试固件（fixtures/midi/）
pnpm tauri build      # 构建发布版（NSIS 安装包）
```

仓库结构：`apps/scoreleap`（Tauri 应用）、`crates/*`（平台无关核心）、
`plugins/tauri-plugin-scoreleap-input`（Windows SendInput 输入后端）、
`game-profiles/`（乐器布局）、`fixtures/`（测试固件与期望输出）。
详细规范见 [docs/PRD.md](docs/PRD.md)、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、
[docs/ROADMAP.md](docs/ROADMAP.md)、[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)。

## 许可证与致谢

- 许可证：[GPL-3.0](LICENSE)（完整文本见仓库 LICENSE 文件）
- 致谢：
  - [Tauri](https://tauri.app/)（应用框架，MIT/Apache-2.0）
  - [Vue 3](https://vuejs.org/)（前端框架，MIT）
  - [midly](https://github.com/kovaxis/midly)（MIDI 解析，Unlicense）
  - [pnpm](https://pnpm.io/)（包管理，MIT）、[GitHub Actions](https://github.com/features/actions)

## 版本路线图

| 版本 | 主题 | 一句话 |
|---|---|---|
| **v0.1.0** | Windows MIDI MVP | 端到端可演奏：导入 MIDI 到游戏内自动演奏（当前） |
| v0.2.0 | Android MIDI MVP | 触屏演奏：复用核心，Android 手势分发 |
| v0.3.0 | Score Editor | 可编辑工程：钢琴卷帘编辑、工程文件、撤销重做 |
| v0.4.0 | Audio Transcription | 音频转 MIDI：Basic Pitch ONNX（Windows） |
| v0.5.0 | Android AI | 移动端 AI 转录与低性能设备降级 |

---

*ScoreLeap（谱跃）—— 让乐谱在游戏世界里跃动。*
