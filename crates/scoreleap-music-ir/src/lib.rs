//! ScoreLeap 统一音乐中间表示（Music IR）。
//!
//! 所有时间字段以**整数微秒**（`*_us: i64`）表示绝对时间，禁止浮点秒。
//! 本 crate 零业务依赖，是 workspace 依赖方向的最底层。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SMF 文件格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiFormat {
    /// 格式 0：单轨。
    SingleTrack,
    /// 格式 1：多轨并行。
    Parallel,
    /// 格式 2：多轨顺序（独立序列）。
    Sequential,
}

/// 平台无关的按键标识。
///
/// Windows 后端承载为扫描码（普通/扩展）；Android 后端暂不使用（手势走坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    /// 普通扫描码（Scan 1 make code）。
    Scan(u16),
    /// 扩展扫描码（E0 前缀，如方向键、右 Ctrl）。
    ExtendedScan(u16),
}

impl KeyCode {
    /// 构造普通扫描码。
    pub fn scan(code: u16) -> Self {
        KeyCode::Scan(code)
    }
    /// 构造扩展扫描码。
    pub fn extended_scan(code: u16) -> Self {
        KeyCode::ExtendedScan(code)
    }
}

/// 单个音符事件。绝对时间微秒。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub track_id: u16,
    /// MIDI note number（0-127）。
    pub note: u8,
    /// 力度 1-127（解析时 0 已归一化为 NoteOff）。
    pub velocity: u8,
    /// 起始绝对时间（微秒）。
    pub start_us: i64,
    /// 时值（微秒），由 NoteOff 时刻 - NoteOn 时刻。
    pub duration_us: i64,
}

/// Tempo 事件：每四分音符微秒（等价于 BPM = 60_000_000 / tempo_us_per_quarter）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoEvent {
    pub time_us: i64,
    pub tempo_us_per_quarter: u32,
}

/// 拍号事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSignatureEvent {
    pub time_us: i64,
    pub numerator: u8,
    /// 分母为 2 的幂（如 4/4 → denominator=4）。
    pub denominator: u8,
}

/// 单条轨道。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: u16,
    pub name: String,
    pub notes: Vec<NoteEvent>,
    pub instrument: Option<String>,
}

/// 统一音乐文档：所有事件绝对时间有序（由解析器保证）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicDocument {
    pub format: MidiFormat,
    pub tracks: Vec<Track>,
    /// 按 time_us 升序。
    pub tempo_events: Vec<TempoEvent>,
    /// 按 time_us 升序。
    pub time_signature_events: Vec<TimeSignatureEvent>,
    /// 文档总时长（微秒）。
    pub duration_us: i64,
}

impl MusicDocument {
    /// 总音符数。
    pub fn note_count(&self) -> usize {
        self.tracks.iter().map(|t| t.notes.len()).sum()
    }

    /// 指定轨道集合的音符数。
    pub fn note_count_of(&self, enabled_tracks: &[u16]) -> usize {
        self.tracks
            .iter()
            .filter(|t| enabled_tracks.contains(&t.id))
            .map(|t| t.notes.len())
            .sum()
    }

    /// BPM 范围（用于 UI 显示；基于 tempo_events 的首尾与默认 120）。
    pub fn bpm_range(&self) -> (f64, f64) {
        let mut min = 120.0f64;
        let mut max = 120.0f64;
        for t in &self.tempo_events {
            let bpm = 60_000_000.0 / t.tempo_us_per_quarter as f64;
            min = min.min(bpm);
            max = max.max(bpm);
        }
        (min, max)
    }
}

/// 校准锚点：归一化坐标（0-1）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KeyAnchor {
    pub note: u8,
    pub x: f32,
    pub y: f32,
}

/// Android 校准 Profile。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub name: String,
    pub device: String,
    pub resolution: (u32, u32),
    pub anchors: Vec<KeyAnchor>,
}

/// 乐器键位槽：音名 + 归一化坐标（Android 布局用）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KeySlot {
    pub note: u8,
    pub x: f32,
    pub y: f32,
}

/// 乐器布局。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentLayout {
    pub keys: Vec<KeySlot>,
}

/// 游戏乐器 Profile（平台无关定义；游戏名仅出现在元数据）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameProfile {
    pub id: String,
    pub display_name: String,
    pub version: u32,
    /// 键数。
    pub keys: u8,
    /// 乐器最低音（MIDI note）。
    pub midi_low: u8,
    /// 乐器最高音（MIDI note）。
    pub midi_high: u8,
    /// 最大复音数。
    pub max_polyphony: u8,
    /// 音名 → 按键映射。
    pub keymap: HashMap<u8, KeyCode>,
    /// Android 布局（归一化坐标）。
    pub layout: InstrumentLayout,
    /// 风险提示文案。
    pub warning: String,
}

impl GameProfile {
    /// 音域是否包含该音符。
    pub fn contains(&self, note: u8) -> bool {
        note >= self.midi_low && note <= self.midi_high
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;

    fn sample_doc() -> MusicDocument {
        MusicDocument {
            format: MidiFormat::Parallel,
            tracks: vec![Track {
                id: 0,
                name: "Test".into(),
                notes: vec![NoteEvent {
                    track_id: 0,
                    note: 60,
                    velocity: 100,
                    start_us: 0,
                    duration_us: 500_000,
                }],
                instrument: None,
            }],
            tempo_events: vec![TempoEvent {
                time_us: 0,
                tempo_us_per_quarter: 500_000,
            }],
            time_signature_events: vec![TimeSignatureEvent {
                time_us: 0,
                numerator: 4,
                denominator: 4,
            }],
            duration_us: 500_000,
        }
    }

    #[test]
    fn serde_roundtrip() {
        let doc = sample_doc();
        let json = serde_json::to_string(&doc).unwrap();
        let back: MusicDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn note_count_and_of() {
        let doc = sample_doc();
        assert_eq!(doc.note_count(), 1);
        assert_eq!(doc.note_count_of(&[0]), 1);
        assert_eq!(doc.note_count_of(&[1]), 0);
    }

    #[test]
    fn bpm_range_default_and_custom() {
        let doc = sample_doc();
        let (lo, hi) = doc.bpm_range();
        assert!((lo - 120.0).abs() < 1e-9 && (hi - 120.0).abs() < 1e-9);
        let doc2 = MusicDocument {
            tempo_events: vec![
                TempoEvent {
                    time_us: 0,
                    tempo_us_per_quarter: 500_000,
                },
                TempoEvent {
                    time_us: 1_000_000,
                    tempo_us_per_quarter: 250_000,
                },
            ],
            ..sample_doc()
        };
        let (lo2, hi2) = doc2.bpm_range();
        assert!((lo2 - 120.0).abs() < 1e-9);
        assert!((hi2 - 240.0).abs() < 1e-9);
    }

    #[test]
    fn keycode_serde() {
        let k = KeyCode::extended_scan(0x1D);
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(serde_json::from_str::<KeyCode>(&json).unwrap(), k);
    }

    #[test]
    fn profile_contains() {
        let p = GameProfile {
            id: "t".into(),
            display_name: "T".into(),
            version: 1,
            keys: 36,
            midi_low: 60,
            midi_high: 95,
            max_polyphony: 4,
            keymap: HashMap::new(),
            layout: InstrumentLayout { keys: vec![] },
            warning: String::new(),
        };
        assert!(p.contains(60));
        assert!(p.contains(95));
        assert!(!p.contains(59));
        assert!(!p.contains(96));
    }
}
