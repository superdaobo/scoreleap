# ADR-0003：Music IR 与时间表示

- 状态：已接受（draft）
- 日期：2026-02（规划期）
- 决策者：Agent A / Agent B

## 背景

多种输入格式（MIDI/MusicXML/音频转录/编辑器）需要统一中间表示；编排器与调度器需要精确、无歧义的时间语义。MIDI 内部使用 ticks，而调度需要绝对时间。

## 决策

1. **统一模型 `MusicDocument`**：`tracks: Vec<Track>` + `tempo_events` + `time_signature_events`；`Track` 含 `notes: Vec<NoteEvent>`；`NoteEvent { track_id, note, velocity, start_us, duration_us }`。
2. **时间统一为整数微秒（i64）**：解析阶段完成 ticks → 微秒换算：Metrical 模式按 `abs_us += delta_ticks × tempo_us / division` 分段累加（遇到 Tempo 事件更新 `tempo_us`，初始 500000µs = 120 BPM）；SMPTE 模式按 `us_per_tick = 1_000_000 / (fps × subframe)`。中间乘法用 u128/checked_mul 防御（tick ≤ 2²⁸−1，i64 足够但需防御）。核心调度禁止浮点秒。
3. **Tempo 表示为 `us_per_quarter`（u32）**：避免浮点 BPM 的精度与表示问题；UI 层再转为 BPM 显示。
4. **解析职责**：`scoreleap-midi` 基于 midly 0.5.3（Unlicense）实现 → Music IR，自行处理：running status、**Note On velocity=0 归一化为 NoteOff（midly 保留原样，必须由我方转换）**、delta 相对 ticks 累加为绝对时间、tempo map 构建、SMPTE division（frames-per-second 模式转换）。
5. **绝对时间缓存**：解析完成后事件即有序绝对时间，编排与调度不再处理 ticks。
6. **duration_us 语义**：NoteOff 时刻 - NoteOn 时刻；跨 tempo 变化的音符按实际事件时刻计算，不做近似。

## 后果

- 正面：所有下游（编排/调度/预览）使用同一时间语义；整数微秒可测、可序列化、无浮点误差；黄金样例可直接断言微秒值。
- 负面：超大曲目（百万事件）内存占用略高于 ticks 表示（可接受，v0.1 上限 50MB 文件）。

## 替代方案

| 方案 | 评估 |
|---|---|
| 保持 ticks + TempoMap 直到调度期 | 每个下游都要重复换算，易出错；拒绝 |
| 浮点秒（f64） | 调度累积误差与序列化歧义；拒绝 |
| 纳秒（i64） | 微秒精度对 10ms 目标绰绰有余，纳秒徒增宽度；接受微秒 |

## 关联

- ARCHITECTURE.md §3/§10/附录 A；ADR-0005（调度精度）。
