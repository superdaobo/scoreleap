use std::fs::File;
use std::io::Write;
use std::path::Path;

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::{NoteEvent, TranscribeError};

const TICKS_PER_QUARTER: u16 = 480;
const TEMPO_US_PER_QUARTER: u32 = 500_000;
const TICKS_PER_SECOND: f64 = TICKS_PER_QUARTER as f64 * 1_000_000.0 / TEMPO_US_PER_QUARTER as f64;

#[derive(Debug, Clone, Copy)]
struct TimedMidiEvent {
    tick: u32,
    is_on: bool,
    pitch: u8,
    velocity: u8,
}

/// 写入并重新解析 MIDI，避免向上层返回损坏或不完整的文件。
pub fn write_midi(path: impl AsRef<Path>, notes: &[NoteEvent]) -> Result<(), TranscribeError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| TranscribeError::MidiWrite(format!("创建输出目录失败: {error}")))?;
    }
    let bytes = encode_midi(notes)?;
    let mut file = File::create(path)
        .map_err(|error| TranscribeError::MidiWrite(format!("创建文件失败: {error}")))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| TranscribeError::MidiWrite(format!("写入文件失败: {error}")))?;
    validate_midi(&bytes, notes.len())
}

fn encode_midi(notes: &[NoteEvent]) -> Result<Vec<u8>, TranscribeError> {
    let mut timed = Vec::with_capacity(notes.len() * 2);
    for note in notes {
        if note.pitch > 127
            || note.velocity == 0
            || !note.start_seconds.is_finite()
            || !note.end_seconds.is_finite()
            || note.start_seconds < 0.0
            || note.end_seconds <= note.start_seconds
        {
            return Err(TranscribeError::MidiWrite(format!(
                "无效音符: pitch={}, velocity={}, start={}, end={}",
                note.pitch, note.velocity, note.start_seconds, note.end_seconds
            )));
        }
        let start = seconds_to_ticks(note.start_seconds)?;
        let end = seconds_to_ticks(note.end_seconds)?.max(start.saturating_add(1));
        timed.push(TimedMidiEvent {
            tick: start,
            is_on: true,
            pitch: note.pitch,
            velocity: note.velocity,
        });
        timed.push(TimedMidiEvent {
            tick: end,
            is_on: false,
            pitch: note.pitch,
            velocity: 0,
        });
    }
    // 同 tick 时先 NoteOff 再 NoteOn，防止连奏音被错误粘连。
    timed.sort_by_key(|event| (event.tick, event.is_on, event.pitch));

    let mut track = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(TEMPO_US_PER_QUARTER))),
    }];
    let mut previous_tick = 0;
    for event in timed {
        track.push(TrackEvent {
            delta: u28::new(event.tick.saturating_sub(previous_tick)),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message: if event.is_on {
                    MidiMessage::NoteOn {
                        key: u7::new(event.pitch),
                        vel: u7::new(event.velocity),
                    }
                } else {
                    MidiMessage::NoteOff {
                        key: u7::new(event.pitch),
                        vel: u7::new(0),
                    }
                },
            },
        });
        previous_tick = event.tick;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let smf = Smf {
        header: Header::new(
            Format::SingleTrack,
            Timing::Metrical(u15::new(TICKS_PER_QUARTER)),
        ),
        tracks: vec![track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .map_err(|error| TranscribeError::MidiWrite(error.to_string()))?;
    Ok(bytes)
}

fn seconds_to_ticks(seconds: f64) -> Result<u32, TranscribeError> {
    let ticks = (seconds * TICKS_PER_SECOND).round();
    if !ticks.is_finite() || ticks < 0.0 || ticks > 0x0fff_ffff_u32 as f64 {
        return Err(TranscribeError::MidiWrite(format!(
            "音符时间超出 MIDI 范围: {seconds} 秒"
        )));
    }
    Ok(ticks as u32)
}

fn validate_midi(bytes: &[u8], expected_notes: usize) -> Result<(), TranscribeError> {
    let parsed =
        Smf::parse(bytes).map_err(|error| TranscribeError::MidiValidation(error.to_string()))?;
    let actual_notes = parsed
        .tracks
        .iter()
        .flatten()
        .filter(|event| {
            matches!(
                event.kind,
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { vel, .. },
                    ..
                } if vel.as_int() > 0
            )
        })
        .count();
    if actual_notes != expected_notes {
        return Err(TranscribeError::MidiValidation(format!(
            "写入后音符数不一致: 期望 {expected_notes}，实际 {actual_notes}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_round_trip_preserves_note_count() {
        let notes = vec![
            NoteEvent {
                start_seconds: 0.0,
                end_seconds: 0.5,
                pitch: 60,
                velocity: 100,
                confidence: 0.8,
            },
            NoteEvent {
                start_seconds: 0.5,
                end_seconds: 1.0,
                pitch: 60,
                velocity: 90,
                confidence: 0.7,
            },
        ];
        let bytes = encode_midi(&notes).unwrap();
        validate_midi(&bytes, 2).unwrap();
        assert_eq!(&bytes[..4], b"MThd");
    }

    #[test]
    fn midi_rejects_invalid_note() {
        let invalid = NoteEvent {
            start_seconds: 1.0,
            end_seconds: 0.5,
            pitch: 60,
            velocity: 100,
            confidence: 0.8,
        };
        assert!(encode_midi(&[invalid]).is_err());
    }
}
