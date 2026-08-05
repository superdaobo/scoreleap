//! scoreleap-transcription：管理 ScoreLeap 原生 ONNX 转录 sidecar。

mod error;
mod job;
mod protocol;
mod service;

pub use error::{TranscriptionError, TranscriptionErrorCode};
pub use job::{JobStatus, TranscriptionJob};
pub use protocol::WorkerMsg;
pub use service::{
    TranscriptionEvent, TranscriptionOptions, TranscriptionPreset, TranscriptionService,
    WorkerSpec, ALLOWED_EXTENSIONS, MAX_FILE_BYTES,
};
