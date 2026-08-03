# Issue 地图 — ScoreLeap（谱跃）

> 规划编号（`V1-xx`）用于阶段四创建前的依赖规划；创建时映射为实际 GitHub Issue 编号并回填本表。
> 每个 Issue 草案见 `planning/issue-drafts/`。创建顺序严格按 `dependency-graph.md` 的 DAG。

## v0.1.0 Windows MIDI MVP（28 项）

| 规划编号 | 标题 | 类型 | 区域 | 优先级 | 依赖 |
|---|---|---|---|---|---|
| V1-01 | 初始化 Tauri 2、Vue 3、Rust Workspace 和 pnpm Workspace | type:feature | area:core | P0 | — |
| V1-02 | 建立项目配置、格式化、Lint 和测试框架 | type:chore | area:ci | P0 | V1-01 |
| V1-03 | 定义 Music IR | type:feature | area:core | P0 | V1-01 |
| V1-04 | 实现 MIDI 解析和 Tempo Map | type:feature | area:midi | P0 | V1-03 |
| V1-05 | 定义 Game Profile Schema | type:feature | area:profile | P0 | V1-01 |
| V1-06 | 创建首个 36 键游戏乐器 Profile | type:feature | area:profile | P0 | V1-05 |
| V1-07 | 实现自动转调 | type:feature | area:arranger | P1 | V1-03 |
| V1-08 | 实现音域折叠 | type:feature | area:arranger | P1 | V1-03 |
| V1-09 | 实现节奏量化 | type:feature | area:arranger | P1 | V1-03 |
| V1-10 | 实现和弦简化 | type:feature | area:arranger | P1 | V1-03 |
| V1-11 | 实现 CompiledSequence | type:feature | area:core | P0 | V1-03、V1-05 |
| V1-12 | 实现精确 Scheduler | type:feature | area:scheduler | P0 | V1-11 |
| V1-13 | 完成 Windows SendInput 技术验证 | type:research | area:windows | P0 | V1-01 |
| V1-14 | 实现 Windows 输入 Backend | type:feature | area:windows | P0 | V1-12、V1-13、V1-06 |
| V1-15 | 实现紧急停止和全部按键释放 | type:feature | area:windows | P0 | V1-14 |
| V1-16 | 实现 MIDI 导入页面 | type:feature | area:ui | P0 | V1-04 |
| V1-17 | 实现轨道选择页面 | type:feature | area:ui | P1 | V1-16 |
| V1-18 | 实现转换参数页面 | type:feature | area:ui | P0 | V1-07、V1-08、V1-09、V1-10 |
| V1-19 | 实现钢琴卷帘预览 | type:feature | area:ui | P0 | V1-11 |
| V1-20 | 实现播放控制界面 | type:feature | area:ui | P0 | V1-12、V1-14、V1-15 |
| V1-21 | 实现全局快捷键 | type:feature | area:windows | P1 | V1-15 |
| V1-22 | 建立黄金 MIDI 测试样例 | type:test | area:midi | P1 | V1-04 |
| V1-23 | 建立单元测试和集成测试 | type:test | area:ci | P0 | V1-12、V1-22、V1-07、V1-08、V1-09、V1-10、V1-15、V1-16、V1-21、V1-28（测试矩阵收口，覆盖对象全部完成后实施） |
| V1-24 | 建立 GitHub Actions | type:chore | area:ci | P0 | V1-02 |
| V1-25 | 构建 Windows 安装包 | type:chore | area:release | P1 | V1-24 |
| V1-26 | 编写用户文档 | type:docs | area:release | P1 | V1-20 |
| V1-27 | 发布 v0.1.0 | type:chore | area:release | P0 | V1-25、V1-26 |
| V1-28 | 实现设置页面与本地日志系统 | type:feature | area:ui | P1 | V1-02、V1-21 |

## v0.2.0 Android MIDI MVP（18 项，阶段四创建 Epic，拆解在 v0.1 发布后进行）

| 规划编号 | 标题 | 依赖 |
|---|---|---|
| V2-01 | 初始化 Android Tauri 构建 | V1-01 |
| V2-02 | 创建 tauri-plugin-scoreleap-input | V2-01 |
| V2-03 | Android Kotlin 插件桥接 | V2-02 |
| V2-04 | AccessibilityService 技术验证 | V2-03 |
| V2-05 | GestureDescription 单音验证 | V2-04 |
| V2-06 | GestureDescription 多点和弦验证 | V2-05 |
| V2-07 | Foreground Service | V2-03 |
| V2-08 | 通知栏暂停和停止 | V2-07 |
| V2-09 | Android 横屏坐标系统 | V2-03 |
| V2-10 | 琴键校准界面 | V2-09 |
| V2-11 | 标准化坐标 Profile | V2-10 |
| V2-12 | Android Playback Scheduler | V1-12、V2-03 |
| V2-13 | 设备旋转与分辨率变化处理 | V2-11 |
| V2-14 | 切换应用后的生命周期处理 | V2-07 |
| V2-15 | Android 安全停止 | V2-12 |
| V2-16 | Android 实机测试 | V2-05、V2-06、V2-15 |
| V2-17 | Android CI | V2-03 |
| V2-18 | APK 构建和发布 | V2-16、V2-17 |

## v0.3.0 / v0.4.0 / v0.5.0（阶段四仅创建 Epic，具体 Issue 在版本启动时拆解）

| Epic | 规划状态 |
|---|---|
| `[EPIC] v0.3.0 Score Editor` | 仅 Epic（拆解时机：v0.2 发布后） |
| `[EPIC] v0.4.0 Audio Transcription` | 仅 Epic（拆解时机：v0.3 发布后） |
| `[EPIC] v0.5.0 Android AI` | 仅 Epic + 研究 Issue（拆解时机：v0.4 发布后） |

## 总路线图 Epic

`[EPIC] ScoreLeap 产品路线图与跨平台架构` — 任务列表引用各版本 Epic。

## Issue 编号映射（阶段四创建后回填）

| 规划编号 | 实际 Issue | 标题 |
|---|---|---|
| V1-01 | #8 | 初始化 Tauri 2、Vue 3、Rust Workspace 和 pnpm Workspace |
| V1-02 | #9 | 建立项目配置、格式化、Lint 和测试框架 |
| V1-03 | #10 | 定义 Music IR |
| V1-04 | #11 | 实现 MIDI 解析和 Tempo Map |
| V1-05 | #12 | 定义 Game Profile Schema |
| V1-06 | #13 | 创建首个 36 键游戏乐器 Profile |
| V1-07 | #14 | 实现自动转调 |
| V1-08 | #15 | 实现音域折叠 |
| V1-09 | #16 | 实现节奏量化 |
| V1-10 | #17 | 实现和弦简化 |
| V1-11 | #18 | 实现 CompiledSequence |
| V1-12 | #19 | 实现精确 Scheduler |
| V1-13 | #20 | 完成 Windows SendInput 技术验证 |
| V1-14 | #21 | 实现 Windows 输入 Backend |
| V1-15 | #22 | 实现紧急停止和全部按键释放 |
| V1-16 | #23 | 实现 MIDI 导入页面 |
| V1-17 | #24 | 实现轨道选择页面 |
| V1-18 | #25 | 实现转换参数页面 |
| V1-19 | #26 | 实现钢琴卷帘预览 |
| V1-20 | #27 | 实现播放控制界面 |
| V1-21 | #28 | 实现全局快捷键 |
| V1-22 | #29 | 建立黄金 MIDI 测试样例 |
| V1-23 | #30 | 建立单元测试和集成测试 |
| V1-24 | #31 | 建立 GitHub Actions |
| V1-25 | #32 | 构建 Windows 安装包 |
| V1-26 | #33 | 编写用户文档 |
| V1-27 | #34 | 发布 v0.1.0 |
| V1-28 | #35 | 实现设置页面与本地日志系统 |

