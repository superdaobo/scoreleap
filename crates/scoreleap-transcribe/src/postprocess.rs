use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};

use crate::{
    ResolvedThresholds, StitchedActivations, TranscribeError, AUDIO_SAMPLE_RATE,
    AUDIO_WINDOW_SAMPLES, FFT_HOP, MIDI_OFFSET, MODEL_OUTPUT_FRAMES, NOTE_BINS,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub pitch: u8,
    pub velocity: u8,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
struct FrameNote {
    start: usize,
    end: usize,
    pitch_index: usize,
    amplitude: f32,
}

pub fn activations_to_notes(
    activations: &StitchedActivations,
    thresholds: ResolvedThresholds,
) -> Result<Vec<NoteEvent>, TranscribeError> {
    let thresholds = thresholds.validate()?;
    validate_activations(activations)?;
    if activations.frame_count == 0 {
        return Ok(Vec::new());
    }

    let mut onsets = activations.onsets.clone();
    if thresholds.infer_onsets {
        infer_onsets(&mut onsets, &activations.frames);
    }

    // `remaining` 与官方 Basic Pitch 一致：已被强 onset 解释的能量会清零，
    // 防止相邻音高和同音重复触发；余能量再交给 melodia 补召回。
    let mut remaining = activations.frames.clone();
    let mut notes = Vec::new();
    for frame in (1..activations.frame_count.saturating_sub(1)).rev() {
        for pitch in (0..NOTE_BINS).rev() {
            let current = onsets[index(frame, pitch)];
            if current < thresholds.onset
                || current <= onsets[index(frame - 1, pitch)]
                || current <= onsets[index(frame + 1, pitch)]
            {
                continue;
            }
            if let Some(note) = extend_forward(
                frame,
                pitch,
                &remaining,
                thresholds.frame,
                thresholds.energy_tolerance_frames,
                activations.frame_count,
            ) {
                if note.end - note.start > minimum_frames(thresholds.minimum_note_length_ms) {
                    clear_note_energy(&mut remaining, &note, activations.frame_count);
                    notes.push(with_amplitude(note, &activations.frames));
                }
            }
        }
    }

    if thresholds.melodia_trick {
        add_remaining_energy_notes(
            &mut notes,
            &mut remaining,
            &activations.frames,
            activations.frame_count,
            thresholds,
        );
    }

    let mut events: Vec<_> = notes
        .into_iter()
        .map(|note| NoteEvent {
            start_seconds: model_frame_to_seconds(note.start),
            end_seconds: model_frame_to_seconds(note.end),
            pitch: MIDI_OFFSET + note.pitch_index as u8,
            velocity: (note.amplitude.clamp(0.0, 1.0) * 127.0)
                .round()
                .clamp(1.0, 127.0) as u8,
            confidence: note.amplitude.clamp(0.0, 1.0),
        })
        .filter(|event| event.end_seconds > event.start_seconds)
        .collect();
    suppress_duplicates(&mut events, thresholds.duplicate_gap_ms as f64 / 1_000.0);
    Ok(events)
}

fn validate_activations(value: &StitchedActivations) -> Result<(), TranscribeError> {
    if value.frames.len() != value.frame_count * NOTE_BINS
        || value.onsets.len() != value.frame_count * NOTE_BINS
        || value.contours.len() != value.frame_count * crate::CONTOUR_BINS
    {
        return Err(TranscribeError::InvalidOutput(
            "完整激活矩阵尺寸与 frame_count 不一致".into(),
        ));
    }
    if value
        .frames
        .iter()
        .chain(&value.onsets)
        .chain(&value.contours)
        .any(|value| !value.is_finite())
    {
        return Err(TranscribeError::InvalidOutput(
            "完整激活矩阵包含 NaN 或无穷值".into(),
        ));
    }
    Ok(())
}

fn infer_onsets(onsets: &mut [f32], frames: &[f32]) {
    let frame_count = frames.len() / NOTE_BINS;
    let mut differences = vec![0.0_f32; frames.len()];
    let mut max_difference = 0.0_f32;
    for frame in 2..frame_count {
        for pitch in 0..NOTE_BINS {
            let current = frames[index(frame, pitch)];
            let diff_one = current - frames[index(frame - 1, pitch)];
            let diff_two = current - frames[index(frame - 2, pitch)];
            let value = diff_one.min(diff_two).max(0.0);
            differences[index(frame, pitch)] = value;
            max_difference = max_difference.max(value);
        }
    }
    let max_onset = onsets.iter().copied().fold(0.0_f32, f32::max);
    if max_difference > f32::EPSILON && max_onset > 0.0 {
        for (onset, difference) in onsets.iter_mut().zip(differences) {
            *onset = onset.max(max_onset * difference / max_difference);
        }
    }
}

fn extend_forward(
    start: usize,
    pitch: usize,
    remaining: &[f32],
    frame_threshold: f32,
    energy_tolerance: usize,
    frame_count: usize,
) -> Option<FrameNote> {
    if start >= frame_count.saturating_sub(1) {
        return None;
    }
    let mut cursor = start + 1;
    let mut below = 0;
    while cursor < frame_count - 1 && below < energy_tolerance {
        if remaining[index(cursor, pitch)] < frame_threshold {
            below += 1;
        } else {
            below = 0;
        }
        cursor += 1;
    }
    let end = cursor.saturating_sub(below);
    (end > start).then_some(FrameNote {
        start,
        end,
        pitch_index: pitch,
        amplitude: 0.0,
    })
}

fn clear_note_energy(remaining: &mut [f32], note: &FrameNote, frame_count: usize) {
    let pitch_start = note.pitch_index.saturating_sub(1);
    let pitch_end = (note.pitch_index + 1).min(NOTE_BINS - 1);
    for frame in note.start..note.end.min(frame_count) {
        for pitch in pitch_start..=pitch_end {
            remaining[index(frame, pitch)] = 0.0;
        }
    }
}

fn with_amplitude(mut note: FrameNote, original: &[f32]) -> FrameNote {
    let mut sum = 0.0;
    for frame in note.start..note.end {
        sum += original[index(frame, note.pitch_index)];
    }
    note.amplitude = sum / (note.end - note.start).max(1) as f32;
    note
}

fn add_remaining_energy_notes(
    notes: &mut Vec<FrameNote>,
    remaining: &mut [f32],
    original: &[f32],
    frame_count: usize,
    thresholds: ResolvedThresholds,
) {
    let minimum = minimum_frames(thresholds.minimum_note_length_ms);
    let mut energy = BinaryHeap::new();
    for (flat_index, value) in remaining.iter().copied().enumerate() {
        if value > thresholds.frame {
            energy.push(EnergyPeak { value, flat_index });
        }
    }
    while let Some(peak) = energy.pop() {
        if remaining[peak.flat_index] <= thresholds.frame
            || remaining[peak.flat_index].total_cmp(&peak.value) != Ordering::Equal
        {
            continue;
        }
        let middle = peak.flat_index / NOTE_BINS;
        let pitch = peak.flat_index % NOTE_BINS;
        remaining[peak.flat_index] = 0.0;

        let mut cursor = middle + 1;
        let mut below = 0;
        while cursor < frame_count.saturating_sub(1) && below < thresholds.energy_tolerance_frames {
            if remaining[index(cursor, pitch)] < thresholds.frame {
                below += 1;
            } else {
                below = 0;
            }
            clear_pitch_and_neighbors(remaining, cursor, pitch);
            cursor += 1;
        }
        let end = cursor.saturating_sub(1 + below);

        let mut backwards = middle as isize - 1;
        below = 0;
        while backwards > 0 && below < thresholds.energy_tolerance_frames {
            let frame = backwards as usize;
            if remaining[index(frame, pitch)] < thresholds.frame {
                below += 1;
            } else {
                below = 0;
            }
            clear_pitch_and_neighbors(remaining, frame, pitch);
            backwards -= 1;
        }
        let start = (backwards + 1 + below as isize).max(0) as usize;
        if end > start && end - start > minimum {
            notes.push(with_amplitude(
                FrameNote {
                    start,
                    end,
                    pitch_index: pitch,
                    amplitude: 0.0,
                },
                original,
            ));
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EnergyPeak {
    value: f32,
    flat_index: usize,
}

impl PartialEq for EnergyPeak {
    fn eq(&self, other: &Self) -> bool {
        self.value.total_cmp(&other.value) == Ordering::Equal && self.flat_index == other.flat_index
    }
}

impl Eq for EnergyPeak {}

impl PartialOrd for EnergyPeak {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EnergyPeak {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value
            .total_cmp(&other.value)
            .then(self.flat_index.cmp(&other.flat_index))
    }
}

fn clear_pitch_and_neighbors(remaining: &mut [f32], frame: usize, pitch: usize) {
    let pitch_start = pitch.saturating_sub(1);
    let pitch_end = (pitch + 1).min(NOTE_BINS - 1);
    for candidate in pitch_start..=pitch_end {
        remaining[index(frame, candidate)] = 0.0;
    }
}

fn minimum_frames(milliseconds: f32) -> usize {
    ((milliseconds / 1_000.0) * (AUDIO_SAMPLE_RATE as f32 / FFT_HOP as f32)).round() as usize
}

fn model_frame_to_seconds(frame: usize) -> f64 {
    let original = frame as f64 * FFT_HOP as f64 / AUDIO_SAMPLE_RATE as f64;
    let window_number = (frame / MODEL_OUTPUT_FRAMES) as f64;
    let window_offset = (FFT_HOP as f64 / AUDIO_SAMPLE_RATE as f64)
        * (MODEL_OUTPUT_FRAMES as f64 - AUDIO_WINDOW_SAMPLES as f64 / FFT_HOP as f64)
        + 0.0018;
    (original - window_offset * window_number).max(0.0)
}

fn suppress_duplicates(events: &mut Vec<NoteEvent>, maximum_gap_seconds: f64) {
    events.sort_by(|left, right| {
        left.pitch
            .cmp(&right.pitch)
            .then_with(|| left.start_seconds.total_cmp(&right.start_seconds))
    });
    let mut merged: Vec<NoteEvent> = Vec::with_capacity(events.len());
    for event in events.drain(..) {
        if let Some(previous) = merged.last_mut() {
            if previous.pitch == event.pitch
                && (event.start_seconds - previous.start_seconds).abs() <= maximum_gap_seconds
                && event.start_seconds < previous.end_seconds
            {
                previous.end_seconds = previous.end_seconds.max(event.end_seconds);
                previous.velocity = previous.velocity.max(event.velocity);
                previous.confidence = previous.confidence.max(event.confidence);
                continue;
            }
        }
        merged.push(event);
    }
    merged.sort_by(|left, right| {
        left.start_seconds
            .total_cmp(&right.start_seconds)
            .then(left.pitch.cmp(&right.pitch))
    });
    *events = merged;
}

#[inline]
fn index(frame: usize, pitch: usize) -> usize {
    frame * NOTE_BINS + pitch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PianoPreset, CONTOUR_BINS};

    fn blank(frame_count: usize) -> StitchedActivations {
        StitchedActivations {
            frame_count,
            frames: vec![0.0; frame_count * NOTE_BINS],
            onsets: vec![0.0; frame_count * NOTE_BINS],
            contours: vec![0.0; frame_count * CONTOUR_BINS],
        }
    }

    #[test]
    fn extracts_sustained_note_and_rejects_click() {
        let mut value = blank(80);
        let pitch = 39;
        value.onsets[index(10, pitch)] = 0.9;
        for frame in 10..40 {
            value.frames[index(frame, pitch)] = 0.8;
        }
        value.onsets[index(55, pitch + 1)] = 0.95;
        value.frames[index(55, pitch + 1)] = 0.95;

        let notes =
            activations_to_notes(&value, PianoPreset::PianoNoiseReduced.thresholds()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].pitch, MIDI_OFFSET + pitch as u8);
        assert!(notes[0].velocity > 90);
    }

    #[test]
    fn merges_same_pitch_near_duplicates() {
        let mut events = vec![
            NoteEvent {
                start_seconds: 0.0,
                end_seconds: 0.4,
                pitch: 60,
                velocity: 80,
                confidence: 0.6,
            },
            NoteEvent {
                start_seconds: 0.02,
                end_seconds: 0.7,
                pitch: 60,
                velocity: 90,
                confidence: 0.7,
            },
        ];
        suppress_duplicates(&mut events, 0.035);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].end_seconds, 0.7);
        assert_eq!(events[0].velocity, 90);
    }

    #[test]
    fn malformed_activation_is_rejected() {
        let mut value = blank(10);
        value.frames.pop();
        assert!(activations_to_notes(&value, PianoPreset::PianoBalanced.thresholds()).is_err());
    }

    #[test]
    fn official_time_correction_stays_monotonic() {
        assert!(model_frame_to_seconds(172) > model_frame_to_seconds(171));
        assert!((crate::ANNOTATION_FPS as f64 - 86.0).abs() < f64::EPSILON);
    }
}
