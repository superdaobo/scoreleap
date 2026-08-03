//! SMF（.mid/.midi）解析 → Music IR。
//!
//! 处理：格式 0/1/2、running status、Note On velocity=0（视为 NoteOff）、
//! SMPTE division、tempo 变化分段累加为绝对微秒时间。
//! 两遍扫描：先收集全局 tempo 断点表（tempo 事件可能在任意轨道），
//! 再按断点表把 ticks 换算为绝对微秒（i128 防御溢出）。

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use scoreleap_music_ir::{
    MidiFormat, MusicDocument, NoteEvent, TempoEvent, TimeSignatureEvent, Track,
};
use std::collections::HashMap;

/// 解析错误。
#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    #[error("不是合法的 MIDI 文件: {0}")]
    Invalid(String),
    #[error("文件为空")]
    Empty,
    #[error("文件过大（上限 50MB）")]
    TooLarge,
    #[error("轨道数过多（上限 256）")]
    TooManyTracks,
}

const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;
const MAX_TRACKS: usize = 256;
const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM

/// 解析 MIDI 字节流为 Music IR。
pub fn parse_midi(bytes: &[u8]) -> Result<MusicDocument, MidiError> {
    if bytes.is_empty() {
        return Err(MidiError::Empty);
    }
    if bytes.len() > MAX_FILE_SIZE {
        return Err(MidiError::TooLarge);
    }
    let smf = Smf::parse(bytes).map_err(|e| MidiError::Invalid(e.to_string()))?;
    if smf.tracks.len() > MAX_TRACKS {
        return Err(MidiError::TooManyTracks);
    }

    // 时间基准：Metrical(div) → ticks/拍；Timecode(fps, subframe) → 每秒 tick 数
    let metrical_div: Option<i64> = match smf.header.timing {
        Timing::Metrical(div) => Some(div.as_int().max(1) as i64),
        Timing::Timecode(_, _) => None,
    };
    let smpte_ticks_per_sec: Option<i64> = match smf.header.timing {
        Timing::Metrical(_) => None,
        Timing::Timecode(fps, subframe) => {
            let fps_num = match fps {
                midly::Fps::Fps24 => 24,
                midly::Fps::Fps25 => 25,
                midly::Fps::Fps29 => 30, // 29.97 按 30 近似
                midly::Fps::Fps30 => 30,
            };
            Some(fps_num as i64 * (subframe.max(1)) as i64)
        }
    };

    let format = match smf.header.format {
        Format::SingleTrack => MidiFormat::SingleTrack,
        Format::Parallel => MidiFormat::Parallel,
        Format::Sequential => MidiFormat::Sequential,
    };

    let tempo_breaks: Vec<(i64, u32)> = {
        let mut seen_ticks: HashMap<i64, u32> = HashMap::new();
        for track in &smf.tracks {
            let mut tick: i64 = 0;
            for event in track {
                tick += event.delta.as_int() as i64;
                if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = event.kind {
                    seen_ticks.entry(tick).or_insert(t.as_int().max(1));
                }
            }
        }
        let mut v: Vec<(i64, u32)> = seen_ticks.into_iter().collect();
        v.sort_by_key(|(t, _)| *t);
        v
    };

    // tick → 绝对微秒（分段线性，按断点表）。
    let tick_to_us = |tick: i64| -> i64 {
        if let Some(div) = metrical_div {
            let mut us = 0i64;
            let mut prev_tick = 0i64;
            let mut tempo = DEFAULT_TEMPO_US as i128;
            for (bt, bt_us) in &tempo_breaks {
                if tick <= *bt {
                    break;
                }
                us += ((*bt - prev_tick) as i128 * tempo / div as i128) as i64;
                prev_tick = *bt;
                tempo = *bt_us as i128;
            }
            us + ((tick - prev_tick) as i128 * tempo / div as i128) as i64
        } else if let Some(tps) = smpte_ticks_per_sec {
            tick * 1_000_000 / tps
        } else {
            0
        }
    };

    let mut tracks = Vec::with_capacity(smf.tracks.len());
    let mut all_tempo: Vec<TempoEvent> = Vec::new();
    let mut all_ts: Vec<TimeSignatureEvent> = Vec::new();
    let mut doc_end_us: i64 = 0;

    for (track_index, track_events) in smf.tracks.iter().enumerate() {
        let track_id = track_index as u16;
        let mut tick: i64 = 0;
        let mut track_name = String::new();
        let mut instrument: Option<String> = None;
        let mut active_notes: HashMap<u8, (i64, u8)> = HashMap::new();
        let mut notes: Vec<NoteEvent> = Vec::new();

        for event in track_events {
            tick += event.delta.as_int() as i64;
            let abs_us = tick_to_us(tick);

            match event.kind {
                TrackEventKind::Midi { message, .. } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        let k = key.as_int();
                        let v = vel.as_int();
                        if v == 0 {
                            // velocity=0 语义：NoteOff
                            close_note(&mut active_notes, &mut notes, k, abs_us, track_id);
                        } else {
                            // 同一音高重复 NoteOn（无 NoteOff）：关闭旧音，保留重复音
                            if let Some((start, vel0)) = active_notes.remove(&k) {
                                notes.push(NoteEvent {
                                    track_id,
                                    note: k,
                                    velocity: vel0,
                                    start_us: start,
                                    duration_us: (abs_us - start).max(1),
                                });
                            }
                            active_notes.insert(k, (abs_us, v));
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        close_note(
                            &mut active_notes,
                            &mut notes,
                            key.as_int(),
                            abs_us,
                            track_id,
                        );
                    }
                    _ => {}
                },
                TrackEventKind::Meta(m) => match m {
                    MetaMessage::TrackName(name) => {
                        if track_name.is_empty() {
                            track_name = String::from_utf8_lossy(name).to_string();
                        }
                    }
                    MetaMessage::InstrumentName(name) => {
                        instrument = Some(String::from_utf8_lossy(name).to_string());
                    }
                    MetaMessage::Tempo(t) => {
                        all_tempo.push(TempoEvent {
                            time_us: abs_us,
                            tempo_us_per_quarter: t.as_int().max(1),
                        });
                    }
                    MetaMessage::TimeSignature(num, den, _, _) => {
                        all_ts.push(TimeSignatureEvent {
                            time_us: abs_us,
                            numerator: num,
                            denominator: 1u8 << den,
                        });
                    }
                    _ => {}
                },
                TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => {}
            }
        }

        // 收尾：轨道末尾未关闭的音符
        let end_us = tick_to_us(tick);
        for (note, (start, vel)) in active_notes.drain() {
            notes.push(NoteEvent {
                track_id,
                note,
                velocity: vel,
                start_us: start,
                duration_us: (end_us - start).max(1),
            });
        }
        notes.sort_by_key(|n| (n.start_us, n.note));
        doc_end_us = doc_end_us.max(end_us);

        tracks.push(Track {
            id: track_id,
            name: if track_name.is_empty() {
                format!("轨道 {}", track_id + 1)
            } else {
                track_name
            },
            notes,
            instrument,
        });
    }

    all_tempo.sort_by_key(|t| t.time_us);
    all_ts.sort_by_key(|t| t.time_us);

    Ok(MusicDocument {
        format,
        tracks,
        tempo_events: all_tempo,
        time_signature_events: all_ts,
        duration_us: doc_end_us,
    })
}

fn close_note(
    active: &mut HashMap<u8, (i64, u8)>,
    notes: &mut Vec<NoteEvent>,
    note: u8,
    end_us: i64,
    track_id: u16,
) {
    if let Some((start, vel)) = active.remove(&note) {
        notes.push(NoteEvent {
            track_id,
            note,
            velocity: vel,
            start_us: start,
            duration_us: (end_us - start).max(1),
        });
    }
}

/// 解析 MIDI 文件路径。
pub fn parse_file(path: &std::path::Path) -> Result<MusicDocument, MidiError> {
    let bytes = std::fs::read(path).map_err(|e| MidiError::Invalid(e.to_string()))?;
    parse_midi(&bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;
    use midly::num::{u15, u24, u28, u7};
    use midly::{Header, TrackEvent};

    /// 构造 SMF 字节流（format 1，两条轨道：控制轨 + 音符轨）。
    fn build_smf(tempo_change: bool) -> Vec<u8> {
        let mut t0: Vec<TrackEvent> = vec![];
        if tempo_change {
            t0.push(TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            });
            t0.push(TrackEvent {
                delta: u28::new(480 * 2),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(250_000))),
            });
        } else {
            t0.push(TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            });
        }
        t0.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
        });

        let mut t1: Vec<TrackEvent> = vec![];
        t1.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOn {
                    key: u7::new(60),
                    vel: u7::new(100),
                },
            },
        });
        t1.push(TrackEvent {
            delta: u28::new(480),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOn {
                    key: u7::new(64),
                    vel: u7::new(90),
                },
            },
        });
        t1.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOff {
                    key: u7::new(60),
                    vel: u7::new(0),
                },
            },
        });
        // running status 风格：NoteOn(67, vel=0) 即 NoteOff
        t1.push(TrackEvent {
            delta: u28::new(480),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOn {
                    key: u7::new(67),
                    vel: u7::new(80),
                },
            },
        });
        t1.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOn {
                    key: u7::new(67),
                    vel: u7::new(0),
                },
            },
        });
        t1.push(TrackEvent {
            delta: u28::new(480),
            kind: TrackEventKind::Midi {
                channel: midly::num::u4::new(0),
                message: MidiMessage::NoteOff {
                    key: u7::new(64),
                    vel: u7::new(0),
                },
            },
        });

        let smf = Smf {
            header: Header {
                format: Format::Parallel,
                timing: Timing::Metrical(u15::new(480)),
            },
            tracks: vec![t0, t1],
        };
        let mut out = Vec::new();
        smf.write(&mut out).unwrap();
        out
    }

    #[test]
    fn parses_basic_smf_with_tempo() {
        let bytes = build_smf(true);
        let doc = parse_midi(&bytes).unwrap();
        assert_eq!(doc.format, MidiFormat::Parallel);
        assert_eq!(doc.tracks.len(), 2);
        // tempo 事件：0 与 960 tick（2 拍 @120BPM）= 1_000_000us
        assert_eq!(doc.tempo_events.len(), 2);
        assert_eq!(doc.tempo_events[0].time_us, 0);
        assert_eq!(doc.tempo_events[0].tempo_us_per_quarter, 500_000);
        assert_eq!(doc.tempo_events[1].time_us, 1_000_000);
        assert_eq!(doc.tempo_events[1].tempo_us_per_quarter, 250_000);
        assert_eq!(doc.time_signature_events[0].numerator, 4);
        assert_eq!(doc.time_signature_events[0].denominator, 4);
    }

    #[test]
    fn note_times_and_velocity_zero_semantics() {
        let bytes = build_smf(false);
        let doc = parse_midi(&bytes).unwrap();
        let track1 = &doc.tracks[1];
        assert_eq!(track1.notes.len(), 3);
        let c4 = track1.notes.iter().find(|n| n.note == 60).unwrap();
        assert_eq!(c4.start_us, 0);
        assert_eq!(c4.duration_us, 500_000);
        let e4 = track1.notes.iter().find(|n| n.note == 64).unwrap();
        assert_eq!(e4.start_us, 500_000);
        assert_eq!(e4.duration_us, 1_000_000);
        let g4 = track1.notes.iter().find(|n| n.note == 67).unwrap();
        assert_eq!(g4.start_us, 1_000_000);
        // 全部音符 track_id 正确
        assert!(track1.notes.iter().all(|n| n.track_id == 1));
    }

    #[test]
    fn tempo_change_affects_note_timing() {
        // tempo 变化在 tick 960（1_000_000us，120BPM→240BPM）
        // E4 起始 480 tick @120BPM = 500_000us
        // E4 关闭 1440 tick：960 之后 480 tick @240BPM = 250_000us → 1_250_000us
        let bytes = build_smf(true);
        let doc = parse_midi(&bytes).unwrap();
        let e4 = doc.tracks[1].notes.iter().find(|n| n.note == 64).unwrap();
        assert_eq!(e4.start_us, 500_000);
        assert_eq!(e4.duration_us, 750_000);
    }

    #[test]
    fn empty_bytes_rejected() {
        assert!(matches!(parse_midi(&[]), Err(MidiError::Empty)));
    }

    #[test]
    fn garbage_rejected() {
        let err = parse_midi(b"not a midi file at all").unwrap_err();
        assert!(matches!(err, MidiError::Invalid(_)));
    }

    #[test]
    fn repeated_note_not_lost() {
        // 同一音高两次完整 NoteOn/NoteOff → 两个音符
        let mut t: Vec<TrackEvent> = vec![];
        for (d, v) in [(0u32, 100u8), (480, 0), (480, 90), (480, 0)] {
            t.push(TrackEvent {
                delta: u28::new(d),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::new(0),
                    message: if v == 0 {
                        MidiMessage::NoteOff {
                            key: u7::new(60),
                            vel: u7::new(0),
                        }
                    } else {
                        MidiMessage::NoteOn {
                            key: u7::new(60),
                            vel: u7::new(v),
                        }
                    },
                },
            });
        }
        let smf = Smf {
            header: Header {
                format: Format::SingleTrack,
                timing: Timing::Metrical(u15::new(480)),
            },
            tracks: vec![t],
        };
        let mut out = Vec::new();
        smf.write(&mut out).unwrap();
        let doc = parse_midi(&out).unwrap();
        assert_eq!(doc.tracks[0].notes.len(), 2);
    }
}
