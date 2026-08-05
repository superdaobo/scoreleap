//! ScoreLeap 跨平台音频解码、下混与重采样。

mod decode;
mod error;
mod types;

pub use decode::{decode_file, probe_file};
pub use error::AudioError;
pub use types::{AudioConfig, AudioInfo, DecodedAudio};
