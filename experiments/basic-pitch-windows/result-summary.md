# Basic Pitch Windows Spike — 结果摘要

日期：2026-08-05 ｜ 分支：research/49-basic-pitch-spike ｜ 环境：Windows + Python 3.11.15

## 结论（TL;DR）

**Basic Pitch 在 Windows 本地转录与 PyInstaller 打包均可行，进入正式实现。**

- 模型随 pip 包分发（`basic_pitch/saved_models` 1.9MB）——打包无需外部下载；
- 25s MP3 → 174 音符 / 2 轨 / 24.8s 时长的 MIDI，**可被现有 `scoreleap-midi` 解析**（核心验证通过）；
- 打包决策：**PyInstaller onedir**（理由见 ADR-0007）。

## 环境与版本

| 项 | 值 |
|---|---|
| Python | 3.11.15（项目 venv：`tools/transcription-worker/.venv/`） |
| basic-pitch | 0.4.0 |
| 推理后端 | tensorflow 2.15.0（CPU；intel 构建） |
| 音频解码 | librosa 0.11.0 + audioread（MP3 经 ffmpeg 8.0.1 预转码测试） |
| MIDI 输出 | mido / pretty-midi 0.2.11 |

## 转录实测（25s 免版权测试音频）

| 指标 | 值 |
|---|---|
| 首次转录（含模型加载） | 31.1s |
| 第二次转录（模型已加载） | 10.5s |
| 峰值内存（tensorflow CPU） | ~522 MB |
| 输出 | `sample-25s_basic_pitch.mid`（1.70 KB） |
| 音符数 | 174（scoreleap-midi 解析确认） |
| 轨道数 / 时长 | 2 轨 / 24.8s |

## 关键发现

1. **GBK 编码崩溃**：中文 Windows 控制台默认 GBK，basic-pitch 完成时打印 emoji（\u2728）→ `UnicodeEncodeError`。**必须在 Worker 入口 `sys.stdout/stderr.reconfigure(encoding="utf-8", errors="replace")`**（Spike 中已修复并验证）。
2. **拒绝覆盖输出**：basic-pitch 对已存在的输出文件直接跳过（`already exists and would be overwritten`）。Rust 端为每个任务创建唯一任务目录可天然规避。
3. **无稳定分段进度**：模型 API 不暴露分段进度 → UI 采用阶段进度 + 不确定进度条（符合任务约束）。
4. **模型随包分发**：`basic_pitch/saved_models`（SavedModel，8 文件 1.9MB），PyInstaller `--collect-all basic_pitch` 即可收集。

## PyInstaller 对比

| 项 | onefile | onedir |
|---|---|---|
| 体积 | 390.5 MB（PKG 压缩） | 1204.4 MB（解压目录） |
| 启动+转录（25s 音频） | 41.7s（含临时解压） | 23.1s |
| 中文+空格路径 | —（onedir 已验证） | ✅ 通过（"中文 空格 目录/测试 音频.mp3"） |
| 崩溃诊断 | 差（临时解压目录） | 好（文件就地可查） |
| Tauri sidecar 集成 | 单文件简单，但每次启动解压慢 | 目录打包，启动快，诊断好 |

**决策：onedir**（详细理由见 `docs/adr/0007-windows-audio-transcription-worker.md`）。

## 许可证（主要依赖）

| 包 | 许可证 |
|---|---|
| basic-pitch 0.4.0 | MIT（GitHub 仓库声明；PyPI metadata 缺失，已按仓库声明记录） |
| tensorflow 2.15.0 | Apache-2.0 |
| librosa 0.11.0 | ISC |
| mido | MIT |
| pretty-midi | MIT |
| certifi | MPL-2.0 |
| 完整清单 | `tools/transcription-worker/requirements.lock.txt` 生成后随 Worker 提交 |

## 安装包体积预估

- Worker（onedir，含 tensorflow）：~1204 MB（解压）；NSIS 压缩后预计 400-500 MB；
- ScoreLeap 安装包将从 ~2.7 MB 增至 ~400+ MB（主要增量来自 tensorflow）。
- 若后续迁移到 ONNX Runtime（basic-pitch 支持 `[onnx]` 安装）可大幅缩减（预计 <200 MB），列为后续优化（见 ADR-0007）。

## 复现

```powershell
# 1. 建 venv 并安装
python -m venv tools/transcription-worker/.venv
tools/transcription-worker/.venv/Scripts/python -m pip install basic-pitch
# 2. 转录（注意：CLI 参数为 output_dir 在前）
$env:PYTHONIOENCODING = "utf-8"
tools/transcription-worker/.venv/Scripts/basic-pitch.exe <output_dir> <input.mp3>
# 3. PyInstaller
tools/transcription-worker/.venv/Scripts/pyinstaller.exe --onedir --name scoreleap-transcriber --collect-all basic_pitch <entry.py>
```

## 未验证 / 后续

- PyInstaller 冻结程序在**无 Python 机器**上的验证（本机已装 Python，无法完全模拟；打包机 CI 验证放 Issue 5）；
- onedir 目录中文路径已验证；安装目录含中文/空格（Tauri NSIS）待 Issue 5 打包后验证；
- tensorflow 巨大体积的替代路线（onnxruntime）留待后续 Issue。
