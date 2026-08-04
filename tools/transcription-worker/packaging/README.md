# Worker 打包（PyInstaller onedir）

依据 ADR-0007：**onedir**（启动快、可诊断、杀软误报低）。

## 构建

```powershell
# 在项目 venv 内
python -m pip install pyinstaller
pyinstaller --noconfirm packaging/scoreleap-transcriber.spec
# 产物：dist/scoreleap-transcriber/（含 scoreleap-transcriber.exe）
```

## 关键点

- `--collect-all basic_pitch` 效果由 spec 的 hiddenimports + Analysis 收集包数据（saved_models 随包分发）保证；
- 入口 `scoreleap_transcriber/__main__.py`（内部 reconfigure UTF-8，修复中文 Windows GBK 崩溃）；
- 产物目录整体作为 Tauri `externalBin` 侧车打包（Issue #46/#48）；
- 真实构建与体积报告见 Issue #48。
