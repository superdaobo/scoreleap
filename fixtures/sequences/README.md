# fixtures/sequences — 期望输出（CompiledSequence JSON）存放约定

> 本目录存放「期望输出」：Rust 集成测试读取 `fixtures/midi/*.mid` 解析并编排后，
> 与对应的期望 JSON 逐字段比对。期望 JSON 属于测试资产，随 PR 审查冻结/更新。

## 1. 存放约定

- 每个 `fixtures/midi/<name>.mid` 对应一个期望文件：`fixtures/sequences/<name>.json`。
- 文件内容为 `scoreleap-sequence::CompiledSequence` 的 serde 序列化结果
  （`serde_json::to_value`，字段顺序无关）：

```json
{
  "actions": [
    { "KeyDown": { "at_us": 0, "key": 40 } },
    { "KeyUp": { "at_us": 500000, "key": 40 } }
  ],
  "duration_us": 2000000,
  "meta": {
    "source_name": "single-track.mid",
    "track_ids": [0],
    "note_count": 8,
    "transpose_semitones": 0
  }
}
```

- `actions`：`PlatformAction` 枚举（`KeyDown` / `KeyUp` / `Gesture`），按 `at_us` 升序、
  同刻顺序稳定；`key` 为 `KeyCode`（扫描码，见 `scoreleap-music-ir`）。
- `duration_us`：时间轴总长（整数微秒）。
- `meta`：`SequenceMeta`（源文件名、启用的轨道、音符数、实际移调量）。
- 期望 JSON 中不保存编排中间态（`ArrangeStats` 等），只冻结最终 `CompiledSequence`。

## 2. 比对方式（Rust 集成测试）

对每个 `fixtures/midi/<name>.mid`：

1. 读取文件字节 → `scoreleap_midi::parse_midi(&bytes)` → `MusicDocument`；
2. 固定编排参数：`ArrangementOptions::default()`，启用全部轨道
   （`track_ids = 0..document.tracks.len()`），Profile 固定使用
   `game-profiles/identity-v/profile.json`（36 键乐器，保证跨机器可复现）；
3. `scoreleap_arranger::arrange(&doc, &options, &profile, &tracks)` →
   `(CompiledSequence, ArrangeStats)`；
4. `serde_json::to_value(&sequence)` 与期望文件反序列化后的
   `serde_json::Value` 做**深比较**（字段顺序无关、浮点/整数类型一致）；
5. 任一字段不一致 → 测试失败，并输出差异（左：期望，右：实际）。

比对是**字节级等价**而非语义近似：任何 `at_us`、`key`、`note_count` 的变化都会
暴露，防止编排行为悄然漂移。

## 3. 期望文件的生命周期

- **首次生成（冻结）**：实现测试阶段以 `SCORELEAP_UPDATE_EXPECTED=1` 环境变量
  运行 → 测试将实际 `CompiledSequence` 序列化写入 `fixtures/sequences/<name>.json`；
  随后由**人工复核**（检查音符、按键、时间点是否符合 fixture 构造说明的语义）后
  提交冻结。复核要点：`duration_us` 与 tempo map 一致、`at_us` 为整数微秒、
  移调/折叠结果符合 `identity-v` 音域、`KeyDown/KeyUp` 成对。
- **更新**：任何改变解析或编排输出的改动（MIDI 解析、tempo map、编排管线、
  game-profile）必须同步更新受影响期望文件，并在同一 PR 中审查差异；
  禁止绕过比对（如删除期望文件或放宽比较）。
- **拒绝**：出现与 fixture 构造说明矛盾的结果（例如 running-status 文件解析出
  错误音符数）时，先修实现，不迁就期望。

## 4. 当前状态

- 本阶段仅建立目录与约定（本 README）；`fixtures/midi/` 固件由
  `node scripts/gen-fixtures.mjs` 生成。
- 期望 JSON 尚不存在，待集成测试实现后按第 3 节流程生成并冻结。
