# 技术调研结论归档 — ScoreLeap（谱跃）

> 阶段一 5+2 路并行调研的结论摘要（官方来源为准），供后续开发阶段直接引用。
> 更新规则：实现阶段如发现与结论冲突，先更新本文档再改实现。

---

## 1. 音乐核心（midly 等）

| 项 | 结论 |
|---|---|
| 解析库 | **midly 0.5.3**（Unlicense/公共领域，零拷贝，性能最佳：24MB 文件约 60ms） |
| 格式支持 | 格式 0/1/2；`Smf::parse` 物化 / `midly::parse` 惰性迭代 |
| Tempo | `MetaMessage::Tempo(u24)` = 每四分音符微秒，默认 500000µs（120 BPM）；tempo map 需自行构建 |
| Running status | midly 解析与写入均支持 |
| velocity=0 | midly **保留原样**，按惯例语义（视为 NoteOff）必须由 `scoreleap-midi` 自行归一化 |
| SMPTE | `Timing::Timecode(Fps, u8)`：`us_per_tick = 1_000_000 / (fps × subframe)` |
| 时间换算 | Metrical：`abs_us += delta_ticks × tempo_us / division`；建议 u128 中间量或 checked_mul（tick ≤ 2²⁸−1，i64 安全但有防御必要） |
| 备选 | wmidi（单条消息编解码，MIT）、midir（**MIDI 硬件 I/O**，非解析库）——均不替代 midly |
| MusicXML | 仅 hedgetechllc/musicxml 1.1.2（MIT，W3C 4.0 全实现）可用但未稳定；**v0.1 不做**；v0.3 评估 |
| tokio | **不需要独立 tokio 依赖**：Tauri 2 自带（`tauri::async_runtime`，spawn/spawn_blocking/channel）；CPU 密集任务用 spawn_blocking 或 std 线程 + mpsc |
| Workspace | 单向依赖：`music-ir`（零依赖）← `midi`/`arranger` ← `sequence` ← 应用；公共类型只定义在 music-ir |

## 2. Windows 输入后端

| 项 | 结论 |
|---|---|
| 注入 API | **SendInput + KEYEVENTF_SCANCODE + KEYEVENTF_KEYUP**（keybd_event 已被官方标记 superseded） |
| 扫描码 | 扫描码与键盘布局无关；官方 Scan 1 表（A=0x1E、Q=0x10…）；`MapVirtualKeyW(MAPVK_VK_TO_VSC_EX)` 可运行时 VK→扫描码（含 E0 高字节），比硬编码表稳 |
| 扩展码 | `KEYEVENTF_EXTENDEDKEY` 标志（不要把 0xE0 拼进 wScan）；Pause 用 E1 |
| UIPI | SendInput **只能注入同/低完整性级别**：游戏以管理员运行时 ScoreLeap 也需管理员权限（文档需说明） |
| 游戏兼容 | DirectInput 官方不推荐用于键鼠；现代引擎走消息/Raw Input，SendInput 均可达；个别旧引擎需实机验证（H1/H7） |
| 冲突 | 注入前可用 GetAsyncKeyState 检测用户手按（v0.1 可选） |
| 调度 | **单线程 deadline 驱动**：QPC/QPF 唯一时钟源（QPF 缓存一次）；播放期间 `timeBeginPeriod(1)`/`timeEndPeriod(1)` 成对调用（Win10 2004+ 仅影响本进程；不影响 QPC 精度）；唤醒用 `CreateWaitableTimerExW(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION)` + SetWaitableTimer + WaitForSingleObject（Win10 1803+，旧系统回退普通定时器）；**每次触发后按 QPC 重算绝对到期时间**（不周期累加）消除漂移；误差预期 1–3ms；可选 MMCSS 提优先级；**弃用 CreateTimerQueueTimer（线程池延迟不可控）与 timeSetEvent（官方 obsolete）** |
| 快捷键 | **tauri-plugin-global-shortcut 2.3.x**（内部隐藏消息窗口 + RegisterHotKey + WM_HOTKEY；支持 Ctrl/Alt/Shift/Win；冲突返回 AlreadyRegistered）；紧急停止默认 **Ctrl+Alt+F9**（避开系统保留组合如 Ctrl+Shift+Esc=任务管理器、Win 键系统组合；F12 留给调试器）；不用手写 RegisterHotKey |
| 崩溃恢复 | **无系统级兑底**（SendInput 不重置键盘状态）；分层缓解：panic hook + SetUnhandledExceptionFilter（保留 WER）+ 退出钩子 release_all；**启动自检**（状态文件标记异常退出 → GetAsyncKeyState 核对补抬）；文档指引「按键卡住按一次该键解除」 |
| windows crate | **0.62.2**（MIT OR Apache-2.0，Rust ≥1.82）；features：`Win32_UI_Input_KeyboardAndMouse`（SendInput/RegisterHotKey）、`Win32_System_Performance`（QPC）、`Win32_System_Threading`（WaitableTimer）、`Win32_Media_Multimedia`（timeBeginPeriod）、`Win32_System_Diagnostics_Debug`（SetUnhandledExceptionFilter）、`Win32_Security`（TokenIntegrityLevel）；`Win32_Foundation` 自动启用 |
| UIPI 检测 | `GetTokenInformation(TokenIntegrityLevel)` 读自身与目标窗口进程（GetWindowThreadProcessId + OpenProcess）完整性级别（medium/high SID）；目标 > 自身时 UI 明确提示（以管理员运行谱跃或普通权限运行游戏），不做提权绕过 |

## 3. Android（可行性确认）

| 项 | 结论 |
|---|---|
| 插件结构 | `npx @tauri-apps/cli plugin new [name] --android`（官方推荐，模板仓库已 404）；android/ 含 build.gradle.kts + src/main（Kotlin + AndroidManifest.xml）；Kotlin 类继承 `app.tauri.plugin.Plugin` + `@TauriPlugin`，方法 `@Command`（主线程执行，长任务需 CoroutineScope(Dispatchers.IO)） |
| Rust→Kotlin | `PluginHandle::run_mobile_plugin("cmd", payload)`（serde 自动 camelCase）；`invoke.resolve(JSObject)/reject` |
| Kotlin→JS/Rust | `Plugin.trigger("event", JSObject)` → JS `addPluginListener`；**Kotlin 主动回调 Rust 无官方直连 API**（可用 JNI 或经 JS 中转）；高频有序流用 JS `Channel<T>` |
| dispatchGesture | **API 24+**；`GestureDescription` 由 1–20 个 Stroke 组成（Builder.addStroke 多次调用）；**官方指南明确支持 multi-touch**；坐标 = **屏幕像素**（display 绝对坐标）；单手势时长 ≤ 60s；零长度 Path = tap；**长按用 continueStroke(willContinue=true) 跨手势续接**；需 `android:canPerformGestures="true"`；回调 onCompleted/onCancelled |
| 游戏注入 | 官方无「仅限 View 层」声明（走系统输入管道），对 SurfaceView/Unity 需**实机验证（H3）**；官方未背书 |
| 前台服务 | Android 8+：startForegroundService 后 5s 内必须 startForeground；**API 34+**：manifest 声明 `android:foregroundServiceType` + `FOREGROUND_SERVICE_SPECIAL_USE` 权限 + PROPERTY_SPECIAL_USE_FGS_SUBTYPE 用途说明；演奏会话期间启动 FGS + 常驻通知（暂停/停止），非演奏不启动 |
| 坐标 | WindowMetrics.getCurrentWindowMetrics().getBounds()（API 30+，含系统栏）/ Display.getRealMetrics()（旧）；横屏 = 当前 orientation 逻辑尺寸；AccessibilityService 自有 Context 可查显示尺寸（不依赖 Activity 可见）；旋转/折叠屏用 onConfigurationChanged/registerDisplayListener 刷新 |
| 测试 | 插桩测试（androidTest + AndroidJUnitRunner）；**UiAutomator 可向其他应用注入真实触摸**（坐标级冒烟）；但 dispatchGesture 本身需真实启用的服务绑定，插桩进程无法直接驱动——手势生成逻辑放 Rust 单测，实机手动验证演奏正确性；Tauri 官方测试只覆盖 Rust 侧（mock runtime + WebDriver） |
| 进程被杀 | 服务被杀 → 手势自然停止（ADR 已接受）；FGS 提升存活概率；Tauri app 退后台、游戏在前台时进程仍在，手势可继续 |

## 4. AI 转录与音频

| 项 | 结论 |
|---|---|
| 解码 | **Symphonia**（MPL-2.0）：MP3/WAV/FLAC/OGG；重采样用 **rubato**（Symphonia 无自带重采样器） |
| Basic Pitch | Spotify 开源（代码 Apache-2.0）；官方 ONNX 模型 **nmp.onnx 约 225KB**（模型内自带 CQT），输入 22050Hz mono、2 秒（43,844 采样点）窗口；官方论文基准 7:45 音频 TF 推理 24s，ONNX 可达近实时 |
| ONNX Runtime | **ort crate（pykeio/ort）**：自动拉取预编译 ORT 库（如 1.28），CPU 推理；`onnxruntime.dll` + VC 运行库随 `bundle.resources` 分发，绝对路径加载 |
| Demucs / Piano Transcription | **无官方 ONNX 导出且体积大，v0.4 不引入**；留作后续「音源分离/钢琴高精度」选项（研究 Issue） |
| 模型再分发 | Basic Pitch（Apache-2.0）、Piano Transcription 代码（Apache-2.0）、piano 权重（CC-BY-4.0 需署名）、Demucs（MIT）——均可再分发，需保留 LICENSE/NOTICE |
| 分发 | 模型不进 Git；tauri updater 只用于应用自身（Ed25519 非 SHA256）；模型下载+校验需自建（reqwest/tauri-plugin-http + sha2 + manifest） |

## 5. 合规与发布

| 项 | 结论 |
|---|---|
| GPL-3.0 义务 | 随发布物附许可证全文 + 第三方声明（THIRD_PARTY_NOTICES）；GitHub Release 附源码链接（满足 §6(d)）；源文件头加版权与许可证指针；静态链接 Rust crate 传染明确 |
| 网易 ToS | 封号风险**确定性高**（「外挂/辅助软件/脚本」兜底条款）；民事/刑事风险低；宣传口径避免「代打/竞技优势」；保留协议版本快照 |
| Android Play 政策 | automation tools **不属于** accessibility tools（不可设 isAccessibilityTool=true）；**确定性规则脚本允许**（「If X then Y」），自主决策禁止；非无障碍用途须应用内显著披露 + 肯定式同意 + Play Console 声明；违规可下架/封号。本产品**不上架 Google Play** |
| 音乐版权 | MIDI 文件可受版权保护（原创编曲时）；用户自备文件、零曲库、不上传 → 项目风险低 |
| 发布 | tauri-action@v1 支持 Windows NSIS + GitHub Release；**不产出 .sha256 需自行生成**（Get-FileHash）；无证书 SmartScreen「未知发布者」警告（非阻塞） |
| 依赖审计 | **cargo-deny**（默认拒绝制 + SPDX 白名单，`EmbarkStudios/cargo-deny-action@v1`）；前端 **license-checker**（--onlyAllow 白名单）；白名单建议：MIT/Apache-2.0/BSD-2/3/ISC/Zlib/CC0-1.0/Unicode-DFS-2016/GPL-3.0/**Unlicense（midly）/MPL-2.0（Symphonia）** |
| 隐私 | 本地处理下 GDPR Art.2(2)(c) 与个保法 §72 基本豁免；一旦遥测则触发全套义务 → **默认零遥测、零网络** |

## 6. 关键待验证项（实现阶段）

- H1/H7：SendInput 扫描码在目标游戏实机验证（v0.1 Issue V1-13）；
- H3：dispatchGesture 对 SurfaceView/Unity 游戏实机验证 + 多指（v0.2 首个技术验证）；
- 长按续接（continueStroke/willContinue）实机验证（v0.2）；
- MMCSS 线程优先级是否启用（调度精度实测后决定）。
