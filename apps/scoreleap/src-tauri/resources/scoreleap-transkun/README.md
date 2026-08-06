# ScoreLeap Transkun 运行时资源

该目录由 `tools/transkun-worker/Prepare-TranskunWorker.ps1` 原子生成。

发布版应至少包含：

- `scoreleap-transkun-worker.exe`
- PyInstaller `_internal/` 运行时目录
- `runtime-manifest.json`
- `licenses/`

仓库不提交数百 MB 的 PyTorch/模型二进制。未执行打包脚本时，应用会将“高质量钢琴”标记为不可用，并继续保留 Basic Pitch 快速模式。
