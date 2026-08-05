# 原生转录资源目录（构建时原子填充）

源码树只保留此说明文件，使普通 `cargo test` 不需要下载第三方二进制。
Windows 发布流程会用 `tools/native-transcriber-packaging/Prepare-NativeTranscriber.ps1`
将整个目录原子替换为以下经过校验的文件：

- `scoreleap-transcriber-native.exe`
- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `LICENSE.onnxruntime.txt`
- `ThirdPartyNotices.onnxruntime.txt`
- `runtime-manifest.json`

Basic Pitch 模型不随安装包分发，而是由应用的模型管理器按需下载、验签和校验。
本目录不得出现 Python、虚拟环境、第三方 Python 包或 `.onnx` 模型。
