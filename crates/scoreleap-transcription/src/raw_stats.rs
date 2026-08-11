//! 转录原始输出统计：解析 worker 写出的 metadata.json 的 notes 数组，
//! 用于区分「模型漏掉高音」与「编排/后处理删除高音」（Issue #57 可观测性）。

use serde::Serialize;

/// 原始转录统计（基于 metadata.notes 的 pitch 分布）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RawTranscriptionStats {
    pub raw_note_count: usize,
    pub raw_min_pitch: Option<u8>,
    pub raw_max_pitch: Option<u8>,
    /// MIDI 21–47：低于游戏音域（48–83）。
    pub low_outside_game: usize,
    /// MIDI 48–83：可直接演奏。
    pub directly_playable: usize,
    /// MIDI 84–108：高于游戏音域，需要折叠。
    pub high_outside_game: usize,
}

impl RawTranscriptionStats {
    /// 从 worker 写出的 metadata.json 字节解析统计。
    /// metadata 结构（Transkun v2 worker）：
    /// `{ "note_count": N, "notes": [{ "start_seconds": f64, "end_seconds": f64,
    ///    "pitch": u8, "velocity": u8 }, ...] }`
    pub fn from_metadata_json(bytes: &[u8]) -> Option<Self> {
        #[derive(serde::Deserialize)]
        struct Metadata {
            notes: Option<Vec<Note>>,
        }
        #[derive(serde::Deserialize)]
        struct Note {
            #[serde(default)]
            pitch: Option<serde_json::Value>,
        }

        let metadata: Metadata = serde_json::from_slice(bytes).ok()?;
        let notes = metadata.notes?;
        let mut stats = RawTranscriptionStats {
            raw_note_count: notes.len(),
            ..Default::default()
        };
        for note in notes {
            let Some(pitch) = note.pitch.and_then(|value| value.as_u64()).map(|p| p as u8) else {
                continue; // 非法 pitch 跳过，不影响其余统计
            };
            stats.raw_min_pitch = Some(stats.raw_min_pitch.map_or(pitch, |m: u8| m.min(pitch)));
            stats.raw_max_pitch = Some(stats.raw_max_pitch.map_or(pitch, |m: u8| m.max(pitch)));
            match pitch {
                21..=47 => stats.low_outside_game += 1,
                48..=83 => stats.directly_playable += 1,
                84..=108 => stats.high_outside_game += 1,
                _ => {} // 21–108 之外不计入分段（worker clean_notes 已过滤）
            }
        }
        Some(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notes_and_segments() {
        let json = br#"{
            "schema_version": 1,
            "engine": "scoreleap-transkun-v2",
            "note_count": 5,
            "notes": [
                {"start_seconds": 0.0, "end_seconds": 0.5, "pitch": 36, "velocity": 90},
                {"start_seconds": 0.0, "end_seconds": 0.5, "pitch": 60, "velocity": 80},
                {"start_seconds": 0.5, "end_seconds": 1.0, "pitch": 96, "velocity": 70},
                {"start_seconds": 0.5, "end_seconds": 1.0, "pitch": 100, "velocity": 66},
                {"start_seconds": 1.0, "end_seconds": 1.5, "pitch": 108, "velocity": 60}
            ]
        }"#;
        let stats = RawTranscriptionStats::from_metadata_json(json).expect("应可解析");
        assert_eq!(stats.raw_note_count, 5);
        assert_eq!(stats.raw_min_pitch, Some(36));
        assert_eq!(stats.raw_max_pitch, Some(108));
        assert_eq!(stats.low_outside_game, 1); // 36
        assert_eq!(stats.directly_playable, 1); // 60
        assert_eq!(stats.high_outside_game, 3); // 96/100/108
    }

    #[test]
    fn missing_notes_yields_none() {
        let json = br#"{"schema_version":1,"engine":"scoreleap-transkun-v2"}"#;
        assert!(RawTranscriptionStats::from_metadata_json(json).is_none());
    }

    #[test]
    fn empty_notes_yields_zeroed_stats() {
        let json = br#"{"note_count":0,"notes":[]}"#;
        let stats = RawTranscriptionStats::from_metadata_json(json).expect("空数组应可解析");
        assert_eq!(stats.raw_note_count, 0);
        assert_eq!(stats.raw_min_pitch, None);
        assert_eq!(stats.high_outside_game, 0);
    }

    #[test]
    fn invalid_pitch_skipped() {
        let json = br#"{"notes":[{"pitch":60},{"pitch":"oops"}]}"#;
        let stats = RawTranscriptionStats::from_metadata_json(json).expect("应可解析");
        assert_eq!(stats.raw_note_count, 2);
        assert_eq!(stats.directly_playable, 1);
        assert_eq!(stats.raw_max_pitch, Some(60));
    }
}
