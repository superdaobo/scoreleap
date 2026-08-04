//! scoreleap-transcription：Windows 音频转录服务（管理 Basic Pitch Python Worker）。

mod error;
mod job;
mod protocol;
mod service;

pub use error::{TranscriptionError, TranscriptionErrorCode};
pub use job::{JobStatus, TranscriptionJob};
pub use protocol::WorkerMsg;
pub use service::{TranscriptionEvent, TranscriptionService, WorkerSpec};
