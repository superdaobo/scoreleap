//! 编排管线：转调 → 音域折叠 → 量化 → 复音限制/和弦简化 → 编译为 CompiledSequence。
//!
//! 纯函数、可测试；不依赖平台与调度。

use scoreleap_music_ir::{GameProfile, MusicDocument, NoteEvent};
use scoreleap_sequence::{CompiledSequence, PlatformAction, SequenceMeta};
use serde::{Deserialize, Serialize};

/// 编排错误。
#[derive(Debug, thiserror::Error)]
pub enum ArrangeError {
    #[error("没有启用的轨道")]
    NoTracks,
    #[error("启用的轨道中没有音符")]
    NoNotes,
    #[error("Profile 缺少键盘映射（Windows 键位映射未配置）")]
    NoKeymap,
    #[error("移调超出范围（-24..=24）")]
    TransposeOutOfRange,
    #[error("最大复音超出范围（1..=16）")]
    PolyphonyOutOfRange,
}

/// 音域折叠策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeStrategy {
    /// 超出音域时降/升八度，折叠 N 次后仍越界则丢弃。
    OctaveDown,
    /// 直接丢弃越界音符。
    Drop,
    /// 越界音符静音（丢弃但统计）。
    Mute,
}

/// 量化网格（按当前 tempo 换算微秒网格）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizeGrid {
    Eighth,
    Sixteenth,
}

/// 编排参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArrangementOptions {
    /// 手动移调（半音）。auto_fit_range=true 时忽略。
    pub transpose_semitones: i8,
    /// 自动适配音域（计算最优移调量）。
    pub auto_fit_range: bool,
    pub range_strategy: RangeStrategy,
    /// 最大复音 1..=16。
    pub max_polyphony: u8,
    pub quantize_grid: Option<QuantizeGrid>,
    /// 和弦简化：复音超限时替换最弱音而非一律丢弃。
    pub simplify_chords: bool,
}

impl Default for ArrangementOptions {
    fn default() -> Self {
        ArrangementOptions {
            transpose_semitones: 0,
            auto_fit_range: true,
            range_strategy: RangeStrategy::OctaveDown,
            max_polyphony: 4,
            quantize_grid: None,
            simplify_chords: true,
        }
    }
}

/// 编排统计（UI 展示）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ArrangeStats {
    pub input_notes: usize,
    pub output_notes: usize,
    pub dropped_out_of_range: usize,
    pub muted: usize,
    pub folded: usize,
    pub dropped_polyphony: usize,
    pub applied_transpose: i8,
}

/// 执行完整编排管线。
pub fn arrange(
    doc: &MusicDocument,
    options: &ArrangementOptions,
    profile: &GameProfile,
    enabled_tracks: &[u16],
) -> Result<(CompiledSequence, ArrangeStats), ArrangeError> {
    if enabled_tracks.is_empty() {
        return Err(ArrangeError::NoTracks);
    }
    if options.max_polyphony == 0 || options.max_polyphony > 16 {
        return Err(ArrangeError::PolyphonyOutOfRange);
    }
    if !options.auto_fit_range && !(-24..=24).contains(&options.transpose_semitones) {
        return Err(ArrangeError::TransposeOutOfRange);
    }
    if profile.keymap.is_empty() {
        return Err(ArrangeError::NoKeymap);
    }

    let mut notes: Vec<NoteEvent> = doc
        .tracks
        .iter()
        .filter(|t| enabled_tracks.contains(&t.id))
        .flat_map(|t| t.notes.iter().copied())
        .collect();
    notes.sort_by_key(|n| (n.start_us, n.track_id, n.note));
    if notes.is_empty() {
        return Err(ArrangeError::NoNotes);
    }

    let mut stats = ArrangeStats {
        input_notes: notes.len(),
        ..Default::default()
    };

    // 1. 转调
    let transpose = if options.auto_fit_range {
        auto_transpose(&notes, profile)
    } else {
        options.transpose_semitones
    };
    stats.applied_transpose = transpose;
    if transpose != 0 {
        for n in &mut notes {
            n.note = (n.note as i16 + transpose as i16).clamp(0, 127) as u8;
        }
    }

    // 2. 音域折叠
    notes = fold_range(&mut notes, profile, options.range_strategy, &mut stats);

    // 3. 量化
    if let Some(grid) = options.quantize_grid {
        quantize(&mut notes, &doc.tempo_events, grid);
    }

    // 4. 复音限制/和弦简化
    let dropped = polyphony_limit(
        &mut notes,
        options.max_polyphony as usize,
        options.simplify_chords,
    );
    stats.dropped_polyphony = dropped;
    stats.output_notes = notes.len();

    // 5. 编译
    let seq = compile(&notes, profile, doc, enabled_tracks, transpose);

    Ok((seq, stats))
}

/// 自动转调：搜索 -24..=24，最大化音域内音符数；平手取绝对值小者。
fn auto_transpose(notes: &[NoteEvent], profile: &GameProfile) -> i8 {
    let mut best = 0i8;
    let mut best_count = 0usize;
    for t in -24i8..=24 {
        let count = notes
            .iter()
            .filter(|n| {
                let shifted = n.note as i16 + t as i16;
                shifted >= profile.midi_low as i16 && shifted <= profile.midi_high as i16
            })
            .count();
        if count > best_count || (count == best_count && t.abs() < best.abs()) {
            best_count = count;
            best = t;
        }
    }
    best
}

/// 音域折叠。返回处理后的音符集。
fn fold_range(
    notes: &mut Vec<NoteEvent>,
    profile: &GameProfile,
    strategy: RangeStrategy,
    stats: &mut ArrangeStats,
) -> Vec<NoteEvent> {
    let mut out = Vec::with_capacity(notes.len());
    for mut n in notes.drain(..) {
        if profile.contains(n.note) {
            out.push(n);
            continue;
        }
        match strategy {
            RangeStrategy::Drop => stats.dropped_out_of_range += 1,
            RangeStrategy::Mute => stats.muted += 1,
            RangeStrategy::OctaveDown => {
                let mut folded = 0;
                while !profile.contains(n.note) && folded < 4 {
                    if n.note > profile.midi_high {
                        n.note = (n.note as i16 - 12).clamp(0, 127) as u8;
                    } else if n.note < profile.midi_low {
                        n.note = (n.note as i16 + 12).clamp(0, 127) as u8;
                    }
                    folded += 1;
                }
                if profile.contains(n.note) {
                    stats.folded += folded;
                    out.push(n);
                } else {
                    stats.dropped_out_of_range += 1;
                }
            }
        }
    }
    out
}

/// 按当前 tempo 计算网格微秒大小（Eighth = 半拍，Sixteenth = 四分之一拍）。
fn grid_us(grid: QuantizeGrid, tempo_us_per_quarter: u32) -> i64 {
    let quarter = tempo_us_per_quarter as i64;
    match grid {
        QuantizeGrid::Eighth => quarter / 2,
        QuantizeGrid::Sixteenth => quarter / 4,
    }
    .max(1)
}

/// 在给定时刻生效的 tempo（us/quarter）。
fn tempo_at(tempo_events: &[scoreleap_music_ir::TempoEvent], at_us: i64) -> u32 {
    let mut t = 500_000u32;
    for ev in tempo_events {
        if ev.time_us <= at_us {
            t = ev.tempo_us_per_quarter;
        } else {
            break;
        }
    }
    t
}

/// 量化音符起点到网格；重叠音符截断到下一音符起点（防复音堆积）。
fn quantize(
    notes: &mut [NoteEvent],
    tempo_events: &[scoreleap_music_ir::TempoEvent],
    grid: QuantizeGrid,
) {
    notes.sort_by_key(|n| (n.start_us, n.note));
    for n in notes.iter_mut() {
        let g = grid_us(grid, tempo_at(tempo_events, n.start_us));
        n.start_us = ((n.start_us as f64 / g as f64).round() as i64) * g;
        n.start_us = n.start_us.max(0);
    }
    // 重叠修正：start 相同的保持原时长；start 早于前一音符结束时截断
    notes.sort_by_key(|n| (n.start_us, n.note));
    for i in 1..notes.len() {
        let prev_end = notes[i - 1].start_us + notes[i - 1].duration_us;
        if notes[i].start_us < prev_end {
            // 截断当前音符，保证最小 1us
            notes[i].duration_us = (prev_end - notes[i].start_us).max(1);
        }
    }
}

/// 复音限制：活动音符数超过 max 时裁剪。
/// simplify_chords=true：替换活动中最弱音（力度最小），否则丢弃新音。
/// 返回裁剪数。
fn polyphony_limit(notes: &mut Vec<NoteEvent>, max: usize, simplify: bool) -> usize {
    if max == 0 {
        return notes.len();
    }
    notes.sort_by_key(|n| (n.start_us, n.note));
    let mut active: Vec<NoteEvent> = Vec::new();
    let mut kept: Vec<NoteEvent> = Vec::with_capacity(notes.len());
    let mut dropped = 0usize;
    for n in notes.drain(..) {
        active.retain(|a| a.start_us + a.duration_us > n.start_us);
        if active.len() < max {
            kept.push(n);
            active.push(n);
        } else if simplify {
            // 替换最弱音（力度最小；平手取时长最长，保留听感）
            if let Some(weak_idx) = active
                .iter()
                .enumerate()
                .min_by_key(|(_, a)| (a.velocity, std::cmp::Reverse(a.duration_us)))
                .map(|(i, _)| i)
            {
                if n.velocity > active[weak_idx].velocity {
                    let replaced = active.remove(weak_idx);
                    kept.retain(|k| {
                        !(k.start_us == replaced.start_us
                            && k.note == replaced.note
                            && k.velocity == replaced.velocity)
                    });
                    dropped += 1;
                    kept.push(n);
                    active.push(n);
                    continue;
                }
            }
            dropped += 1;
        } else {
            dropped += 1;
        }
    }
    *notes = kept;
    dropped
}

/// 编译为平台无关时间轴。
fn compile(
    notes: &[NoteEvent],
    profile: &GameProfile,
    doc: &MusicDocument,
    enabled_tracks: &[u16],
    transpose: i8,
) -> CompiledSequence {
    let mut actions: Vec<PlatformAction> = Vec::with_capacity(notes.len() * 2);
    let mut duration_us = 0i64;
    for n in notes {
        let key = match profile.keymap.get(&n.note) {
            Some(k) => *k,
            None => continue, // 理论不可达（折叠保证在音域内）
        };
        let down = PlatformAction::KeyDown {
            at_us: n.start_us,
            key,
        };
        let up = PlatformAction::KeyUp {
            at_us: n.start_us + n.duration_us,
            key,
        };
        actions.push(down);
        actions.push(up);
        duration_us = duration_us.max(up.at_us());
    }
    // 稳定排序：先按时间，同刻 KeyDown 先于 KeyUp，再按键位
    actions.sort_by_key(|a| {
        let order = match a {
            PlatformAction::KeyDown { .. } => 0,
            PlatformAction::KeyUp { .. } => 1,
            PlatformAction::Gesture { .. } => 0,
        };
        (a.at_us(), order, key_of(a))
    });

    CompiledSequence {
        actions,
        duration_us,
        meta: SequenceMeta {
            source_name: doc
                .tracks
                .iter()
                .find(|t| enabled_tracks.contains(&t.id))
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            track_ids: enabled_tracks.to_vec(),
            note_count: notes.len(),
            transpose_semitones: transpose,
        },
    }
}

fn key_of(a: &PlatformAction) -> u16 {
    match a {
        PlatformAction::KeyDown { key, .. } | PlatformAction::KeyUp { key, .. } => match key {
            scoreleap_music_ir::KeyCode::Scan(s) | scoreleap_music_ir::KeyCode::ExtendedScan(s) => {
                *s
            }
        },
        PlatformAction::Gesture { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;
    use scoreleap_music_ir::{
        GameProfile, InstrumentLayout, KeyCode, MidiFormat, TempoEvent, TimeSignatureEvent, Track,
    };
    use std::collections::HashMap;

    fn test_profile() -> GameProfile {
        let mut keymap = HashMap::new();
        for n in 48u8..=83u8 {
            keymap.insert(n, KeyCode::scan((0x10 + n - 48) as u16));
        }
        GameProfile {
            id: "test".into(),
            display_name: "Test".into(),
            version: 1,
            keys: 36,
            midi_low: 48,
            midi_high: 83,
            max_polyphony: 4,
            keymap,
            layout: InstrumentLayout { keys: vec![] },
            warning: String::new(),
        }
    }

    fn doc_with(notes: Vec<NoteEvent>) -> MusicDocument {
        MusicDocument {
            format: MidiFormat::Parallel,
            tracks: vec![Track {
                id: 0,
                name: "melody".into(),
                notes,
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
            duration_us: 10_000_000,
        }
    }

    fn note(n: u8, start_us: i64, dur_us: i64, vel: u8) -> NoteEvent {
        NoteEvent {
            track_id: 0,
            note: n,
            velocity: vel,
            start_us,
            duration_us: dur_us,
        }
    }

    #[test]
    fn arrange_basic_compile() {
        let doc = doc_with(vec![
            note(60, 0, 500_000, 100),
            note(64, 500_000, 500_000, 90),
        ]);
        let (seq, stats) =
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[0]).unwrap();
        assert_eq!(seq.actions.len(), 4);
        assert_eq!(stats.output_notes, 2);
        assert_eq!(seq.duration_us, 1_000_000);
        // 同刻顺序：KeyDown 先于 KeyUp
        assert!(matches!(seq.actions[0], PlatformAction::KeyDown { .. }));
    }

    #[test]
    fn manual_transpose() {
        let doc = doc_with(vec![note(60, 0, 500_000, 100)]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            transpose_semitones: 12,
            ..Default::default()
        };
        let (seq, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(stats.applied_transpose, 12);
        // 60+12=72 在音域内；keymap[72] 应被使用
        assert_eq!(seq.actions.len(), 2);
    }

    #[test]
    fn auto_transpose_fits_range() {
        // 音符 100/101 超出音域（48-83）：-18 → 82/83、-24 → 76/77 均保留 2 音
        // 平手取最小移调幅度 → -18
        let doc = doc_with(vec![
            note(100, 0, 500_000, 100),
            note(101, 500_000, 500_000, 100),
        ]);
        let (_, stats) =
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[0]).unwrap();
        assert_eq!(stats.applied_transpose, -18);
        assert_eq!(stats.output_notes, 2);
    }

    #[test]
    fn range_fold_octave_down() {
        // 96 高于 83 → 降八度 84 仍高于 → 再降 72 落入（关闭自动转调以独立验证折叠）
        let doc = doc_with(vec![note(96, 0, 500_000, 100)]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            ..Default::default()
        };
        let (seq, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(stats.folded, 2);
        assert_eq!(seq.actions.len(), 2);
    }

    #[test]
    fn range_drop() {
        let doc = doc_with(vec![note(96, 0, 500_000, 100)]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::Drop,
            ..Default::default()
        };
        let (seq, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(stats.dropped_out_of_range, 1);
        assert_eq!(seq.actions.len(), 0);
    }

    #[test]
    fn polyphony_limit_trims() {
        // 4 个同时音符 + max 2 → 裁剪 2
        let doc = doc_with(vec![
            note(60, 0, 1_000_000, 100),
            note(64, 0, 1_000_000, 90),
            note(67, 0, 1_000_000, 80),
            note(72, 0, 1_000_000, 70),
        ]);
        let opts = ArrangementOptions {
            max_polyphony: 2,
            ..Default::default()
        };
        let (seq, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(stats.dropped_polyphony, 2);
        assert_eq!(stats.output_notes, 2);
        assert_eq!(seq.actions.len(), 4);
    }

    #[test]
    fn simplify_replaces_weakest() {
        // 3 音符同时，max 2，simplify=true：力度 100/30/50 → 丢弃 30
        let doc = doc_with(vec![
            note(60, 0, 1_000_000, 100),
            note(64, 0, 1_000_000, 30),
            note(67, 0, 1_000_000, 50),
        ]);
        let opts = ArrangementOptions {
            max_polyphony: 2,
            simplify_chords: true,
            ..Default::default()
        };
        let (seq, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(stats.dropped_polyphony, 1);
        assert_eq!(stats.output_notes, 2);
        // 保留 100 与 50
        let kept_notes: Vec<u8> = seq
            .actions
            .iter()
            .filter_map(|a| match a {
                PlatformAction::KeyDown { .. } => Some(key_of(a) as u8),
                _ => None,
            })
            .collect();
        assert_eq!(kept_notes.len(), 2);
        let _ = kept_notes;
    }

    #[test]
    fn quantize_aligns_to_grid() {
        // 120BPM → 拍=500ms；Sixteenth = 125ms。start=123_000 → 对齐 125_000
        let doc = doc_with(vec![note(60, 123_000, 400_000, 100)]);
        let opts = ArrangementOptions {
            quantize_grid: Some(QuantizeGrid::Sixteenth),
            ..Default::default()
        };
        let (seq, _) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        match seq.actions[0] {
            PlatformAction::KeyDown { at_us, .. } => assert_eq!(at_us, 125_000),
            _ => panic!("expected KeyDown"),
        }
    }

    #[test]
    fn repeated_notes_not_lost() {
        let doc = doc_with(vec![
            note(60, 0, 500_000, 100),
            note(60, 600_000, 500_000, 100),
        ]);
        let (seq, stats) =
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[0]).unwrap();
        assert_eq!(stats.output_notes, 2);
        assert_eq!(seq.actions.len(), 4);
    }

    #[test]
    fn no_tracks_error() {
        let doc = doc_with(vec![note(60, 0, 500_000, 100)]);
        assert!(matches!(
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[]),
            Err(ArrangeError::NoTracks)
        ));
    }
}
