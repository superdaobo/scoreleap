# fixtures — 测试固件

> 本目录由 `node scripts/gen-fixtures.mjs` 生成（纯 JS 手写 SMF 二进制，零第三方依赖）。
> 修改 fixture 时请改脚本后重新生成；**不要手改** `midi/*.mid` 与本 README。

## 生成方式

```bash
node scripts/gen-fixtures.mjs
```

脚本将 SMF 二进制写入 `midi/`，并（重新）生成此 README（含每个文件的构造说明）。
SMF 结构：头块 `MThd`（length=6：format u16 / ntrks u16 / division u16）+ 每轨 `MTrk`
（length + 事件流：delta 变长量 + 事件字节 + Meta 事件 `FF 类型 长度 数据`）。

## 文件清单

| 文件 | 格式 | 轨道数 | division | 大小 | SHA256（前 16 位） |
|---|---|---|---|---|---|
| `single-track.mid` | 0 | 1 | 480 | 113 B | `fa3ee21662e39472…` |
| `multi-track.mid` | 1 | 3 | 480 | 134 B | `640d4621740dde38…` |
| `running-status.mid` | 0 | 1 | 480 | 78 B | `6f62adda8567ecc6…` |
| `velocity-zero.mid` | 0 | 1 | 480 | 59 B | `df02a24e4848294a…` |
| `repeated-notes.mid` | 0 | 1 | 480 | 68 B | `55348eabaa65fdb9…` |
| `out-of-range.mid` | 0 | 1 | 480 | 68 B | `47be93b108fdd051…` |

## 构造说明

### single-track.mid — 格式 0 · 1 轨 · division 480

最基础的单轨音阶：验证格式 0 解析与默认 tempo（120 BPM）。

```text
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    0      NoteOn  ch0 n=60 v=100
    480    NoteOff ch0 n=60
    0      NoteOn  ch0 n=61 v=100
    480    NoteOff ch0 n=61
    …      （依此类推 62–66）
    0      NoteOn  ch0 n=67 v=100
    480    NoteOff ch0 n=67
    0      Meta EndOfTrack
```

- 文件大小：113 字节
- SHA256：`fa3ee21662e39472891111444354d504a1dcbddd1f7cb29bbfae559e87414d2f`

### multi-track.mid — 格式 1 · 3 轨 · division 480

格式 1 多轨：验证多轨道解析、tempo map 分段累加（120→240 BPM）、拍号事件，以及和弦（同刻多音）与旋律并轨。

```text
    轨道 0（conductor）：
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    1440   Meta Tempo 250000 µs/拍（240 BPM）——第 4 拍起点
    0      Meta EndOfTrack
    轨道 1（旋律）：
    0      NoteOn  ch0 n=60 v=100（C4）
    480    NoteOff ch0 n=60
    0      NoteOn  ch0 n=64 v=100（E4）
    480    NoteOff ch0 n=64
    0      NoteOn  ch0 n=67 v=100（G4）
    480    NoteOff ch0 n=67
    0      NoteOn  ch0 n=72 v=100（C5）
    480    NoteOff ch0 n=72
    0      Meta EndOfTrack
    轨道 2（和弦块）：
    0      NoteOn  ch0 n=60 v=100
    0      NoteOn  ch0 n=64 v=100
    0      NoteOn  ch0 n=67 v=100
    960    NoteOff ch0 n=60
    0      NoteOff ch0 n=64
    0      NoteOff ch0 n=67
    0      Meta EndOfTrack
```

- 文件大小：134 字节
- SHA256：`640d4621740dde3839463e1d6d0ae1c103be92a1c4fb62b2df144f6a6129183b`

### running-status.mid — 格式 0 · 1 轨 · division 480

验证 running status 解码：同一状态字节（0x90）连续出现时省略，解析器必须正确还原。

```text
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    0      NoteOn  ch0 n=60 v=100（完整状态字节 0x90）
    240    NoteOn  ch0 n=64 v=100（省略 0x90，running status）
    240    NoteOn  ch0 n=67 v=100（省略 0x90）
    240    NoteOn  ch0 n=72 v=100（省略 0x90）
    240    NoteOn  ch0 n=76 v=100（省略 0x90）
    240    NoteOff ch0 n=60 v=0（状态变为 0x80，完整字节）
    0      NoteOff ch0 n=64 v=0（省略 0x80）
    0      NoteOff ch0 n=67 v=0（省略 0x80）
    0      NoteOff ch0 n=72 v=0（省略 0x80）
    0      NoteOff ch0 n=76 v=0（省略 0x80）
    0      Meta EndOfTrack
```

- 文件大小：78 字节
- SHA256：`6f62adda8567ecc6d7b289b3d0f48efe75aa39912a052ebb9e6c96f640613db0`

### velocity-zero.mid — 格式 0 · 1 轨 · division 480

验证 NoteOn velocity=0 被识别为 NoteOff：整个文件不包含任何 0x80 状态字节。

```text
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    0      NoteOn  ch0 n=60 v=100
    480    NoteOn  ch0 n=60 v=0（等价 NoteOff）
    0      NoteOn  ch0 n=64 v=100
    480    NoteOn  ch0 n=64 v=0（等价 NoteOff）
    0      Meta EndOfTrack
```

- 文件大小：59 字节
- SHA256：`df02a24e4848294aafee5bad7a17434a1f77a9d49c02898cf38737f66d2279fd`

### repeated-notes.mid — 格式 0 · 1 轨 · division 480

验证同一音高连续两次完整 NoteOn/NoteOff 时不会互相吞并（active-notes 状态机正确处理）。

```text
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    0      NoteOn  ch0 n=72 v=100（C5 第 1 次）
    240    NoteOff ch0 n=72
    0      NoteOn  ch0 n=72 v=100（C5 第 2 次）
    240    NoteOff ch0 n=72
    0      NoteOn  ch0 n=60 v=100（C4 对照）
    480    NoteOff ch0 n=60
    0      Meta EndOfTrack
```

- 文件大小：68 字节
- SHA256：`55348eabaa65fdb9aafbe0dc70acdf2a6d75827d6f5f002bc2b5f367610276c6`

### out-of-range.mid — 格式 0 · 1 轨 · division 480

验证编排器的音域折叠/越界处理：MIDI 20 与 100 超出 36 键乐器（如 identity-v）音域一个八度以上。

```text
    0      Meta Tempo 500000 µs/拍（120 BPM）
    0      Meta TimeSignature 4/4
    0      NoteOn  ch0 n=20 v=100（越界低音）
    480    NoteOff ch0 n=20
    0      NoteOn  ch0 n=60 v=100（C4 对照）
    480    NoteOff ch0 n=60
    0      NoteOn  ch0 n=100 v=100（越界高音）
    480    NoteOff ch0 n=100
    0      Meta EndOfTrack
```

- 文件大小：68 字节
- SHA256：`47be93b108fdd0511bad2bcd91aa767580ce4ee81fb30f067fefd2ac6f33659f`

---
生成时间：2026-08-03T15:51:13.038Z（由 gen-fixtures.mjs 自动生成）
