# Windows 原生转录打包

此目录负责把 Rust 原生转录 sidecar 与微软官方 ONNX Runtime x64 CPU 运行库注入
Tauri 安装包。构建链不安装全局包、不调用 Python，也不把 Basic Pitch 模型内置到安装包。

## 本地构建

在 Windows x64、Rust stable、Node.js 22、pnpm 10.33.4 环境中，从仓库根目录执行：

```powershell
pnpm install --frozen-lockfile
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/run-cargo.ps1 test -p scoreleap-transcribe -p scoreleap-transcriber-native --locked
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/run-cargo.ps1 build --locked --release -p scoreleap-transcriber-native
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/native-transcriber-packaging/Prepare-NativeTranscriber.ps1 -SkipNativeBuild
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/native-transcriber-packaging/Test-NativeTranscriberBundle.ps1 -ResourceDirectory apps/scoreleap/src-tauri/resources/scoreleap-transcriber
pnpm tauri build --bundles nsis
```

准备脚本会缓存 ZIP，但每次命中缓存仍同时验证固定大小和 SHA-256。下载先写唯一临时文件，
校验通过后才原子移动到缓存；资源目录也通过同盘 staging/rename 更新，失败时恢复原目录。
在解压前还会拒绝绝对路径、`..` 穿越、固定根目录外条目、大小写重复项、符号链接和异常膨胀；
如果极端情况下原目录恢复失败，脚本会保留唯一的 `.backup-*` 副本并停止构建，不会在清理阶段删除它。

## 固定的第三方资产与许可证

- 项目：Microsoft ONNX Runtime 1.24.4，Windows x64 CPU。
- 官方发布页：`https://github.com/microsoft/onnxruntime/releases/tag/v1.24.4`。
- 固定资产：`onnxruntime-win-x64-1.24.4.zip`，asset id `376015528`，大小
  `74442783` bytes。
- 固定 URL：`https://github.com/microsoft/onnxruntime/releases/download/v1.24.4/onnxruntime-win-x64-1.24.4.zip`。
- SHA-256：`d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357`。
- 摘要来源：微软仓库的 GitHub Releases API `digest` 字段；API 资产地址为
  `https://api.github.com/repos/microsoft/onnxruntime/releases/assets/376015528`。
- 许可证：MIT。准备脚本从已校验的官方 ZIP 复制 `LICENSE` 和
  `ThirdPartyNotices.txt` 到最终资源目录。

最终转录资源目录严格包含原生 sidecar、`onnxruntime.dll`、
`onnxruntime_providers_shared.dll`、许可证/第三方声明和 `runtime-manifest.json`。
模型由应用模型管理器按需下载；安装包审计会拒绝 `.onnx`、Python DLL、venv、librosa、
numba、tensorflow 和其他未批准文件。

## E2E 与真实性能报告

基础 E2E 会生成一个短 WAV，仅验证 JSONL schema v1、缺模型失败和失败不留 MIDI；不依赖
Python。要运行真实转录，设置下列本地路径后执行：

```powershell
$env:SCORELEAP_E2E_MODEL = "D:\models\basic-pitch.onnx"
$env:SCORELEAP_E2E_AUDIO = "D:\audio\piano.mp3"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/native-transcriber-packaging/Invoke-NativeTranscriberE2E.ps1 -SidecarPath apps/scoreleap/src-tauri/resources/scoreleap-transcriber/scoreleap-transcriber-native.exe -RuntimeDirectory apps/scoreleap/src-tauri/resources/scoreleap-transcriber -RequireRealAssets
```

脚本验证 MIDI `MThd`、metadata 顶层 `duration_seconds` 和正音符数，并写出墙钟耗时、音频时长与 RTF。它只记录音频
文件名和 SHA-256，不把绝对音频路径写入报告。CI 未配置真实资产时会明确跳过真实转录，而不把
协议冒烟误报为质量验收。

## 故障诊断

- `SHA-256 校验失败`：缓存或下载内容损坏；不要跳过校验。换用干净的缓存目录重试。
- `官方资产缺少必要文件`：上游资产结构与固定版本不一致；停止发布并人工核对 asset id。
- `MODEL_LOAD_FAILED`：模型不存在、损坏或与 Basic Pitch 输出契约不兼容；模型不属于安装包资源。
- `ONNX_RUNTIME_LOAD_FAILED`：确认两个 ORT DLL 与 sidecar 在同一目录，且运行环境为 Windows x64。
- clean-install 找不到 sidecar：先检查 `tauri.conf.json` 的资源目录，再运行资源审计。

## 正式签名发布前提

当前工作流只生成并上传未发布的 CI artifact，不创建 GitHub Release。公开发布前仍必须完成：

1. 配置受保护的 Windows 代码签名证书与时间戳服务，对应用 EXE、sidecar 和 NSIS 安装包签名；
2. 配置正式模型清单的 Ed25519 私钥保管、双源 HTTPS 地址和轮换/吊销流程；
3. 在干净 Windows 4 核/8GB 设备上通过真实音频质量门槛与 p95 RTF 门槛；
4. 对最终安装包执行 clean-install、内容审计、恶意软件扫描并核对发布 SHA-256。

以上前提未满足时，不应把 CI 产物标记为正式发布版本。
