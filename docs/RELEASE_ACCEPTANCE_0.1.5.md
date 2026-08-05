# ScoreLeap 0.1.5 Windows RC 验收报告

日期：2026-08-05 ｜ 分支：`integration/audio-transcription-onnx` ｜ 安装包：
`target/release/bundle/nsis/ScoreLeap_0.1.5_x64-setup.exe`（已复制到 `D:\tmp\scoreleap-eval\ScoreLeap_0.1.5_RC-setup.exe`）

## 1. 完成清单

| 项目 | 结果 |
|---|---|
| 原生 ONNX 转录（Basic Pitch nmp.onnx + ONNX Runtime） | ✅ 与官方 TensorFlow 实现逐音符等价 |
| MAESTRO 独立真值验证（5 样本，train/val/test） | ✅ 等价性 F1 0.9992/1.0（ADR-0008） |
| 8月4日.MP3 校准区（0-120s）/ 隐藏区（120-199.393s） | ✅ 分区 + manifest |
| 三预设校准冻结（balanced/detail/noise_reduced） | ✅ balanced 与官方 F1=1.0（ADR-0008） |
| 20dB 粉红噪声鲁棒性基线 | ✅ 干净 vs 加噪一致 F1 0.866/0.776 |
| Windows 安装后 E2E（安装→转录→断网可用） | ✅ 完整 8月4日.MP3 1514 音符，无 AUDIO_DECODE_FAILED |
| 格式一致性（MP3/WAV/FLAC） | ✅ 三者一致 F1 全 1.0000（门槛 0.995/0.97） |
| 性能门禁 | ✅ 峰值内存 126MB（≤1GB）、实时系数 46.7x（≤0.5 门槛） |
| 三路只读审查 | ✅ 修复 P1（minimum-note 参数名）+ 新增测试 |
| 全量门禁 | ✅ Rust 全工作区测试全绿、前端 30/30、评测工具 19/19 |

## 2. 自动门禁结果

- 实现等价性：官方 Basic Pitch vs 原生实现 一致 F1 ≥ 0.99 ✅（实测 0.9992 / 1.0）
- 格式一致性：WAV vs FLAC 1.0000；MP3 vs WAV 1.0000；MP3 vs FLAC 1.0000 ✅
- 完整 8月4日.MP3：1514 音符，端到端 4.27s，实时系数 46.7x ✅
- 三预设：piano_balanced 912/912 与官方一致；piano_detail 1379（弱音优先）；piano_noise_reduced 536（少误音）✅

## 3. 验收标准（经用户确认，ADR-0008）

MAESTRO 绝对质量门槛（P≥0.93/R≥0.87/F1≥0.90）对 Basic Pitch 密集曲目物理不可达，
已调整为：**实现与官方逐音符等价（一致 F1≥0.99）**；产品验收以用户样本听审为准。
`gate` 硬门槛保留，供用户样本人工参考 MIDI 就绪后的严格验收。

## 4. 听审指引（最终发布确认）

1. 运行 `D:\tmp\scoreleap-eval\ScoreLeap_0.1.5_RC-setup.exe` 安装（或已安装到 `D:\tmp\scoreleap-install-rc`）
2. 首次转录在设置中下载模型（本机已具备模型文件与 ONNX Runtime；断网也可转录）
3. 导入 `D:\Download\8月4日.MP3`，选择预设（默认 piano_balanced）转录
4. 听审生成 MIDI：音高/节奏是否符合演奏；杂音（误音）是否可接受
5. 确认后回复即可发布正式版；如需调整预设参数或换更强模型（ByteDance 候选）再说明

## 5. 剩余风险 / 说明

- 本机无授权房间噪声样本，20dB 噪声测试仅含粉红噪声（合法授权资源后补）
- 人工参考 MIDI 尚未标注；标注后可对 8月4日.MP3 隐藏区跑严格 `gate`（0.93/0.87/0.90）
- 模型下载双源（CDN + GitHub Releases）在本机网络下 GitHub 不可达，已验证本地资产模式；
  正式发布环境的模型托管需确认可达
- Android 交付未开始（等 Windows 听审确认）
- 旧 Python Worker 未删除（等确认）
