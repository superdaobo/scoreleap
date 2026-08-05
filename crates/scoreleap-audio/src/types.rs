use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_TARGET_SAMPLE_RATE: u32 = 22_050;
pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 200 * 1024 * 1024;
pub const DEFAULT_MAX_DURATION_SECONDS: f64 = 10.0 * 60.0;

/// 音频处理的资源边界与目标采样率。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub target_sample_rate: u32,
    pub max_file_size_bytes: u64,
    pub max_duration_seconds: f64,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: DEFAULT_TARGET_SAMPLE_RATE,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
            max_duration_seconds: DEFAULT_MAX_DURATION_SECONDS,
        }
    }
}

/// 输入音频的源格式信息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioInfo {
    pub path: PathBuf,
    pub format: String,
    pub file_size_bytes: u64,
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_seconds: f64,
}

/// 已下混并重采样的单声道音频。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub source: AudioInfo,
}

impl DecodedAudio {
    pub fn duration_seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.sample_rate)
    }
}
