# ADR-0006：AI 转录策略（音频转 MIDI）

- 状态：已接受（draft，v0.4 前置）
- 日期：2026-02（规划期）
- 决策者：Agent A / Agent B

## 背景

用户音频（MP3 等）需要转为可演奏的 MIDI。候选方案：Basic Pitch（Spotify）、Piano Transcription（字节）、Demucs 音源分离 + 转录、在线 API。v0.1/v0.2 明确不引入 AI；v0.4 在 Windows 落地，v0.5 评估 Android。

## 决策

1. **v0.4 主路径 = Basic Pitch ONNX + ONNX Runtime（CPU）+ Symphonia 解码**：本地推理；分段处理控制内存；后处理（onset 合并、最小音长过滤、置信度阈值）在 Rust 实现。
2. **模型分发**：模型不进 Git 仓库与安装包；通过 GitHub Release 资产或应用内下载，`models/manifest.json` 记录名称/大小/SHA256/许可证/来源 URL；下载后校验，校验失败不加载。
3. **引入门槛**：任何模型须先完成「许可证确认 + 体积评估 + 性能/质量测试」（对应技术验证 Issue），通过后才能进入 v0.4 实现。
4. **Piano Transcription / Demucs**：仅研究（可行性 + 许可证 + 体积），不承诺实现；Demucs 若引入需评估 MPL/MIT 许可与 CPU 性能。
5. **v0.5（Android）**：先研究 ONNX Runtime Mobile、模型裁剪、内存/温度/功耗、低端机降级；在线转换仅作为「用户显式选择」的可选备选，且必须说明传输内容。
6. **不引入**：云端强制转录、黑盒 API 依赖（无法本地化的在线服务不作为主路径）。

## 后果

- 正面：隐私友好（本地推理）；v0.1/v0.2 零 AI 复杂度；模型质量与性能有数据支撑后再承诺。
- 负面：移动端 AI 延迟到 v0.5 且可能仅部分能力落地；模型体积需评估（Basic Pitch 的 nmp.onnx 约 225KB 很小，Demucs 等候选模型较大），下载管理 UX 需设计。

## 替代方案

| 方案 | 评估 |
|---|---|
| 在线转录 API 为主 | 违反本地优先；仅作显式备选 |
| 直接集成 Basic Pitch Python 版本 | 需捆绑 Python 运行时，体积爆炸；拒绝 |
| Demucs + Basic Pitch 全流程 v0.4 | 复杂度高，先研究后决定 |
| 无 AI，仅 MIDI/MusicXML | v0.1–v0.3 如此；音频需求留到 v0.4 验证 |

## 关联

- PRODUCT_PLAN.md §15/§22；ROADMAP.md v0.4/v0.5；RISK_AND_COMPLIANCE.md §7。
