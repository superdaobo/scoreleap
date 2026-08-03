# 架构文档 — ScoreLeap（谱跃）

| 项 | 值 |
|---|---|
| 版本 | v0.1-draft-1 |
| 关联 | PRODUCT_PLAN.md、PRD.md、docs/adr/* |

---

## 1. 总体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│  apps/scoreleap (Tauri 2 + Vue 3 + TypeScript)                  │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌─────────────────┐ │
│  │ 曲谱库/详情 │ │ 转换参数页  │ │ 钢琴卷帘预览 │ │ 演奏控制/测试页  │ │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └────────┬────────┘ │
│        └─────────────┴── Pinia 状态 ┴── services (invoke) ──┘   │
└──────────────────────────────┬──────────────────────────────────┘
                               │ Tauri IPC (invoke / events)
┌──────────────────────────────┴──────────────────────────────────┐
│  src-tauri (Rust)                                                │
│  ┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐ │
│  │ app commands │──▶│ scoreleap-core    │──▶│ 业务 orchestration│ │
│  └──────────────┘   └────────┬─────────┘   └──────────────────┘ │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │  crates（平台无关）                                          │  │
│  │  music-ir ← midi ← arranger → sequence（CompiledSequence）│  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │  tauri-plugin-scoreleap-input                              │  │
│  │  desktop.rs: Scheduler + SendInput / MockInputBackend      │  │
│  │  mobile.rs: 桥接 Kotlin（GestureDispatcher）                │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────┘
                Windows: SendInput   Android: Kotlin 插件
                               │
                       游戏自由演奏页面（用户主动聚焦/授权）
```

## 2. 模块依赖图（Rust）

【箭头约定：`─►` 表示「指向被依赖方」，即箭头尾部的 crate 依赖箭头头部的 crate】

```
scoreleap-music-ir       （零依赖：MusicDocument/Track/NoteEvent/TempoEvent/…，serde）
scoreleap-game-profile   （零依赖：GameProfile/InstrumentLayout schema + 校验）
     │                              │
     └──► scoreleap-midi            └──► scoreleap-sequence
             （midly → Music IR）          （CompiledSequence/PlatformAction/
                                          PlaybackState；依赖 music-ir）
               │                              │
               └──────────► scoreleap-arranger ◄──┘
                     （依赖 music-ir + game-profile + sequence；
                      转调/音域/量化/和弦 → CompiledSequence）
                                    │
                    scoreleap-scheduler（依赖 sequence；定义 InputBackend/Clock；
                                       不依赖 Tauri）
                                    │
                    tauri-plugin-scoreleap-input（实现 InputBackend：
                                                 desktop.rs / mobile.rs / models.rs）
                    scoreleap-core（Tauri 命令编排；依赖全部核心 crate）
```

依赖方向约束：`music-ir` 与 `game-profile` 零依赖；`midi` 只依赖 `music-ir`；`arranger` 依赖 `music-ir`/`game-profile`/`sequence`（不依赖 `midi`，便于独立测试）；`sequence` 依赖 `music-ir`；禁止循环依赖；`scheduler` 不依赖 Tauri（纯核心），由插件层适配。

## 3. 数据流

```
.mid 文件
  → scoreleap-midi::parse(bytes) → MusicDocument
      （绝对时间已转为整数微秒；TempoEvent/TimeSignatureEvent 有序数组）
  → scoreleap-arranger::arrange(document, options, profile) → CompiledSequence
      （转调 → 音域折叠 → 复音限制/和弦简化 → 量化 → Profile 映射 → 排序）
  → scoreleap-scheduler::Scheduler::new(sequence, backend, clock)
  → PlaybackCommand 驱动（Start/Pause/Resume/Stop/EmergencyStop）
  → InputBackend 事件（KeyDown/KeyUp 或 Gesture）
  → 释放全部按键（Stop/EmergencyStop 时）
```

## 4. Rust Workspace 结构

```toml
[workspace]
members = ["crates/*", "plugins/tauri-plugin-scoreleap-input", "apps/scoreleap/src-tauri"]
resolver = "2"
```

| crate | 职责 | 平台 |
|---|---|---|
| `scoreleap-music-ir` | MusicDocument/Track/NoteEvent/TempoEvent/TimeSignatureEvent；整数微秒时间 | 全平台 |
| `scoreleap-midi` | midly 0.5.3（Unlicense）解析 → Music IR；running status、velocity=0、SMPTE 处理 | 全平台 |
| `scoreleap-arranger` | 编排管线：转调、音域折叠、复音限制、量化、和弦简化 | 全平台 |
| `scoreleap-sequence` | CompiledSequence、PlatformAction、播放状态机类型 | 全平台 |
| `scoreleap-game-profile` | GameProfile schema（serde）、加载与校验 | 全平台 |
| `scoreleap-scheduler` | 虚拟时钟 + 精确调度；InputBackend trait；MockInputBackend | 全平台核心 |
| `scoreleap-core` | Tauri 命令编排、状态管理、错误聚合、日志、配置 | 桌面（Tauri） |
| `scoreleap-audio` | 音频解码（Symphonia，MPL-2.0）/重采样（rubato）（v0.4 启用） | 桌面 |
| `tauri-plugin-scoreleap-input` | 输入后端插件：desktop.rs（SendInput）/ mobile.rs（桥接 Kotlin） | Windows/Android |

## 5. Tauri 前后端通信

- 命令（invoke，单向请求/响应）：
  - `import_midi(path) → MusicDocumentSummary`
  - `get_tracks(doc_id)`
  - `compile(doc_id, ArrangementOptions) → CompiledSequenceSummary`
  - `start_playback(seq_id)` / `pause_playback()` / `resume_playback()` / `stop_playback()` / `emergency_stop()`
  - `set_speed(f64)` / `get_progress()`（进度查询；seek 属 v0.3 编辑器能力，不纳入 v0.1）
  - `list_profiles()` / `load_profile(id)`
  - `list_settings()` / `save_settings(...)`
  - `test_key(scancode, down)`（测试页）
- 事件（前端订阅）：
  - `playback://state`（PlaybackState）
  - `playback://progress`（PlaybackProgress：position_us、note、key count）
  - `playback://error`
- 大文件解析在后台线程（`tauri::async_runtime::spawn_blocking`——Tauri 2 自带 tokio runtime，**不引入独立 tokio 依赖**），解析期间 UI 不阻塞。

## 6. Android Kotlin 插件结构

```
plugins/tauri-plugin-scoreleap-input/
├── src/
│   ├── lib.rs          # 注册插件（desktop/mobile 分流）
│   ├── desktop.rs      # Windows 实现（SendInput backend）
│   ├── mobile.rs       # Rust → Kotlin 桥（tauri::plugin::mobile::PluginHandle）
│   └── models.rs       # PlatformAction / GestureAction 序列化
├── guest-js/           # JS API（window.__TAURI__.scoreleapInput）
├── android/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml      # accessibility service 声明
│       ├── kotlin/com/superdaobo/scoreleap/
│       │   ├── ScoreleapInputPlugin.kt    # Tauri 插件入口
│       │   ├── GestureDispatcher.kt       # dispatchGesture 封装
│       │   ├── PlaybackForegroundService.kt
│       │   └── ScoreleapAccessibilityService.kt
│       └── res/                    # 通知图标、字符串
└── kotlin 测试（instrumented）
```

## 7. Windows 输入后端

- `InputBackend` trait（定义于 `scoreleap-scheduler`）：

```rust
pub trait InputBackend: Send {
    fn key_down(&mut self, key: KeyCode) -> Result<(), BackendError>;
    fn key_up(&mut self, key: KeyCode) -> Result<(), BackendError>;
    fn release_all(&mut self) -> Result<(), BackendError>;
}
```

- 实现：
  - `SendInputBackend`：`SendInput` + `KEYEVENTF_SCANCODE`/`KEYEVENTF_KEYUP`；扩展扫描码用 `KEYEVENTF_EXTENDEDKEY` 标志（不拼 0xE0 进 wScan）；键位表优先 `MapVirtualKeyW(MAPVK_VK_TO_VSC_EX)` 运行时换算，静态表后备；
  - `MockInputBackend`：内存记录事件序列，供测试与虚拟时钟使用；
- UIPI：SendInput 只能注入同/低完整性级别进程——游戏以管理员运行时需提示用户以管理员运行 ScoreLeap；
- 按键集合跟踪：`HashSet<KeyCode>` 记录按下中的按键，`release_all` 遍历抬起；
- 崩溃兜底：`panic::set_hook` 与 Tauri 退出钩子调用 `release_all`（尽力而为，记录日志）；
- 窗口策略：演奏前检查前台窗口（`GetForegroundWindow`）是否等于用户确认的目标窗口（v0.1 先做「前台窗口确认」交互，不做自动窗口名匹配）。

## 8. Android 输入后端

- `mobile.rs` 通过 `PluginHandle` 调用 Kotlin：
  - `dispatch_gestures(Vec<GestureAction>)`（单点/多点）；
- Kotlin `GestureDispatcher`：
  - 单点：`GestureDescription.Builder().addStroke(Path)`；
  - 多点：构建多 Stroke 的 GestureDescription（按 API 能力评估，见调研结论）；
  - 回调：`GestureResultCallback` 上报完成/超时；
- `ScoreleapAccessibilityService`：仅实现手势分发所需接口；不读取节点内容、不收集数据；
- `PlaybackForegroundService`：前台通知（曲名/暂停/停止按钮）；播放结束自动退出；
- 坐标：`CalibrationProfile` 存归一化坐标，运行时乘以当前显示尺寸（WindowMetrics API 30+，旧版本兼容方案见 ADR-0005）。

## 9. Scheduler 设计

- 输入：`CompiledSequence`（已按时间排序的 `PlatformAction` 数组，整数微秒）；
- 时钟抽象：

```rust
pub trait Clock: Send {
    fn now_us(&self) -> i64;           // 单调时钟，微秒
}
pub struct SystemClock;                 // QPC（Windows）
pub struct VirtualClock { ... }         // 测试用：手动推进
```

- 线程模型：调度线程 + `std::sync::mpsc` 命令通道；不依赖 tokio（评估结论见调研）；
- 时钟实现：Windows 用 QPC（`scoreleap-scheduler` 的 SystemClock）；Android 端时钟源 v0.2 定义（桥接 System.nanoTime，见调研归档）；
- 调度算法：**deadline 驱动**主循环——按 `next_action_time_us - now_us` 精确等待（`timeBeginPeriod(1)` + 高分辨率等待计时器，详见 ADR-0005），到期执行并**基于 QPC 重算下一次绝对到期时间**（不周期累加，消除累积漂移）；
- 暂停：记录暂停点，释放所有按下按键；继续：重建持续音符按下状态；
- 速度：时间轴按 `speed` 缩放（`effective_time = (t - t0) / speed`）；
- 紧急停止：最高优先级命令，立即清空队列、释放全部按键、复位状态机；
- 虚拟时钟模式：测试直接推进时间，验证序列与暂停/恢复/停止语义。

## 10. Game Profile 设计

```jsonc
// game-profiles/identity-v/profile.json（示意）
{
  "id": "identity-v",
  "display_name": "Identity V 游戏乐器",
  "version": 1,
  "instrument": {
    "keys": 36,
    "midi_low": 60, "midi_high": 95,     // 乐器音域（MIDI note number）
    "max_polyphony": 4
  },
  "keymap_windows": "windows-keymap.json",
  "layout_android": "android-layout.json",
  "warning": "仅用于自由演奏/个人空间，注意游戏服务条款"
}
```

- 加载流程：serde 反序列化 → schema 校验（必填字段/范围）→ 映射表校验（音名唯一、扫描码有效）；
- 失败策略：返回结构化错误，UI 提示并禁用演奏；
- 公共接口只出现 `GameProfile`，不出现游戏名硬编码。

## 11. AI Worker 设计（v0.4 预留）

- 模块：`scoreleap-audio`（解码）+ `scoreleap-transcribe`（v0.4 新增，Basic Pitch ONNX）；
- 流水线：解码（Symphonia，MPL-2.0）→ 重采样（rubato）22050Hz mono → 分帧（2 秒/43,844 采样点窗口，Basic Pitch nmp.onnx 约 225KB，模型内自带 CQT）→ ONNX Runtime（ort crate，pykeio/ort，自动拉取预编译 ORT 库）CPU 推理 → 后处理（onset/velocity 合并、最低音符时长过滤）→ Music IR；
- 进程模型：推理在后台线程池，进度通过事件上报；
- 模型目录：`<data_dir>/models/`，manifest.json（名称/大小/SHA256/许可证/来源），下载校验后使用；onnxruntime.dll 随 `bundle.resources` 打包、绝对路径加载；
- v0.1/v0.2 不实现该模块（仅架构预留）。

## 12. 错误处理

- 统一错误类型：`scoreleap-core::Error`（thiserror 派生），分类：`Parse` / `Arrange` / `Profile` / `Backend` / `Scheduler` / `Config` / `Io`；
- 错误跨 IPC：序列化为 `{ code, message, details? }`，前端映射为可读文案；
- 关键路径（解析、编译、注入失败）必须结构化错误 + 日志；
- 可恢复错误（手势超时）与致命错误（后端失效）分级：致命 → 自动紧急停止。

## 13. 日志系统

- `tracing` + `tracing-subscriber`；输出到：
  - 终端（dev）；
  - 滚动日志文件：`<data_dir>/logs/scoreleap-YYYYMMDD.log`（保留 7 天）；
- 级别：默认 info，可配置 debug；
- 日志包含：启动信息、导入/编排摘要、播放会话（开始/暂停/停止/紧急停止原因）、后端错误；
- 隐私：默认不记录文件内容；文件名可配置脱敏。

## 14. 配置存储

- 首选项：`tauri-plugin-store`（JSON，`<data_dir>/settings.json`）：Profile 选择、快捷键、日志级别、风险确认状态、上次导入目录；
- 工程数据：`<data_dir>/library/`（导入曲谱的缓存副本 + 编译缓存）；
- Android 校准：`CalibrationProfile` 存应用私有存储；
- 敏感项：不存任何密钥（无账号体系）。

## 15. 测试架构

| 层 | 工具 | 范围 |
|---|---|---|
| Rust 单元测试 | `cargo test` | music-ir/midi/arranger/sequence/game-profile/scheduler（虚拟时钟） |
| Rust 集成测试 | `tests/` 目录 | 黄金样例（fixtures/midi + fixtures/sequences 期望结果） |
| 前端单测 | Vitest | Pinia stores、编排参数校验、错误映射 |
| 端到端 | Playwright（WebView 内） | 导入→预览→状态流转（后端用 Mock） |
| Windows 实机 | 手动 + 测试页 | SendInput 验证（不进 CI） |
| Android | Instrumented Test | GestureDispatcher 单元 + 实机验证（v0.2） |

约束：测试不得依赖真实按键注入；`MockInputBackend` + `VirtualClock` 是调度测试的唯一途径。

## 16. CI 架构

- `ci.yml`（PR 触发）：前端安装/typecheck/test、Rust fmt/clippy/test、构建验证；
- `windows-build.yml`：windows-latest 构建 NSIS 安装包，上传产物（v0.1 起）；
- `android-check.yml`：v0.2 起（Gradle + Rust Android target + debug APK + 单元测试）；
- `release.yml`：tag 触发，版本一致性检查、Windows 构建（v0.2 起含 Android）、SHA256、GitHub Release；v0.1 发布可先手工触发（V1-27），自动化流水线后续版本评估；
- 原则：先保证检查稳定，再增加缓存/矩阵/发布自动化。

## 17. 发布架构

- Windows：NSIS 安装包（`tauri-action` / `tauri build`），无证书时记录 SmartScreen 风险；
- 版本一致性：`package.json`、`Cargo.toml`、`tauri.conf.json`、Android 版本号同步（脚本检查）；
- 产物：安装包 + `.sig`/SHA256 文件 + Release Notes（自动生成自 PR 标题聚合）；
- 模型资产（v0.4）：独立 Release 附件 + manifest 校验。

---

## 附录 A：核心公共类型（草案，Agent A 管理）

所有时间字段单位明确标注：`*_us` = 整数微秒；禁止核心调度使用浮点秒。

```rust
// music-ir
pub struct MusicDocument { pub format: MidiFormat, pub tracks: Vec<Track>, pub tempo_events: Vec<TempoEvent>, pub time_signature_events: Vec<TimeSignatureEvent>, pub duration_us: i64 }
pub struct Track { pub id: u16, pub name: String, pub notes: Vec<NoteEvent>, pub instrument: Option<String> }
pub struct NoteEvent { pub track_id: u16, pub note: u8, pub velocity: u8, pub start_us: i64, pub duration_us: i64 }
pub struct TempoEvent { pub time_us: i64, pub tempo_us_per_quarter: u32 }
pub struct TimeSignatureEvent { pub time_us: i64, pub numerator: u8, pub denominator: u8 }

// arranger
pub struct ArrangementOptions { pub transpose_semitones: i8, pub auto_fit_range: bool, pub range_strategy: RangeStrategy, pub max_polyphony: u8, pub quantize_grid: Option<QuantizeGrid>, pub simplify_chords: bool }

// game-profile
pub struct GameProfile { pub id: String, pub keys: u8, pub midi_low: u8, pub midi_high: u8, pub max_polyphony: u8, pub keymap: HashMap<u8, KeyCode>, pub layout: InstrumentLayout }
pub struct InstrumentLayout { pub keys: Vec<KeySlot> }  // KeySlot: 归一化坐标 (x, y) + 音名

// sequence（Playback 状态机类型亦属此 crate）
pub struct CompiledSequence { pub actions: Vec<PlatformAction>, pub duration_us: i64, pub meta: SequenceMeta }
pub enum PlatformAction { KeyDown { at_us: i64, key: KeyCode }, KeyUp { at_us: i64, key: KeyCode }, Gesture { at_us: i64, points: Vec<Point>, kind: GestureKind } }
pub enum GestureKind { Tap, LongPress, Chord }  // Android 后端；LongPress 用 continueStroke/willContinue 续接
pub enum KeyCode { Scan(u16), ExtendedScan(u16) }  // 平台无关按键标识（Windows 扫描码承载；序列化稳定）
pub enum PlaybackState { Idle, Countdown, Playing, Paused, Stopped, Finished }
pub enum PlaybackCommand { Start, Pause, Resume, Stop, EmergencyStop }
pub struct PlaybackProgress { pub position_us: i64, pub current_note: Option<NoteEvent>, pub pressed_keys: u32 }

// calibration
pub struct CalibrationProfile { pub name: String, pub device: String, pub resolution: (u32, u32), pub anchors: Vec<KeyAnchor> }
```

## 附录 B：目录结构总览

```
scoreleap/
├── apps/scoreleap/            # Tauri 2 + Vue 3 应用
├── crates/                    # Rust workspace
├── plugins/tauri-plugin-scoreleap-input/
├── game-profiles/identity-v/
├── models/                    # AI 模型 manifest（v0.4）
├── fixtures/                  # midi/sequences/profiles 黄金样例
├── docs/  planning/  scripts/
└── .github/                   # 模板 + workflows
```
