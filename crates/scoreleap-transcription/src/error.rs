//! 转录错误：结构化错误码（与 Worker 端约定一致）。

use serde::Serialize;

/// 结构化错误码（任务契约 16 项）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TranscriptionErrorCode {
    WorkerNotFound,
    WorkerStartFailed,
    EngineUnavailable,
    TranscriptionBusy,
    InvalidAudioPath,
    UnsupportedAudioFormat,
    AudioFileTooLarge,
    AudioTooLong,
    AudioDecodeFailed,
    ModelMissing,
    ModelDownloadRequired,
    ModelLoadFailed,
    RuntimeMissing,
    RuntimeLoadFailed,
    InferenceFailed,
    MidiWriteFailed,
    MidiValidationFailed,
    JobCancelled,
    WorkerProtocolError,
    WorkerExitedUnexpectedly,
    InternalError,
}

impl std::fmt::Display for TranscriptionErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TranscriptionErrorCode {
    /// 稳定的字符串编码（与 Worker JSON error.code 对齐）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkerNotFound => "WORKER_NOT_FOUND",
            Self::WorkerStartFailed => "WORKER_START_FAILED",
            Self::EngineUnavailable => "ENGINE_UNAVAILABLE",
            Self::TranscriptionBusy => "TRANSCRIPTION_BUSY",
            Self::InvalidAudioPath => "INVALID_AUDIO_PATH",
            Self::UnsupportedAudioFormat => "UNSUPPORTED_AUDIO_FORMAT",
            Self::AudioFileTooLarge => "AUDIO_FILE_TOO_LARGE",
            Self::AudioTooLong => "AUDIO_TOO_LONG",
            Self::AudioDecodeFailed => "AUDIO_DECODE_FAILED",
            Self::ModelMissing => "MODEL_MISSING",
            Self::ModelDownloadRequired => "MODEL_DOWNLOAD_REQUIRED",
            Self::ModelLoadFailed => "MODEL_LOAD_FAILED",
            Self::RuntimeMissing => "RUNTIME_MISSING",
            Self::RuntimeLoadFailed => "RUNTIME_LOAD_FAILED",
            Self::InferenceFailed => "INFERENCE_FAILED",
            Self::MidiWriteFailed => "MIDI_WRITE_FAILED",
            Self::MidiValidationFailed => "MIDI_VALIDATION_FAILED",
            Self::JobCancelled => "JOB_CANCELLED",
            Self::WorkerProtocolError => "WORKER_PROTOCOL_ERROR",
            Self::WorkerExitedUnexpectedly => "WORKER_EXITED_UNEXPECTEDLY",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

/// 转录错误（Serialize 供 Tauri 命令返回）。
#[derive(Debug, Clone, serde::Serialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct TranscriptionError {
    pub code: TranscriptionErrorCode,
    pub message: String,
}

impl TranscriptionError {
    pub fn new(code: TranscriptionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
