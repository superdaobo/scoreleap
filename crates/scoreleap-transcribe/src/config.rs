use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::TranscribeError;

/// 面向钢琴独奏优化的预设。阈值可按任务覆盖，但必须先通过严格校验。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PianoPreset {
    #[default]
    PianoBalanced,
    PianoDetail,
    PianoNoiseReduced,
}

impl PianoPreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PianoBalanced => "piano_balanced",
            Self::PianoDetail => "piano_detail",
            Self::PianoNoiseReduced => "piano_noise_reduced",
        }
    }

    pub const fn thresholds(self) -> ResolvedThresholds {
        match self {
            Self::PianoBalanced => ResolvedThresholds {
                onset: 0.50,
                frame: 0.30,
                // 与 Basic Pitch 官方默认值一致，作为可复现的平衡基线。
                minimum_note_length_ms: 127.70,
                energy_tolerance_frames: 11,
                duplicate_gap_ms: 35.0,
                infer_onsets: true,
                melodia_trick: true,
            },
            Self::PianoDetail => ResolvedThresholds {
                onset: 0.42,
                frame: 0.26,
                minimum_note_length_ms: 75.0,
                energy_tolerance_frames: 8,
                duplicate_gap_ms: 25.0,
                infer_onsets: true,
                melodia_trick: true,
            },
            Self::PianoNoiseReduced => ResolvedThresholds {
                onset: 0.58,
                frame: 0.38,
                minimum_note_length_ms: 160.0,
                energy_tolerance_frames: 14,
                duplicate_gap_ms: 55.0,
                infer_onsets: false,
                melodia_trick: false,
            },
        }
    }
}

impl FromStr for PianoPreset {
    type Err = TranscribeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "piano_balanced" => Ok(Self::PianoBalanced),
            "piano_detail" => Ok(Self::PianoDetail),
            "piano_noise_reduced" => Ok(Self::PianoNoiseReduced),
            _ => Err(TranscribeError::InvalidOptions(format!(
                "未知钢琴预设 `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResolvedThresholds {
    pub onset: f32,
    pub frame: f32,
    pub minimum_note_length_ms: f32,
    pub energy_tolerance_frames: usize,
    pub duplicate_gap_ms: f32,
    pub infer_onsets: bool,
    pub melodia_trick: bool,
}

impl ResolvedThresholds {
    pub fn validate(self) -> Result<Self, TranscribeError> {
        validate_probability("onset", self.onset)?;
        validate_probability("frame", self.frame)?;
        if !self.minimum_note_length_ms.is_finite()
            || !(20.0..=2_000.0).contains(&self.minimum_note_length_ms)
        {
            return Err(TranscribeError::InvalidOptions(
                "minimum_note_length_ms 必须在 20..=2000 之间".into(),
            ));
        }
        if !(1..=60).contains(&self.energy_tolerance_frames) {
            return Err(TranscribeError::InvalidOptions(
                "energy_tolerance_frames 必须在 1..=60 之间".into(),
            ));
        }
        if !self.duplicate_gap_ms.is_finite() || !(0.0..=500.0).contains(&self.duplicate_gap_ms) {
            return Err(TranscribeError::InvalidOptions(
                "duplicate_gap_ms 必须在 0..=500 之间".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ThresholdOverrides {
    pub onset: Option<f32>,
    pub frame: Option<f32>,
    pub minimum_note_length_ms: Option<f32>,
    pub energy_tolerance_frames: Option<usize>,
    pub duplicate_gap_ms: Option<f32>,
}

impl ThresholdOverrides {
    pub fn resolve(self, preset: PianoPreset) -> Result<ResolvedThresholds, TranscribeError> {
        let mut value = preset.thresholds();
        if let Some(v) = self.onset {
            value.onset = v;
        }
        if let Some(v) = self.frame {
            value.frame = v;
        }
        if let Some(v) = self.minimum_note_length_ms {
            value.minimum_note_length_ms = v;
        }
        if let Some(v) = self.energy_tolerance_frames {
            value.energy_tolerance_frames = v;
        }
        if let Some(v) = self.duplicate_gap_ms {
            value.duplicate_gap_ms = v;
        }
        value.validate()
    }
}

fn validate_probability(name: &str, value: f32) -> Result<(), TranscribeError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(TranscribeError::InvalidOptions(format!(
            "{name} 必须是 0..=1 的有限数"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_expected_noise_ordering() {
        let detail = PianoPreset::PianoDetail.thresholds();
        let balanced = PianoPreset::PianoBalanced.thresholds();
        let reduced = PianoPreset::PianoNoiseReduced.thresholds();
        assert!(detail.onset < balanced.onset && balanced.onset < reduced.onset);
        assert!(detail.minimum_note_length_ms < reduced.minimum_note_length_ms);
    }

    #[test]
    fn override_is_applied_and_validated() {
        let thresholds = ThresholdOverrides {
            onset: Some(0.61),
            ..ThresholdOverrides::default()
        }
        .resolve(PianoPreset::PianoBalanced)
        .unwrap();
        assert_eq!(thresholds.onset, 0.61);

        assert!(ThresholdOverrides {
            frame: Some(f32::NAN),
            ..ThresholdOverrides::default()
        }
        .resolve(PianoPreset::PianoBalanced)
        .is_err());
    }
}
