# 转录 Worker 资源目录（构建时填充）

本目录由构建流程填充 PyInstaller onedir 产物（`scoreleap-transcriber.exe` + `_internal/`，约 1.2GB）。
占位 README 使 CI 的 cargo 构建通过（tauri build script 校验 resources 存在）。

本地完整安装包构建：
```powershell
# 1. 构建 Worker（tools/transcription-worker，见 packaging/README.md）
pyinstaller --noconfirm packaging/scoreleap-transcriber.spec
# 2. 复制产物到本目录
Copy-Item -Recurse dist/scoreleap-transcriber/* apps/scoreleap/src-tauri/resources/scoreleap-transcriber/
# 3. 构建安装包
pnpm tauri build
```

注意：Windows Build workflow（tag 触发）若直接打包，本目录为占位内容；正式发布前需先注入 Worker。
