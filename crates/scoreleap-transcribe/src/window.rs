use crate::{
    TranscribeError, ANNOTATION_FPS, AUDIO_SAMPLE_RATE, AUDIO_WINDOW_HOP, AUDIO_WINDOW_SAMPLES,
    CONTOUR_BINS, HALF_OVERLAP_FRAMES, HALF_OVERLAP_SAMPLES, MODEL_OUTPUT_FRAMES, NOTE_BINS,
};

/// 单个模型窗口的三个输出，均为 time-major 连续数组。
#[derive(Debug, Clone, PartialEq)]
pub struct WindowActivations {
    pub frames: Vec<f32>,
    pub onsets: Vec<f32>,
    pub contours: Vec<f32>,
}

/// 拼接后的完整激活矩阵。
#[derive(Debug, Clone, PartialEq)]
pub struct StitchedActivations {
    pub frame_count: usize,
    pub frames: Vec<f32>,
    pub onsets: Vec<f32>,
    pub contours: Vec<f32>,
}

/// Basic Pitch 窗口迭代器。它按需构造单个窗口，避免复制十分钟音频的全部窗口。
pub struct AudioWindows<'a> {
    samples: &'a [f32],
    padded_offset: usize,
    next_start: usize,
}

impl<'a> AudioWindows<'a> {
    pub fn new(samples: &'a [f32]) -> Self {
        Self {
            samples,
            padded_offset: HALF_OVERLAP_SAMPLES,
            next_start: 0,
        }
    }

    pub fn len(&self) -> usize {
        let padded_len = self.samples.len() + self.padded_offset;
        padded_len.div_ceil(AUDIO_WINDOW_HOP)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

impl Iterator for AudioWindows<'_> {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        let padded_len = self.samples.len() + self.padded_offset;
        if self.next_start >= padded_len {
            return None;
        }

        let start = self.next_start;
        self.next_start += AUDIO_WINDOW_HOP;
        let mut window = vec![0.0; AUDIO_WINDOW_SAMPLES];
        for (index, value) in window.iter_mut().enumerate() {
            let padded_index = start + index;
            if padded_index >= self.padded_offset {
                if let Some(sample) = self.samples.get(padded_index - self.padded_offset) {
                    *value = *sample;
                }
            }
        }
        Some(window)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let total = self.len();
        let consumed = self.next_start.div_ceil(AUDIO_WINDOW_HOP);
        let remaining = total.saturating_sub(consumed);
        (remaining, Some(remaining))
    }
}

/// 严格复现官方 `unwrap_output`：每窗去掉前后 15 帧，再裁到原音频理论帧数。
pub fn stitch_outputs(
    windows: &[WindowActivations],
    original_samples: usize,
) -> Result<StitchedActivations, TranscribeError> {
    if windows.is_empty() {
        return Err(TranscribeError::InvalidOutput(
            "模型没有返回任何窗口".into(),
        ));
    }
    let expected_note_values = MODEL_OUTPUT_FRAMES * NOTE_BINS;
    let expected_contour_values = MODEL_OUTPUT_FRAMES * CONTOUR_BINS;
    let kept_frames = MODEL_OUTPUT_FRAMES - 2 * HALF_OVERLAP_FRAMES;
    let target_frames =
        original_samples.saturating_mul(ANNOTATION_FPS) / AUDIO_SAMPLE_RATE as usize;

    let mut frames = Vec::with_capacity(windows.len() * kept_frames * NOTE_BINS);
    let mut onsets = Vec::with_capacity(windows.len() * kept_frames * NOTE_BINS);
    let mut contours = Vec::with_capacity(windows.len() * kept_frames * CONTOUR_BINS);
    for (index, window) in windows.iter().enumerate() {
        if window.frames.len() != expected_note_values
            || window.onsets.len() != expected_note_values
            || window.contours.len() != expected_contour_values
        {
            return Err(TranscribeError::InvalidOutput(format!(
                "第 {index} 个窗口张量尺寸错误: note={}, onset={}, contour={}",
                window.frames.len(),
                window.onsets.len(),
                window.contours.len()
            )));
        }
        let note_start = HALF_OVERLAP_FRAMES * NOTE_BINS;
        let note_end = (MODEL_OUTPUT_FRAMES - HALF_OVERLAP_FRAMES) * NOTE_BINS;
        let contour_start = HALF_OVERLAP_FRAMES * CONTOUR_BINS;
        let contour_end = (MODEL_OUTPUT_FRAMES - HALF_OVERLAP_FRAMES) * CONTOUR_BINS;
        frames.extend_from_slice(&window.frames[note_start..note_end]);
        onsets.extend_from_slice(&window.onsets[note_start..note_end]);
        contours.extend_from_slice(&window.contours[contour_start..contour_end]);
    }

    let available_frames = frames.len() / NOTE_BINS;
    let frame_count = target_frames.min(available_frames);
    frames.truncate(frame_count * NOTE_BINS);
    onsets.truncate(frame_count * NOTE_BINS);
    contours.truncate(frame_count * CONTOUR_BINS);
    Ok(StitchedActivations {
        frame_count,
        frames,
        onsets,
        contours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(marker: f32) -> WindowActivations {
        WindowActivations {
            frames: (0..MODEL_OUTPUT_FRAMES)
                .flat_map(|frame| vec![marker + frame as f32; NOTE_BINS])
                .collect(),
            onsets: vec![marker; MODEL_OUTPUT_FRAMES * NOTE_BINS],
            contours: vec![marker; MODEL_OUTPUT_FRAMES * CONTOUR_BINS],
        }
    }

    #[test]
    fn windows_match_basic_pitch_padding_and_hop() {
        let samples: Vec<f32> = (0..AUDIO_WINDOW_SAMPLES).map(|v| v as f32).collect();
        let windows: Vec<_> = AudioWindows::new(&samples).collect();
        assert_eq!(windows.len(), 2);
        assert!(windows[0][..HALF_OVERLAP_SAMPLES].iter().all(|v| *v == 0.0));
        assert_eq!(windows[0][HALF_OVERLAP_SAMPLES], 0.0);
        assert_eq!(windows[0][HALF_OVERLAP_SAMPLES + 1], 1.0);
        assert_eq!(
            windows[1][0],
            samples[AUDIO_WINDOW_HOP - HALF_OVERLAP_SAMPLES]
        );
    }

    #[test]
    fn stitch_drops_both_overlap_halves_and_trims() {
        let original_samples = AUDIO_SAMPLE_RATE as usize * 3;
        let stitched = stitch_outputs(&[output(0.0), output(1_000.0)], original_samples).unwrap();
        assert_eq!(stitched.frame_count, 258);
        assert_eq!(stitched.frames[0], HALF_OVERLAP_FRAMES as f32);
        assert_eq!(stitched.frames[141 * NOTE_BINS], 156.0);
        assert_eq!(stitched.frames[142 * NOTE_BINS], 1_015.0);
        assert_eq!(stitched.frames.len(), 258 * NOTE_BINS);
    }

    #[test]
    fn stitch_rejects_malformed_tensor() {
        let mut malformed = output(0.0);
        malformed.onsets.pop();
        assert!(stitch_outputs(&[malformed], AUDIO_SAMPLE_RATE as usize).is_err());
    }
}
