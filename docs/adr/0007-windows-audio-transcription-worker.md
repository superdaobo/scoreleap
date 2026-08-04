# ADR-0007：Windows 音频转录 Worker（Basic Pitch Sidecar）

- 状态：已接受（2026-08-05，基于 Spike #49 实测数据）
- 关联：Epic #44

## 1. 为什么选择 Basic Pitch

Spike 实测（Windows + Python 3.11.15，basic-pitch 0.4.0）：

- 25s MP3 → 174 音符 / 2 轨 / 24.8s 时长，输出 MIDI **可被现有 `scoreleap-midi` 成功解析**；
- 模型**随 pip 包分发**（`basic_pitch/saved_models`，1.9MB，SavedModel）——无需额外下载、打包可收集；
- 单模型同时输出旋律与多音（polyphonic piano transcription），适合游戏乐器编排场景；
- MIT 许可证（GitHub 仓库声明），无商业限制。

## 2. 为什么 MVP 使用 Python Worker

- Basic Pitch 官方推理栈基于 tensorflow/keras，Rust 侧无稳定同等实现；
- Python Worker 以独立进程隔离模型崩溃与内存（峰值 ~522MB），主程序稳定性不受影响；
- 可独立测试、独立打包（PyInstaller）、后续可整体替换（见 14）。

## 3. 为什么输出 MIDI

ScoreLeap 已有完整的 MIDI 解析 → MusicDocument → 编排 → 曲谱库 → 播放管线。输出标准 MIDI 使转录结果**零改动进入现有管线**，避免维护两套音符数据格式；用户也可导出/保留生成的 MIDI。metadata.json 仅作补充（来源、耗时、音符数），不承载主要音乐数据。

## 4. 为什么复用现有 MIDI 管线

- 同一解析器（`scoreleap-midi`）、同一 MusicDocument、同一曲谱库、同一编排器、同一 Scheduler、同一 Profile；
- 转录 MIDI 与直接导入 MIDI 走同一 `import_midi` 业务入口（Issue #46 提取共享函数）；
- 不实现第二套演奏系统（禁止事项）。

## 5. 为什么不在 MVP 使用 Demucs（音源分离）

- Demucs 模型体积大（数百 MB）、推理耗时明显高于 Basic Pitch，且 MVP 目标是"旋律清晰的音频"；
- 音源分离是独立的算法问题，不影响"转录→MIDI→编排"主链路；
- 已列为后续 Issue（Epic 路线图 v0.4 范围外）。

## 6. 为什么不直接实现 Rust ONNX

- basic-pitch 官方模型为 SavedModel（tensorflow 格式）；[onnx] 安装路径存在（onnxruntime），但 Spike 未验证其精度/速度与模型转换可行性；
- Rust onnxruntime crate 依赖 C 库分发与模型转换工具链，MVP 周期内风险高；
- 保留迁移路径（见 14），MVP 用 Python Worker 快速交付。

## 7. Sidecar 通信协议

- 启动：Rust `TranscriptionService` 以**参数数组**启动 Worker（禁止 shell=true / 字符串拼接 / cmd /c / PowerShell 执行用户输入）；
- stdout 仅输出 **JSON Lines**（每行一个完整 JSON 对象）：`ready` / `stage` / `result` / `error`；
- 字段：`schema_version`、`type`、`request_id`、`timestamp_ms`、`stage`、`message`、`data`；
- 日志走 stderr（Rust 侧转写日志文件）；Python Traceback 不进 UI；
- 协议版本随 Worker 版本：`worker_version`（见 Issue #45 实现）。

## 8. onefile 与 onedir 比较（Spike 实测）

| 项 | onefile | onedir |
|---|---|---|
| 体积 | 390.5 MB（PKG 压缩） | 1204.4 MB（解压目录） |
| 启动+转录（25s 音频） | 41.7s（每次启动临时解压） | 23.1s |
| 中文+空格路径 | 未测 | ✅ 通过 |
| 崩溃诊断 | 差（临时解压目录） | 好（文件就地） |
| 杀毒误报风险 | 高（单文件自解压模式常见误报） | 相对低 |
| Tauri 资源打包 | 单文件简单 | 目录打包（externalBin） |

## 9. 最终打包决策：PyInstaller **onedir**

理由：启动快（无每次解压）、崩溃可诊断、杀软误报相对低、Tauri `externalBin` 支持目录形式；体积差异（解压态）在 NSIS 压缩后差异有限。onefile 仅在需要单文件分发场景保留为备选。

## 10. 许可证

- basic-pitch 0.4.0：MIT（GitHub 声明；PyPI metadata 缺失，已记录）；
- tensorflow 2.15.0：Apache-2.0；librosa：ISC；mido：MIT；pretty-midi：MIT；certifi：MPL-2.0；
- 完整依赖清单随 `tools/transcription-worker/requirements.lock.txt` 提交（Issue #45）。

## 11. 安装包体积

- Worker onedir 解压态 ~1204 MB；NSIS 压缩后预计 400–500 MB；
- ScoreLeap 安装包预计从 ~2.7 MB 增至 ~400+ MB（主要增量 tensorflow）；
- 后续 onnxruntime 路线预计 <200 MB（见 14）。

## 12. 性能测量（Spike 实测）

- 首次转录（含模型加载）：31.1s；第二次：10.5s（25s 音频）；
- 峰值内存：~522 MB（tensorflow CPU）；
- 模型加载无法分段 → UI 使用阶段进度 + 不确定进度条，不伪造百分比。

## 13. 已知限制

- 完整歌曲会出现鼓点/伴奏/人声杂音符（无音源分离）；
- 钢琴独奏/旋律清晰音频效果最佳；
- tensorflow 体积大、启动慢；
- 一次仅一个转录任务（TRANSCRIPTION_BUSY）；
- 仅 Windows。

## 14. 后续迁移到原生 ONNX 的路径

1. basic-pitch `[onnx]` 安装路径验证精度/速度（现有代码路径相同，仅推理后端切换）；
2. 模型导出 ONNX 后评估 Rust onnxruntime crate；
3. Worker 协议不变，仅替换 Python Worker 内部实现或整体替换为 Rust 实现；
4. 预期收益：体积 <200MB、启动快、免 Python 运行时。

## 15. 回滚

- 功能开关：转录入口标记"实验"；回滚 = 移除入口 + 不注册转录命令；
- Worker 替换：Sidecar 路径配置化，指向旧 Worker 即可；
- 曲谱库兼容：转录导入的曲谱与普通 MIDI 完全同构（source_type 字段 serde default 向后兼容），无迁移成本。
