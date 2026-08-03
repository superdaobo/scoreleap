# 版本路线图 — ScoreLeap（谱跃）

> 版本化路线图。每个版本的入口 = 对应 Epic Issue；范围变更必须通过 Issue 讨论，不得私下扩大。

## 版本一览

| 版本 | 主题 | 平台 | 关键依赖 | 目标 |
|---|---|---|---|---|
| v0.1.0 | Windows MIDI MVP | Windows | 无（纯本地） | 端到端可演奏 |
| v0.2.0 | Android MIDI MVP | Android | v0.1 的 Music IR/编排/时间轴 | 触屏演奏可用 |
| v0.3.0 | Score Editor | 全平台 | v0.1/v0.2 | 可编辑工程 |
| v0.4.0 | Audio Transcription | Windows | v0.3；Symphonia + ONNX Runtime | 音频转 MIDI |
| v0.5.0 | Android AI | Android | v0.4 研究结论 | 移动端 AI 或在线备选 |

## v0.1.0 — Windows MIDI MVP（首发）

**范围**：见 PRODUCT_PLAN.md 第 7 节（必须实现清单）与第 21 节（不做什么）。

**里程碑拆解（27 项，Issue 化）**：

1. 初始化 Tauri 2、Vue 3、Rust Workspace 和 pnpm Workspace
2. 建立项目配置、格式化、Lint 和测试框架
3. 定义 Music IR
4. 实现 MIDI 解析和 Tempo Map
5. 定义 Game Profile Schema
6. 创建首个 36 键游戏乐器 Profile
7. 实现自动转调
8. 实现音域折叠
9. 实现节奏量化
10. 实现和弦简化
11. 实现 CompiledSequence
12. 实现精确 Scheduler
13. 完成 Windows SendInput 技术验证
14. 实现 Windows 输入 Backend
15. 实现紧急停止和全部按键释放
16. 实现 MIDI 导入页面
17. 实现轨道选择页面
18. 实现转换参数页面
19. 实现钢琴卷帘预览
20. 实现播放控制界面
21. 实现全局快捷键
22. 建立黄金 MIDI 测试样例
23. 建立单元测试和集成测试
24. 建立 GitHub Actions
25. 构建 Windows 安装包
26. 编写用户文档
27. 发布 v0.1.0

**退出标准**：PRODUCT_PLAN.md 第 24 节 18 条验收标准全部满足；CI 全绿；NSIS 安装包产出。

## v0.2.0 — Android MIDI MVP

**范围**：Android 触摸演奏（依赖 v0.1 核心）。

**里程碑拆解（18 项，Issue 化）**：

1. 初始化 Android Tauri 构建
2. 创建 tauri-plugin-scoreleap-input
3. Android Kotlin 插件桥接
4. AccessibilityService 技术验证
5. GestureDescription 单音验证
6. GestureDescription 多点和弦验证
7. Foreground Service
8. 通知栏暂停和停止
9. Android 横屏坐标系统
10. 琴键校准界面
11. 标准化坐标 Profile
12. Android Playback Scheduler
13. 设备旋转与分辨率变化处理
14. 切换应用后的生命周期处理
15. Android 安全停止
16. Android 实机测试
17. Android CI
18. APK 构建和发布

**前置依赖**：v0.1.0 的 `scoreleap-music-ir`、`scoreleap-arranger`、`scoreleap-sequence`、`scoreleap-scheduler`（核心与平台无关部分）。

## v0.3.0 — Score Editor

**范围**：

- 钢琴卷帘编辑（删除/移动音符、修改时长）
- 轨道静音、主旋律选择
- 和弦简化预览、转调预览
- `.scoreleap` 工程文件（导入/导出）
- 撤销与重做

**入口 Epic**：`[EPIC] v0.3.0 Score Editor`（Issue 拆解在阶段四生成）。

## v0.4.0 — Audio Transcription（Windows）

**范围**：

- Basic Pitch ONNX 技术验证（先研究、后实现；官方 nmp.onnx 约 225KB，Apache-2.0 可随包分发）
- Windows ONNX Runtime 集成与打包（ort crate 自动拉取预编译 ORT；onnxruntime.dll 入 bundle.resources）
- 音频解码（Symphonia，MPL-2.0）与重采样（rubato，22050Hz mono）
- 分段推理、转录置信度、音符后处理
- 模型下载管理（manifest + SHA256 校验）
- 转录结果编辑（复用 v0.3 编辑器）
- Piano Transcription 可行性研究
- Demucs 可选音源分离研究（仅研究）

**约束**：模型许可证确认 + 体积评估 + 性能测试通过前，不得进入安装包。

## v0.5.0 — Android AI

**范围（先研究，后决定实现与否）**：

- ONNX Runtime Mobile
- 模型裁剪
- Android 内存占用、分段推理
- 温度和耗电
- 前台任务
- 低性能设备降级
- 可选在线转换服务（用户显式选择）

**退出标准**：研究 Issue 全部关闭，输出实现/放弃决策记录。

---

## 通用发布规则

1. 每个版本 = 一个里程碑 + 一个版本 Epic；
2. 版本内所有 Issue 完成（含测试、文档、CI）才允许发布；
3. 发布 = tag `vX.Y.Z` + release.yml 自动构建 + Release Notes + SHA256；
4. 版本范围变更需更新本文件并走 PR 审查。
