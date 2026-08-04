# Basic Pitch Windows Spike

验证 Basic Pitch 在 Windows 本地转录与 PyInstaller 打包可行性（Issue #49）。

## 目录

- `run.ps1` — 转录脚本（使用项目 venv）
- `spike_entry.py` — PyInstaller 打包入口（含 UTF-8 reconfigure，修复中文 Windows GBK 崩溃）
- `result-summary.md` — 完整结果摘要（耗时/内存/体积/许可证/决策）

## 快速复现

见 `result-summary.md` 的「复现」一节。测试音频放在 `out/`（已 gitignore，不提交）。
