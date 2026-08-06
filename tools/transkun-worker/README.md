# ScoreLeap Transkun 高质量钢琴 Worker

该目录提供 **Transkun v2 CPU-only** 高质量钢琴转录路径。它不会要求最终用户安装 Python、PyTorch、CUDA、ffmpeg 或 Visual C++ Redistributable；这些运行文件会由 PyInstaller `onedir` 形式随 ScoreLeap 的 Tauri/NSIS 安装包一起分发。

## 架构

```text
ScoreLeap.exe（Tauri 主程序）
  ├─ 快速模式：scoreleap-transcriber-native.exe + Basic Pitch ONNX
  └─ 高质量钢琴：scoreleap-transkun-worker.exe + _internal/（Python/PyTorch/模型）
```

这里的“单 Tauri 程序”指用户只安装、启动一个 ScoreLeap 应用。高质量引擎是安装目录内部的受控 sidecar，不是需要用户另外安装的软件，也不会常驻系统。

## 为什么不直接嵌进 Rust 主进程

Transkun v2 使用 PyTorch Transformer、神经 Semi-CRF 与动态 Viterbi/分段合并逻辑。直接转换成 ONNX 会改变核心解码路径并引入较高质量回归风险。独立 sidecar 可以：

- 保持官方模型与解码语义；
- 崩溃或内存占用与 WebView 主进程隔离；
- 随安装包自包含；
- 后续独立升级、禁用或回滚。

## 音频依赖

Worker 使用 `miniaudio` 在进程内解码 MP3/WAV/FLAC，并直接输出 44.1kHz 单声道浮点采样，因此不分发或调用 ffmpeg。

## 构建

构建机要求：Windows x64、PowerShell、uv 管理的 Python 3.11，以及足够磁盘空间。最终用户不需要这些工具。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File tools/transkun-worker/Prepare-TranskunWorker.ps1
```

脚本会：

1. 创建隔离 Python 3.11 环境；
2. 安装固定版本 CPU-only PyTorch、Transkun 2.0.1 和运行依赖；
3. 使用 PyInstaller `onedir` 构建；
4. 补齐 app-local VC Runtime；
5. 拒绝 CUDA、ffmpeg、python.exe、pip.exe 等非目标文件；
6. 运行 Worker 自检；
7. 为所有资源生成 SHA-256 manifest；
8. 原子更新 `apps/scoreleap/src-tauri/resources/scoreleap-transkun/`。

复用已经准备的虚拟环境与 PyInstaller 输出：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File tools/transkun-worker/Prepare-TranskunWorker.ps1 `
  -SkipDependencyInstall -SkipBuild
```

## 审计

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File tools/transkun-worker/Test-TranskunBundle.ps1
```

仅检查仓库占位资源、允许尚未生成大体积运行时：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File tools/transkun-worker/Test-TranskunBundle.ps1 -AllowPlaceholder
```

## 协议

Worker 与现有原生转录器共用 JSON Lines 协议版本 1：

- `ready`
- `stage`
- `result`
- `error`

高质量 sidecar 是自包含 Worker，因此不会接收 `--model` 或 `--onnx-runtime`。Rust 服务根据 `engine=fast|high_quality` 选择正确的 Worker。

## 数据与隐私

音频、模型推理、MIDI 和元数据均在本机处理。Worker 不包含网络代码，也不会上传音频。
