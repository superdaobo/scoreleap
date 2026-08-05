//! ScoreLeap 原生 ONNX 音频转录内核。
//!
//! 该 crate 只处理本地音频、ONNX 推理、音符后处理与 MIDI 输出；
//! ONNX Runtime 动态库由宿主程序在创建 [`Transcriber`] 前显式初始化。

mod config;
mod error;
mod inference;
mod midi;
mod postprocess;
mod window;

pub use config::{PianoPreset, ResolvedThresholds, ThresholdOverrides};
pub use error::{ErrorCode, TranscribeError};
pub use inference::{
    initialize_onnx_runtime, BasicPitchModel, ModelActivations, Transcriber, TranscriptionMetadata,
    TranscriptionResult,
};
pub use midi::write_midi;
pub use postprocess::{activations_to_notes, NoteEvent};
pub use window::{stitch_outputs, AudioWindows, StitchedActivations, WindowActivations};

/// 原生转录协议版本。
pub const PROTOCOL_VERSION: u32 = 1;
pub const AUDIO_SAMPLE_RATE: u32 = 22_050;
pub const FFT_HOP: usize = 256;
pub const ANNOTATION_FPS: usize = 86;
pub const AUDIO_WINDOW_SAMPLES: usize = 43_844;
pub const MODEL_OUTPUT_FRAMES: usize = 172;
pub const OVERLAP_FRAMES: usize = 30;
pub const HALF_OVERLAP_FRAMES: usize = OVERLAP_FRAMES / 2;
pub const OVERLAP_SAMPLES: usize = OVERLAP_FRAMES * FFT_HOP;
pub const AUDIO_WINDOW_HOP: usize = AUDIO_WINDOW_SAMPLES - OVERLAP_SAMPLES;
pub const HALF_OVERLAP_SAMPLES: usize = OVERLAP_SAMPLES / 2;
pub const NOTE_BINS: usize = 88;
pub const CONTOUR_BINS: usize = 264;
pub const MIDI_OFFSET: u8 = 21;
