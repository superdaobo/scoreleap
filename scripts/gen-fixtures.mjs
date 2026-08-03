#!/usr/bin/env node
/**
 * gen-fixtures.mjs — ScoreLeap 测试固件生成器
 *
 * 用纯 JavaScript（零第三方依赖）手写 SMF（Standard MIDI File）二进制，
 * 输出到 fixtures/midi/，并把每个文件的构造说明写入 fixtures/README.md。
 *
 * 用法：
 *   node scripts/gen-fixtures.mjs
 *
 * SMF 格式要点（本脚本实现）：
 *   头块  MThd <u32 length=6> <u16 format> <u16 ntrks> <u16 division>
 *   轨道  MTrk <u32 length> { <vlq delta> <event> }*
 *   事件  通道消息：<status byte> <data...>（running status 时省略状态字节）
 *         Meta 事件：FF <type> <vlq len> <data>
 *   变长量（VLQ）：7bit 分组，除最后一段外高位全部置 1
 *
 * 生成后可用任意 SMF 工具（或 scoreleap-midi 的 parse_midi）解析校验。
 */
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = join(ROOT, "fixtures", "midi");

// ---------------------------------------------------------------------------
// 基础编码
// ---------------------------------------------------------------------------

/** 变长量（VLQ）：7bit 分组，除最后一段外高位为 1 */
function vlq(value) {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`vlq 需要非负整数，收到: ${value}`);
  }
  const bytes = [value & 0x7f];
  while (value > 0x7f) {
    value = Math.floor(value / 128);
    bytes.unshift((value & 0x7f) | 0x80);
  }
  return bytes;
}

/** 大端 u16 */
function u16be(value) {
  return [(value >> 8) & 0xff, value & 0xff];
}

/** 大端 u32 */
function u32be(value) {
  return [
    (value >>> 24) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 8) & 0xff,
    value & 0xff,
  ];
}

/** ASCII 字符串 → 字节数组 */
function ascii(text) {
  return [...text].map((ch) => ch.charCodeAt(0));
}

// ---------------------------------------------------------------------------
// 事件构造
// ---------------------------------------------------------------------------

/** 通道 NoteOn（status = 0x90 | ch） */
function noteOn(channel, note, velocity) {
  return [0x90 | channel, note, velocity];
}

/** 通道 NoteOff（status = 0x80 | ch） */
function noteOff(channel, note, velocity = 0) {
  return [0x80 | channel, note, velocity];
}

/** Meta 事件：FF <type> <vlq len> <data> */
function meta(type, data) {
  return [0xff, type, ...vlq(data.length), ...data];
}

/** Tempo 元事件：3 字节，微秒/四分音符（120 BPM = 500000） */
const tempoMeta = (bpm) => meta(0x51, u32be(Math.round(60_000_000 / bpm)).slice(1));

/** 拍号元事件：nn dd(2 的幂) cc bb */
const timeSigMeta = (nn = 4, dd = 2, cc = 24, bb = 8) => meta(0x58, [nn, dd, cc, bb]);

/** 轨尾元事件 */
const endOfTrackMeta = () => meta(0x2f, []);

// ---------------------------------------------------------------------------
// 轨道构建器
// ---------------------------------------------------------------------------

/**
 * 轨道事件收集器。
 *
 * opts.running = true 时启用 running status：通道消息与上一条通道消息
 * 状态字节相同则省略状态字节；状态变化（如 NoteOn→NoteOff）或遇 Meta
 * 事件（本脚本约定 Meta 一律放在音符流之前，避免 running status 歧义）
 * 则写完整状态字节。
 */
class Track {
  constructor({ running = false } = {}) {
    this.running = running;
    this.events = []; // { delta, bytes, status }，status 为通道消息状态字节或 null
  }

  /** 追加原始事件；status 参与 running status 比较 */
  add(delta, bytes, status = null) {
    this.events.push({ delta, bytes, status });
    return this;
  }

  /** 追加 Meta 事件（不参与 running status） */
  metaEvent(delta, bytes) {
    return this.add(delta, bytes, null);
  }

  /** 追加 NoteOn（默认通道 0，力度 100） */
  noteOn(delta, note, velocity = 100, channel = 0) {
    return this.add(delta, noteOn(channel, note, velocity), 0x90 | channel);
  }

  /** 追加 NoteOff（默认通道 0，力度 0） */
  noteOff(delta, note, velocity = 0, channel = 0) {
    return this.add(delta, noteOff(channel, note, velocity), 0x80 | channel);
  }

  /** 编码为轨道字节流（delta 变长量 + 事件；按 running status 规则省略状态字节） */
  encode() {
    const out = [];
    let prevStatus = -1;
    for (const { delta, bytes, status } of this.events) {
      out.push(...vlq(delta));
      if (this.running && status !== null && status === prevStatus) {
        out.push(...bytes.slice(1)); // 省略状态字节（running status）
      } else {
        out.push(...bytes);
      }
      if (status !== null) {
        prevStatus = status;
      }
    }
    return out;
  }
}

// ---------------------------------------------------------------------------
// SMF 组装
// ---------------------------------------------------------------------------

/** 组装完整 SMF 文件字节 */
function buildSmf(format, tracks, division = 480) {
  const chunks = [];
  chunks.push(
    ...ascii("MThd"),
    ...u32be(6), // 头块长度恒为 6
    ...u16be(format),
    ...u16be(tracks.length),
    ...u16be(division),
  );
  for (const track of tracks) {
    const body = track.encode();
    chunks.push(...ascii("MTrk"), ...u32be(body.length), ...body);
  }
  return Buffer.from(chunks);
}

// ---------------------------------------------------------------------------
// 固件定义
// ---------------------------------------------------------------------------

const D = 480; // division：每四分音符的 tick 数

const fixtures = [];

// 1. single-track.mid —— 格式 0、120 BPM、C 大调音阶 8 个四分音符（MIDI 60–67）
{
  const track = new Track()
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta())
    .noteOn(0, 60)
    .noteOff(D, 60);
  for (let note = 61; note <= 67; note++) {
    track.noteOn(0, note).noteOff(D, note);
  }
  track.metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "single-track.mid",
    format: 0,
    division: D,
    purpose: "最基础的单轨音阶：验证格式 0 解析与默认 tempo（120 BPM）。",
    events: [
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "0      NoteOn  ch0 n=60 v=100",
      "480    NoteOff ch0 n=60",
      "0      NoteOn  ch0 n=61 v=100",
      "480    NoteOff ch0 n=61",
      "…      （依此类推 62–66）",
      "0      NoteOn  ch0 n=67 v=100",
      "480    NoteOff ch0 n=67",
      "0      Meta EndOfTrack",
    ],
    tracks: [track],
  });
}

// 2. multi-track.mid —— 格式 1、3 轨：
//    轨道 0：tempo 120→240（第 4 拍处）+ 拍号 4/4；
//    轨道 1：旋律 C4 E4 G4 C5 各一拍；
//    轨道 2：和弦块 C4+E4+G4 同时按下、持续两拍。
{
  const conductor = new Track()
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta())
    .metaEvent(3 * D, tempoMeta(240)) // 第 4 拍起点（3×480 ticks 处）加速到 240 BPM
    .metaEvent(0, endOfTrackMeta());
  const melody = new Track()
    .noteOn(0, 60)
    .noteOff(D, 60)
    .noteOn(0, 64)
    .noteOff(D, 64)
    .noteOn(0, 67)
    .noteOff(D, 67)
    .noteOn(0, 72)
    .noteOff(D, 72)
    .metaEvent(0, endOfTrackMeta());
  const chord = new Track()
    .noteOn(0, 60)
    .noteOn(0, 64)
    .noteOn(0, 67)
    .noteOff(2 * D, 60)
    .noteOff(0, 64)
    .noteOff(0, 67)
    .metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "multi-track.mid",
    format: 1,
    division: D,
    purpose:
      "格式 1 多轨：验证多轨道解析、tempo map 分段累加（120→240 BPM）、" +
      "拍号事件，以及和弦（同刻多音）与旋律并轨。",
    events: [
      "轨道 0（conductor）：",
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "1440   Meta Tempo 250000 µs/拍（240 BPM）——第 4 拍起点",
      "0      Meta EndOfTrack",
      "轨道 1（旋律）：",
      "0      NoteOn  ch0 n=60 v=100（C4）",
      "480    NoteOff ch0 n=60",
      "0      NoteOn  ch0 n=64 v=100（E4）",
      "480    NoteOff ch0 n=64",
      "0      NoteOn  ch0 n=67 v=100（G4）",
      "480    NoteOff ch0 n=67",
      "0      NoteOn  ch0 n=72 v=100（C5）",
      "480    NoteOff ch0 n=72",
      "0      Meta EndOfTrack",
      "轨道 2（和弦块）：",
      "0      NoteOn  ch0 n=60 v=100",
      "0      NoteOn  ch0 n=64 v=100",
      "0      NoteOn  ch0 n=67 v=100",
      "960    NoteOff ch0 n=60",
      "0      NoteOff ch0 n=64",
      "0      NoteOff ch0 n=67",
      "0      Meta EndOfTrack",
    ],
    tracks: [conductor, melody, chord],
  });
}

// 3. running-status.mid —— 连续 NoteOn 使用 running status（省略状态字节 0x90）
{
  const notes = [60, 64, 67, 72, 76]; // C4 E4 G4 C5 E5
  const track = new Track({ running: true })
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta());
  // 5 个 NoteOn 连续出现：第 2 个起省略 0x90（running status）
  notes.forEach((note, i) => track.noteOn(i === 0 ? 0 : 240, note));
  // 状态字节变为 0x80（NoteOff），恢复完整状态字节
  notes.forEach((note, i) => track.noteOff(i === 0 ? 240 : 0, note));
  track.metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "running-status.mid",
    format: 0,
    division: D,
    purpose:
      "验证 running status 解码：同一状态字节（0x90）连续出现时省略，解析器必须正确还原。",
    events: [
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "0      NoteOn  ch0 n=60 v=100（完整状态字节 0x90）",
      "240    NoteOn  ch0 n=64 v=100（省略 0x90，running status）",
      "240    NoteOn  ch0 n=67 v=100（省略 0x90）",
      "240    NoteOn  ch0 n=72 v=100（省略 0x90）",
      "240    NoteOn  ch0 n=76 v=100（省略 0x90）",
      "240    NoteOff ch0 n=60 v=0（状态变为 0x80，完整字节）",
      "0      NoteOff ch0 n=64 v=0（省略 0x80）",
      "0      NoteOff ch0 n=67 v=0（省略 0x80）",
      "0      NoteOff ch0 n=72 v=0（省略 0x80）",
      "0      NoteOff ch0 n=76 v=0（省略 0x80）",
      "0      Meta EndOfTrack",
    ],
    tracks: [track],
  });
}

// 4. velocity-zero.mid —— NoteOn velocity=0（等价 NoteOff）
{
  const track = new Track()
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta())
    .noteOn(0, 60, 100)
    .noteOn(D, 60, 0) // NoteOn v=0 ≡ NoteOff，不出现 0x80 事件
    .noteOn(0, 64, 100)
    .noteOn(D, 64, 0)
    .metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "velocity-zero.mid",
    format: 0,
    division: D,
    purpose:
      "验证 NoteOn velocity=0 被识别为 NoteOff：整个文件不包含任何 0x80 状态字节。",
    events: [
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "0      NoteOn  ch0 n=60 v=100",
      "480    NoteOn  ch0 n=60 v=0（等价 NoteOff）",
      "0      NoteOn  ch0 n=64 v=100",
      "480    NoteOn  ch0 n=64 v=0（等价 NoteOff）",
      "0      Meta EndOfTrack",
    ],
    tracks: [track],
  });
}

// 5. repeated-notes.mid —— 同一音高连续两次完整 NoteOn/NoteOff
{
  const track = new Track()
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta())
    .noteOn(0, 72, 100)
    .noteOff(D / 2, 72)
    .noteOn(0, 72, 100) // 同一音高（C5）第二次完整 NoteOn/NoteOff
    .noteOff(D / 2, 72)
    .noteOn(0, 60, 100) // 对照音（C4），验证两次 72 均独立产出事件
    .noteOff(D, 60)
    .metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "repeated-notes.mid",
    format: 0,
    division: D,
    purpose:
      "验证同一音高连续两次完整 NoteOn/NoteOff 时不会互相吞并（active-notes 状态机正确处理）。",
    events: [
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "0      NoteOn  ch0 n=72 v=100（C5 第 1 次）",
      "240    NoteOff ch0 n=72",
      "0      NoteOn  ch0 n=72 v=100（C5 第 2 次）",
      "240    NoteOff ch0 n=72",
      "0      NoteOn  ch0 n=60 v=100（C4 对照）",
      "480    NoteOff ch0 n=60",
      "0      Meta EndOfTrack",
    ],
    tracks: [track],
  });
}

// 6. out-of-range.mid —— 超出常用 36 键乐器音域的音符（MIDI 20 与 100）
{
  const track = new Track()
    .metaEvent(0, tempoMeta(120))
    .metaEvent(0, timeSigMeta())
    .noteOn(0, 20, 100) // 低于基准音域一个八度以上
    .noteOff(D, 20)
    .noteOn(0, 60, 100) // 基准音域内（对照）
    .noteOff(D, 60)
    .noteOn(0, 100, 100) // 高于基准音域一个八度以上
    .noteOff(D, 100)
    .metaEvent(0, endOfTrackMeta());
  fixtures.push({
    name: "out-of-range.mid",
    format: 0,
    division: D,
    purpose:
      "验证编排器的音域折叠/越界处理：MIDI 20 与 100 超出 36 键乐器（如 identity-v）音域一个八度以上。",
    events: [
      "0      Meta Tempo 500000 µs/拍（120 BPM）",
      "0      Meta TimeSignature 4/4",
      "0      NoteOn  ch0 n=20 v=100（越界低音）",
      "480    NoteOff ch0 n=20",
      "0      NoteOn  ch0 n=60 v=100（C4 对照）",
      "480    NoteOff ch0 n=60",
      "0      NoteOn  ch0 n=100 v=100（越界高音）",
      "480    NoteOff ch0 n=100",
      "0      Meta EndOfTrack",
    ],
    tracks: [track],
  });
}

// ---------------------------------------------------------------------------
// 输出
// ---------------------------------------------------------------------------

mkdirSync(OUT_DIR, { recursive: true });

const rows = [];
for (const fx of fixtures) {
  const bytes = buildSmf(fx.format, fx.tracks, fx.division);
  const path = join(OUT_DIR, fx.name);
  writeFileSync(path, bytes);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  rows.push({ ...fx, size: bytes.length, sha256 });
  console.log(`✓ ${fx.name}  ${bytes.length} bytes  sha256=${sha256.slice(0, 16)}…`);
}

// 生成 fixtures/README.md（构造说明，随脚本重新生成）
const table = rows
  .map(
    (r) =>
      `| \`${r.name}\` | ${r.format} | ${r.tracks.length} | ${r.division} | ${r.size} B | \`${r.sha256.slice(0, 16)}…\` |`,
  )
  .join("\n");

const sections = rows
  .map((r) => {
    const lines = r.events.map((line) => `    ${line}`).join("\n");
    return [
      `### ${r.name} — 格式 ${r.format} · ${r.tracks.length} 轨 · division ${r.division}`,
      "",
      r.purpose,
      "",
      "```text",
      lines,
      "```",
      "",
      `- 文件大小：${r.size} 字节`,
      `- SHA256：\`${r.sha256}\``,
    ].join("\n");
  })
  .join("\n\n");

const readme = `# fixtures — 测试固件

> 本目录由 \`node scripts/gen-fixtures.mjs\` 生成（纯 JS 手写 SMF 二进制，零第三方依赖）。
> 修改 fixture 时请改脚本后重新生成；**不要手改** \`midi/*.mid\` 与本 README。

## 生成方式

\`\`\`bash
node scripts/gen-fixtures.mjs
\`\`\`

脚本将 SMF 二进制写入 \`midi/\`，并（重新）生成此 README（含每个文件的构造说明）。
SMF 结构：头块 \`MThd\`（length=6：format u16 / ntrks u16 / division u16）+ 每轨 \`MTrk\`
（length + 事件流：delta 变长量 + 事件字节 + Meta 事件 \`FF 类型 长度 数据\`）。

## 文件清单

| 文件 | 格式 | 轨道数 | division | 大小 | SHA256（前 16 位） |
|---|---|---|---|---|---|
${table}

## 构造说明

${sections}

---
生成时间：${new Date().toISOString()}（由 gen-fixtures.mjs 自动生成）
`;

const readmePath = join(ROOT, "fixtures", "README.md");
writeFileSync(readmePath, readme);
console.log(`✓ fixtures/README.md  已生成（${readme.length} 字符）`);
console.log(`\n全部完成：${fixtures.length} 个文件 → ${OUT_DIR}`);
