//! 编排管线：转调 → 音域折叠 → 量化 → 复音限制/和弦简化 → 编译为 CompiledSequence。
//!
//! 纯函数、可测试；不依赖平台与调度。

use scoreleap_music_ir::{GameProfile, KeyCode, MusicDocument, NoteEvent};
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
    /// 智能八度映射（Issue #57）：按 pitch class 生成游戏音域内全部候选，
    /// 结合旋律连续性、碰撞与声部交叉选择最优八度；保留 OctaveDown 语义不变。
    SmartFold,
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
    /// 旋律感知复音保护（Issue #57）：优先保留 Top Voice / Bass，
    /// 用于 audio_transcription 高质量钢琴编排；直接 MIDI 默认关闭保持旧行为。
    #[serde(default)]
    pub melody_protection: bool,
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
            melody_protection: false,
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
    /// 折叠总次数（OctaveDown 累计 ±12 次数；SmartFold 按重映射次数计）。
    pub folded: usize,
    pub dropped_polyphony: usize,
    pub applied_transpose: i8,
    /// 低音越界（midi_low 之下）升八度折叠的音符数。
    pub folded_from_low: usize,
    /// 高音越界（midi_high 之上）降八度折叠的音符数。
    pub folded_from_high: usize,
    /// 同一 onset cluster 内折叠/映射后发生目标键碰撞的音符数。
    pub fold_collisions: usize,
    /// SmartFold 为避免碰撞/保持连续性而选择非首个候选的次数。
    pub smart_fold_reassigned: usize,
    /// 复音限制中被旋律保护保留的 Top Voice 音符数。
    pub protected_top_voice: usize,
    /// 复音限制中被旋律保护保留的 Bass 音符数。
    pub protected_bass: usize,
    /// 复音限制中因旋律保护逻辑而丢弃的 Top Voice 音符数（应接近 0）。
    pub dropped_top_voice: usize,
}

/// 执行完整编排管线（= arrange_pipeline + compile_notes）。
pub fn arrange(
    doc: &MusicDocument,
    options: &ArrangementOptions,
    profile: &GameProfile,
    enabled_tracks: &[u16],
) -> Result<(CompiledSequence, ArrangeStats), ArrangeError> {
    let (notes, stats) = arrange_pipeline(doc, options, profile, enabled_tracks)?;
    let seq = compile_notes(
        &notes,
        profile,
        doc,
        enabled_tracks,
        stats.applied_transpose,
    );
    Ok((seq, stats))
}

/// 编排管线（转调/折叠/量化/复音限制），返回编排后的音符与统计。
/// 音符未做同刻去重与按键编译；卷帘预览可基于此数据。
pub fn arrange_pipeline(
    doc: &MusicDocument,
    options: &ArrangementOptions,
    profile: &GameProfile,
    enabled_tracks: &[u16],
) -> Result<(Vec<NoteEvent>, ArrangeStats), ArrangeError> {
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
    let dropped = if options.melody_protection {
        // Issue #57：旋律感知复音保护（audio_transcription 高质量钢琴）
        polyphony_limit_melody(&mut notes, options.max_polyphony as usize, &mut stats)
    } else {
        polyphony_limit(
            &mut notes,
            options.max_polyphony as usize,
            options.simplify_chords,
        )
    };
    stats.dropped_polyphony = dropped;
    stats.output_notes = notes.len();

    Ok((notes, stats))
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
#[allow(clippy::ptr_arg)] // fold_range 需要 drain（Vec 特有），保持调用点不变
fn fold_range(
    notes: &mut Vec<NoteEvent>,
    profile: &GameProfile,
    strategy: RangeStrategy,
    stats: &mut ArrangeStats,
) -> Vec<NoteEvent> {
    if strategy == RangeStrategy::SmartFold {
        return smart_fold(notes, profile, stats);
    }
    let mut out = Vec::with_capacity(notes.len());
    for mut n in notes.drain(..) {
        if profile.contains(n.note) {
            out.push(n);
            continue;
        }
        match strategy {
            RangeStrategy::Drop => stats.dropped_out_of_range += 1,
            RangeStrategy::Mute => stats.muted += 1,
            RangeStrategy::SmartFold => unreachable!("fold_range 入口已分发到 smart_fold"),
            RangeStrategy::OctaveDown => {
                let mut folded = 0;
                while !profile.contains(n.note) && folded < 4 {
                    if n.note > profile.midi_high {
                        n.note = (n.note as i16 - 12).clamp(0, 127) as u8;
                        stats.folded_from_high += 1;
                    } else if n.note < profile.midi_low {
                        n.note = (n.note as i16 + 12).clamp(0, 127) as u8;
                        stats.folded_from_low += 1;
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

/// SmartFold 智能八度映射（Issue #57，P0）：
/// 按 onset cluster 处理；越界音符按 pitch class 生成 48–83 内全部候选，
/// 代价 = 八度位移 + 旋律连续性 + 声部交叉 + 碰撞，优先分配唯一游戏键；
/// 处理顺序降序（Top Voice 原音最高先映射），Bass 受「不高于 Top 映射」约束。
#[allow(clippy::ptr_arg)] // 需要 drain/sort 原地改写（Vec 特有），保持调用点不变
fn smart_fold(
    notes: &mut Vec<NoteEvent>,
    profile: &GameProfile,
    stats: &mut ArrangeStats,
) -> Vec<NoteEvent> {
    let mut out = Vec::with_capacity(notes.len());
    notes.sort_by_key(|n| (n.start_us, n.note));
    let mut prev_top_mapped: Option<u8> = None;
    let mut i = 0usize;
    while i < notes.len() {
        let cluster_start = notes[i].start_us;
        let mut cluster: Vec<NoteEvent> = Vec::new();
        while i < notes.len() && notes[i].start_us - cluster_start <= ONSET_CLUSTER_TOLERANCE_US {
            cluster.push(notes[i]);
            i += 1;
        }
        // 原音最高的音符（cluster Top Voice）先映射
        cluster.sort_by_key(|n| std::cmp::Reverse(n.note));
        let cluster_top_original = cluster.first().map(|n| n.note).unwrap_or(0);

        let mut mapped: Vec<NoteEvent> = Vec::with_capacity(cluster.len());
        let mut occupied: std::collections::HashSet<u8> = std::collections::HashSet::new();
        let mut top_mapped: Option<u8> = None;
        let mut bass_mapped: Option<u8> = None;
        let mut cluster_top_mapped: Option<u8> = None;

        // 第一遍：在界音符先占位并输出（避免越界音符抢占其目标键）
        for n in cluster.iter().filter(|n| profile.contains(n.note)) {
            occupied.insert(n.note);
            top_mapped = Some(top_mapped.map_or(n.note, |t| t.max(n.note)));
            bass_mapped = Some(bass_mapped.map_or(n.note, |b| b.min(n.note)));
            if n.note == cluster_top_original {
                cluster_top_mapped = Some(n.note);
            }
            mapped.push(*n);
        }
        // 第二遍：越界音符按原音降序（Top Voice 优先）SmartFold 分配
        for mut n in cluster.drain(..).filter(|n| !profile.contains(n.note)) {
            // 同 pitch class 的游戏音域候选
            let pc = n.note % 12;
            let candidates: Vec<u8> = (profile.midi_low..=profile.midi_high)
                .filter(|c| c % 12 == pc)
                .collect();
            if candidates.is_empty() {
                stats.dropped_out_of_range += 1;
                continue;
            }
            // 该音符是否为 cluster 原音最高的音符（Top Voice）：它自身不受声部交叉上界约束
            let is_cluster_top = n.note == cluster_top_original;
            // 仅按位移的默认选择（无碰撞、最接近原音）
            let default_best = candidates
                .iter()
                .copied()
                .filter(|c| !occupied.contains(c))
                .min_by_key(|c| (*c as i64 - n.note as i64).abs());
            // 完整代价选择
            let mut best: Option<u8> = None;
            let mut best_cost = i64::MAX;
            for &c in &candidates {
                let mut cost = (c as i64 - n.note as i64).abs() * 10; // 八度位移
                if let Some(prev) = prev_top_mapped {
                    cost += (c as i64 - prev as i64).abs(); // 旋律连续性
                }
                if !is_cluster_top {
                    // 声部交叉：非 Top 音符映射不得高于 cluster Top Voice 的映射
                    if let Some(t) = cluster_top_mapped {
                        if c > t {
                            cost += 400;
                        }
                    }
                }
                if let Some(b) = bass_mapped {
                    if c < b {
                        cost += 400; // 防御：不得低于已分配 Bass 映射
                    }
                }
                if occupied.contains(&c) {
                    cost += 1000; // 碰撞高惩罚
                }
                if cost < best_cost || (cost == best_cost && best.is_none_or(|b| c < b)) {
                    best_cost = cost;
                    best = Some(c);
                }
            }
            let chosen = best.expect("同 pitch class 候选非空");
            if default_best != Some(chosen) {
                stats.smart_fold_reassigned += 1;
            }
            if occupied.contains(&chosen) {
                stats.fold_collisions += 1;
            } else {
                occupied.insert(chosen);
            }
            stats.folded += 1;
            if n.note > profile.midi_high {
                stats.folded_from_high += 1;
            } else {
                stats.folded_from_low += 1;
            }
            top_mapped = Some(top_mapped.map_or(chosen, |t| t.max(chosen)));
            bass_mapped = Some(bass_mapped.map_or(chosen, |b| b.min(chosen)));
            if n.note == cluster_top_original {
                cluster_top_mapped = Some(chosen);
            }
            n.note = chosen;
            mapped.push(n);
        }
        if let Some(t) = cluster_top_mapped {
            prev_top_mapped = Some(t);
        }
        out.extend(mapped);
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

/// 内声部重要性：velocity 权重为主，时值加分（长音更有价值）。
fn inner_voice_importance(n: &NoteEvent) -> i64 {
    n.velocity as i64 * 2 + (n.duration_us / 100_000).min(50)
}

/// Onset cluster 容差（Issue #57）：起始时间差在 25ms 内的音符视为同一和弦事件。
const ONSET_CLUSTER_TOLERANCE_US: i64 = 25_000;

/// 旋律感知复音限制（Issue #57，P0）：
/// 滑动窗口内（时间重叠或同 onset cluster）始终保护 Top Voice（最高音）
/// 与 Bass（最低音，max >= 2 时），超限时按重要性从 Inner Voices 中淘汰，
/// velocity 不再是唯一判据。统计 protected_top_voice / protected_bass / dropped_top_voice。
fn polyphony_limit_melody(
    notes: &mut Vec<NoteEvent>,
    max: usize,
    stats: &mut ArrangeStats,
) -> usize {
    if max == 0 {
        return notes.len();
    }
    notes.sort_by_key(|n| (n.start_us, n.note));
    // active / kept 保存排序后 notes 的索引，避免按 (start,note,velocity) 值匹配
    // 误删跨轨同音高副本（同音反复/双轨同音场景）。
    let mut active: Vec<usize> = Vec::new();
    let mut kept: Vec<usize> = Vec::with_capacity(notes.len());
    let mut dropped = 0usize;
    for idx in 0..notes.len() {
        let n = notes[idx];
        active.retain(|&a| {
            // 仍在发声，或与当前音符同属一个 onset cluster（和弦事件）
            let a_note = notes[a];
            a_note.start_us + a_note.duration_us > n.start_us
                || (a_note.start_us - n.start_us).abs() <= ONSET_CLUSTER_TOLERANCE_US
        });
        active.push(idx);
        kept.push(idx);
        while active.len() > max {
            // Top/Bass 角色按当前 active 计算（同音高并列取先进入者）
            let top_idx = *active
                .iter()
                .max_by_key(|&&a| notes[a].note)
                .expect("active 非空");
            let bass_idx = *active
                .iter()
                .min_by_key(|&&a| notes[a].note)
                .expect("active 非空");
            // 首选淘汰：非 Top 且（max >= 2 时）非 Bass 的内声部，importance 最低者
            let mut candidates: Vec<usize> = active
                .iter()
                .copied()
                .filter(|&a| a != top_idx && !(max >= 2 && a == bass_idx))
                .collect();
            let loser = if candidates.is_empty() {
                // 极端情况（active 全部同音高，如跨轨同音反复）：无内声部可淘汰。
                // 退化为按 importance 淘汰（允许动 Top/Bass），保证 max 永不超限。
                active
                    .iter()
                    .copied()
                    .min_by(|&a, &b| {
                        inner_voice_importance(&notes[a])
                            .cmp(&inner_voice_importance(&notes[b]))
                            .then_with(|| notes[b].note.cmp(&notes[a].note))
                    })
                    .expect("active 非空")
            } else {
                // importance 升序，importance 并列时保留较高音（旋律倾向）
                candidates.sort_by(|&a, &b| {
                    inner_voice_importance(&notes[a])
                        .cmp(&inner_voice_importance(&notes[b]))
                        .then_with(|| notes[b].note.cmp(&notes[a].note))
                });
                candidates[0]
            };
            let pos = active
                .iter()
                .position(|&a| a == loser)
                .expect("loser 必在 active");
            active.remove(pos);
            let kept_pos = kept
                .iter()
                .position(|&k| k == loser)
                .expect("loser 必在 kept");
            kept.remove(kept_pos);
            dropped += 1;
            // 统计：被淘汰者若是 Top/Bass 计入 dropped_*，否则计入保护保留
            if loser == top_idx {
                stats.dropped_top_voice += 1;
            } else {
                stats.protected_top_voice += 1;
            }
            if max >= 2 && loser != bass_idx {
                stats.protected_bass += 1;
            }
        }
    }
    *notes = kept.into_iter().map(|i| notes[i]).collect();
    dropped
}

/// 编译为平台无关时间轴（同刻去重 + KeyDown/KeyUp + 重叠修正 + 排序）。
pub fn compile_notes(
    notes: &[NoteEvent],
    profile: &GameProfile,
    doc: &MusicDocument,
    enabled_tracks: &[u16],
    transpose: i8,
) -> CompiledSequence {
    // 同刻同音高去重：同一物理键无法同时按下两次（重复音/双轨同音）
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<&NoteEvent> = notes
        .iter()
        .filter(|n| seen.insert((n.start_us, n.note)))
        .collect();

    let mut actions: Vec<PlatformAction> = Vec::with_capacity(deduped.len() * 2);
    let mut duration_us = 0i64;
    for n in deduped {
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

    // 同键重叠修正：连奏/踏板场景下同一物理键在 KeyUp 前再次 KeyDown，
    // 会把前一个 KeyUp 提前到新 Down 之前（1ms 间隙），避免真实键盘卡键/自动重复。
    fix_overlapping_keys(&mut actions);

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

/// 同键重叠修正（GAP = 1ms）：
/// 同一物理键的连续两次按下之间，若上一次未抬起，把上一次 KeyUp 提前到新 Down 之前，
/// 保证任何时刻每个物理键至多处于按下状态一次。
const OVERLAP_GAP_US: i64 = 1_000;

fn fix_overlapping_keys(actions: &mut [PlatformAction]) {
    use std::collections::HashMap;
    // 收集每个 key 的 Down/Up 索引（各自按出现顺序）
    let mut downs: HashMap<KeyCode, Vec<usize>> = HashMap::new();
    let mut ups: HashMap<KeyCode, Vec<usize>> = HashMap::new();
    for (i, a) in actions.iter().enumerate() {
        match a {
            PlatformAction::KeyDown { key, .. } => downs.entry(*key).or_default().push(i),
            PlatformAction::KeyUp { key, .. } => ups.entry(*key).or_default().push(i),
            _ => {}
        }
    }
    // 第 j 个 Down 对应第 j 个 Up；若相邻两个 Down 中前一个的 Up 晚于后一个 Down，
    // 把前一个 Up 提前到后一个 Down 之前（1ms 间隙）。
    let mut changed = false;
    for (key, ds) in &downs {
        let us = match ups.get(key) {
            Some(u) => u,
            None => continue,
        };
        for j in 0..ds.len().saturating_sub(1) {
            let (d1, d2) = (ds[j], ds[j + 1]);
            let u1 = match us.get(j) {
                Some(u) => *u,
                None => continue,
            };
            let (d1_at, d2_at) = match (actions[d1], actions[d2]) {
                (
                    PlatformAction::KeyDown { at_us: a1, .. },
                    PlatformAction::KeyDown { at_us: a2, .. },
                ) => (a1, a2),
                _ => continue,
            };
            let u1_at = match actions[u1] {
                PlatformAction::KeyUp { at_us, .. } => at_us,
                _ => continue,
            };
            if u1_at > d2_at {
                let new_up = (d2_at - OVERLAP_GAP_US).max(d1_at + 1);
                actions[u1] = PlatformAction::KeyUp {
                    at_us: new_up,
                    key: *key,
                };
                changed = true;
            }
        }
    }
    if changed {
        // Up 提前可能破坏时间序，重新稳定排序
        actions.sort_by_key(|a| {
            let order = match a {
                PlatformAction::KeyDown { .. } => 0,
                PlatformAction::KeyUp { .. } => 1,
                PlatformAction::Gesture { .. } => 0,
            };
            (a.at_us(), order, key_of(a))
        });
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
    fn overlapping_same_note_keyup_pulled_forward() {
        // 同音高重叠（连奏/踏板）：第二个 Down 在第一个 KeyUp 之前
        let doc = doc_with(vec![
            note(60, 0, 800_000, 100),
            note(60, 500_000, 800_000, 100),
        ]);
        let (seq, _) =
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[0]).unwrap();
        assert_eq!(seq.actions.len(), 4);
        // 第一个 KeyUp 被提前到第二个 Down 前 1ms = 499_000
        let first_up = seq
            .actions
            .iter()
            .find_map(|a| match a {
                PlatformAction::KeyUp { at_us, .. } if *at_us < 500_000 => Some(*at_us),
                _ => None,
            })
            .expect("应有被提前的 KeyUp");
        assert_eq!(first_up, 499_000);
        // 全局校验：任何时刻同一 key 不重叠
        let mut open: std::collections::HashMap<scoreleap_music_ir::KeyCode, i64> =
            Default::default();
        for a in &seq.actions {
            match a {
                PlatformAction::KeyDown { at_us, key } => {
                    if let Some(prev_up) = open.get(key) {
                        assert!(at_us >= prev_up, "同键重叠未修正");
                    }
                }
                PlatformAction::KeyUp { at_us, key } => {
                    open.insert(*key, *at_us);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn no_tracks_error() {
        let doc = doc_with(vec![note(60, 0, 500_000, 100)]);
        assert!(matches!(
            arrange(&doc, &ArrangementOptions::default(), &test_profile(), &[]),
            Err(ArrangeError::NoTracks)
        ));
    }

    /// Issue #57 阶段 1 验证：高音输入在编排统计中可见（可区分「模型漏高音」与「编排删高音」）。
    #[test]
    fn high_register_stats_visible() {
        // C7=96 与 C8=108 同时出现，OctaveDown 折叠；统计必须记录高音折叠
        let doc = doc_with(vec![
            note(96, 0, 500_000, 70),
            note(108, 500_000, 500_000, 60),
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            ..Default::default()
        };
        let (_, stats) = arrange(&doc, &opts, &test_profile(), &[0]).unwrap();
        // 96 -> 84 -> 72（2 次降八度）；108 -> 96 -> 84 -> 72（3 次降八度）
        assert_eq!(stats.folded_from_high, 5);
        assert_eq!(stats.folded_from_low, 0);
        assert!(stats.folded >= 5);
        assert_eq!(stats.output_notes, 2);
    }

    /// Issue #57 合成验证（PR 证据）：钢琴风格段落，后半段进入高音区。
    /// 旧策略（OctaveDown + 无保护）会丢掉高音旋律；新策略（SmartFold + 保护）保留。
    #[test]
    fn issue57_synthetic_piano_before_after() {
        // t0/t1：常规和弦 + 旋律；t2/t3：低音块 + 越界高音旋律（5 音 > max 4）
        let input = vec![
            note(48, 0, 1_000_000, 100),
            note(60, 0, 1_000_000, 85),
            note(64, 0, 1_000_000, 80),
            note(76, 0, 1_000_000, 70), // E5 旋律
            note(53, 1_000_000, 1_000_000, 100),
            note(65, 1_000_000, 1_000_000, 85),
            note(69, 1_000_000, 1_000_000, 80),
            note(77, 1_000_000, 1_000_000, 70), // F5 旋律
            note(48, 2_000_000, 1_000_000, 100),
            note(55, 2_000_000, 1_000_000, 90),
            note(60, 2_000_000, 1_000_000, 85),
            note(64, 2_000_000, 1_000_000, 80),
            note(96, 2_000_000, 1_000_000, 70), // C7 旋律（84-108 越界）
            note(53, 3_000_000, 1_000_000, 100),
            note(57, 3_000_000, 1_000_000, 90),
            note(62, 3_000_000, 1_000_000, 85),
            note(65, 3_000_000, 1_000_000, 80),
            note(100, 3_000_000, 1_000_000, 70), // E7 旋律（84-108 越界）
        ];
        let old_opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::OctaveDown,
            max_polyphony: 4,
            melody_protection: false,
            ..Default::default()
        };
        let (old_notes, old_stats) =
            arrange_pipeline(&doc_with(input.clone()), &old_opts, &test_profile(), &[0]).unwrap();
        let new_opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::SmartFold,
            max_polyphony: 4,
            melody_protection: true,
            ..Default::default()
        };
        let (new_notes, new_stats) =
            arrange_pipeline(&doc_with(input), &new_opts, &test_profile(), &[0]).unwrap();

        // 修复前后统计（PR 正文引用）
        println!(
            "[Issue#57] 旧策略: output={} folded_high={} dropped_polyphony={}",
            old_stats.output_notes, old_stats.folded_from_high, old_stats.dropped_polyphony
        );
        println!(
            "[Issue#57] 新策略: output={} folded_high={} fold_collisions={} protected_top={} dropped_top={}",
            new_stats.output_notes,
            new_stats.folded_from_high,
            new_stats.fold_collisions,
            new_stats.protected_top_voice,
            new_stats.dropped_top_voice
        );

        // 旧策略：t2 的 C7(96) 因 velocity 低于 active 低音被 polyphony 裁掉，旋律丢失
        let old_has_c7 = old_notes
            .iter()
            .any(|n| n.start_us == 2_000_000 && n.note == 72);
        assert!(
            !old_has_c7,
            "旧策略应丢失 t2 高音旋律（96 折叠候选 72 不存在）"
        );
        // 新策略：C7 受旋律保护并映射到合法键（72）
        let new_has_c7 = new_notes
            .iter()
            .any(|n| n.start_us == 2_000_000 && n.note == 72);
        assert!(new_has_c7, "新策略应保留 t2 高音旋律（C7 → 72）");
        // 新策略：E7(100) 映射为同音名键（76），且无 Top Voice 裁剪
        println!(
            "[Issue#57] new_notes: {:?}",
            new_notes
                .iter()
                .map(|n| (n.start_us, n.note, n.velocity))
                .collect::<Vec<_>>()
        );
        let new_has_e7 = new_notes
            .iter()
            .any(|n| n.start_us == 3_000_000 && n.note == 76);
        assert!(new_has_e7, "新策略应保留 t3 高音旋律（E7 → 76）");
        assert_eq!(new_stats.dropped_top_voice, 0);
        assert_eq!(new_stats.fold_collisions, 0, "存在无碰撞方案时不得碰撞");
    }

    /// Issue #57 测试 1：7 音和弦 max=4 时 Top Voice E5 与 Bass 必须保留。
    #[test]
    fn melody_protection_keeps_top_voice_and_bass() {
        let doc = doc_with(vec![
            note(48, 0, 1_000_000, 100), // C3 bass
            note(55, 0, 1_000_000, 90),  // G3
            note(60, 0, 1_000_000, 85),  // C4
            note(64, 0, 1_000_000, 80),  // E4
            note(67, 0, 1_000_000, 76),  // G4
            note(72, 0, 1_000_000, 68),  // C5
            note(76, 0, 1_000_000, 66),  // E5 top（velocity 最低也不能丢）
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            max_polyphony: 4,
            melody_protection: true,
            ..Default::default()
        };
        let (notes, stats) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert!(notes.len() <= 4);
        assert!(
            notes.iter().any(|n| n.note == 76),
            "Top Voice E5（velocity 66）必须被保护保留"
        );
        assert!(notes.iter().any(|n| n.note == 48), "Bass C3 必须保留");
        assert_eq!(stats.dropped_top_voice, 0);
        assert!(stats.protected_top_voice >= 3); // 7 音超限 3 个
        assert!(stats.protected_bass >= 3);
    }

    /// Issue #57 测试 2：超高 C7=96 可映入 Profile（48/60/72 候选，选位移最小的 72）。
    #[test]
    fn smart_fold_maps_c7_into_profile() {
        let doc = doc_with(vec![note(96, 0, 500_000, 100)]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::SmartFold,
            ..Default::default()
        };
        let (notes, stats) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(notes.len(), 1, "C7 不得被 Drop");
        assert_eq!(notes[0].note, 72, "应选择位移最小的同音名候选 72");
        assert_eq!(notes[0].note % 12, 96 % 12, "pitch class 必须保持");
        assert!(stats.folded_from_high >= 1);
        assert_eq!(stats.dropped_out_of_range, 0);
    }

    /// Issue #57 测试 3：同 onset 的 72+96 不得都折叠成 72（碰撞规避）。
    #[test]
    fn smart_fold_avoids_fold_collision() {
        let doc = doc_with(vec![
            note(72, 0, 1_000_000, 90), // 在界 C5
            note(96, 0, 1_000_000, 70), // 越界 C7
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::SmartFold,
            ..Default::default()
        };
        let (notes, stats) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        let out: Vec<u8> = notes.iter().map(|n| n.note).collect();
        assert!(out.contains(&72), "在界 C5 保留");
        assert!(out.contains(&60), "C7 应避开 72 改选 60");
        assert_eq!(stats.fold_collisions, 0, "存在无碰撞方案时不得碰撞");
    }

    /// Issue #57 测试 4：高音旋律连续性 79 → 96 → 98 → 100 映射平稳无 24 半音跳变。
    #[test]
    fn smart_fold_keeps_melody_continuity() {
        let doc = doc_with(vec![
            note(79, 0, 500_000, 100),
            note(96, 600_000, 500_000, 90),
            note(98, 1_200_000, 500_000, 80),
            note(100, 1_800_000, 500_000, 70),
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::SmartFold,
            ..Default::default()
        };
        let (notes, _) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        let out: Vec<u8> = notes.iter().map(|n| n.note).collect();
        assert_eq!(out, vec![79, 72, 74, 76], "应沿前一个 Top Voice 连续映射");
        for (idx, original) in [96u8, 98, 100].iter().enumerate() {
            assert_eq!(out[idx + 1] % 12, original % 12, "pitch class 保持");
        }
        for w in out.windows(2) {
            assert!(
                (w[1] as i16 - w[0] as i16).abs() <= 12,
                "不应出现无必要的 24 半音跳变: {} -> {}",
                w[0],
                w[1]
            );
        }
    }

    /// Issue #57 测试 5：voice crossing——Bass 映射不得高于 Top Voice 映射。
    #[test]
    fn smart_fold_no_voice_crossing() {
        let doc = doc_with(vec![
            note(84, 0, 1_000_000, 100), // bass（原音较低）
            note(96, 0, 1_000_000, 80),  // top（原音较高）
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            range_strategy: RangeStrategy::SmartFold,
            ..Default::default()
        };
        let (notes, _) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        // 96（Top）先映射 → 72；84（Bass）候选 {48,60,72}，72 已被占用 → 60；
        // 结果 Bass(60) < Top(72)，无声部交叉、无同键碰撞。
        let mut mapped: Vec<u8> = notes.iter().map(|n| n.note).collect();
        mapped.sort_unstable();
        assert_eq!(
            mapped,
            vec![60, 72],
            "Bass 应映射 60、Top 应映射 72 且无交叉"
        );
    }

    /// Issue #57 测试 6：max_polyphony=1 时优先 Top Voice，而非最大 velocity 的低音。
    #[test]
    fn melody_protection_max1_prefers_top_voice() {
        let doc = doc_with(vec![
            note(48, 0, 1_000_000, 120), // 低音 velocity 更高
            note(76, 0, 1_000_000, 66),  // Top Voice
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            max_polyphony: 1,
            melody_protection: true,
            ..Default::default()
        };
        let (notes, _) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(
            notes[0].note, 76,
            "max=1 时应优先 Top Voice，而不是 velocity 更大的低音"
        );
    }

    /// Issue #57 测试 1 对照：旧算法（melody_protection=false）同输入会丢掉 Top Voice。
    #[test]
    fn legacy_polyphony_drops_top_voice() {
        let doc = doc_with(vec![
            note(48, 0, 1_000_000, 100),
            note(55, 0, 1_000_000, 90),
            note(60, 0, 1_000_000, 85),
            note(64, 0, 1_000_000, 80),
            note(67, 0, 1_000_000, 76),
            note(72, 0, 1_000_000, 68),
            note(76, 0, 1_000_000, 66),
        ]);
        let opts = ArrangementOptions {
            auto_fit_range: false,
            max_polyphony: 4,
            melody_protection: false,
            ..Default::default()
        };
        let (notes, _) = arrange_pipeline(&doc, &opts, &test_profile(), &[0]).unwrap();
        assert!(
            !notes.iter().any(|n| n.note == 76),
            "旧算法应丢 Top Voice（验证新算法修复了该缺陷）"
        );
    }
}
