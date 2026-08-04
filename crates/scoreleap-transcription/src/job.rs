//! 任务状态机。

use serde::Serialize;

/// 转录任务状态（任务契约）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JobStatus {
    Queued,
    Starting,
    ValidatingInput,
    LoadingModel,
    Transcribing,
    WritingMidi,
    ImportingMidi,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Starting => "Starting",
            Self::ValidatingInput => "ValidatingInput",
            Self::LoadingModel => "LoadingModel",
            Self::Transcribing => "Transcribing",
            Self::WritingMidi => "WritingMidi",
            Self::ImportingMidi => "ImportingMidi",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }

    /// 是否终态。
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// 转录任务视图（对外查询返回）。
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionJob {
    pub job_id: String,
    pub request_id: String,
    pub source_name: String,
    pub status: JobStatus,
    pub stage: String,
    pub message: String,
    pub started_at_ms: u64,
    pub elapsed_ms: i64,
    pub note_count: Option<u64>,
    pub midi_path: Option<String>,
    pub metadata_path: Option<String>,
    pub result_doc_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
