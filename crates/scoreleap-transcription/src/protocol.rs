//! Worker 协议：JSON Lines 解析（未知字段忽略，向前兼容）与消息处理。

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use serde::Deserialize;

use crate::error::TranscriptionErrorCode;
use crate::job::JobStatus;
use crate::service::{ActiveJob, EventFn, TranscriptionEvent};

/// Worker stdout 单行消息（schema_version=1）。
/// 所有字段可选：解析器只读取关心的字段，未知字段由 serde 默认忽略。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerMsg {
    pub schema_version: Option<i32>,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub request_id: Option<String>,
    pub timestamp_ms: Option<i64>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub midi_path: Option<String>,
    #[serde(default)]
    pub metadata_path: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<i64>,
    #[serde(default)]
    pub note_count: Option<u64>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub worker_version: Option<String>,
}

impl WorkerMsg {
    pub fn parse_line(line: &str) -> Result<WorkerMsg, serde_json::Error> {
        serde_json::from_str(line)
    }
}

/// 退出码 → 结构化错误码（Worker 契约）。
pub(crate) fn map_exit_code(
    code: i32,
    last_code: &Option<String>,
    last_msg: &Option<String>,
) -> (String, String) {
    // Worker 已通过 JSONL 给出结构化错误时，以协议内容为准，避免退出码降级信息。
    if let Some(worker_code) = last_code {
        return (
            worker_code.clone(),
            last_msg
                .clone()
                .unwrap_or_else(|| "转录组件返回错误".into()),
        );
    }
    let mapped = match code {
        2 => Some((
            TranscriptionErrorCode::WorkerProtocolError,
            "Worker 参数错误".into(),
        )),
        3 => Some((
            TranscriptionErrorCode::InvalidAudioPath,
            "Worker 输入错误".into(),
        )),
        4 => Some((
            TranscriptionErrorCode::AudioDecodeFailed,
            "音频解码失败".into(),
        )),
        5 => Some((
            TranscriptionErrorCode::ModelLoadFailed,
            "模型加载失败".into(),
        )),
        6 => Some((
            TranscriptionErrorCode::InferenceFailed,
            "音符识别失败".into(),
        )),
        7 => Some((
            TranscriptionErrorCode::MidiWriteFailed,
            "MIDI 写入失败".into(),
        )),
        8 => Some((TranscriptionErrorCode::JobCancelled, "任务已取消".into())),
        9 => Some((
            TranscriptionErrorCode::InternalError,
            "Worker 内部错误".into(),
        )),
        _ => None,
    };
    mapped
        .map(|(code, message)| (code.as_str().into(), message))
        .unwrap_or_else(|| {
            (
                TranscriptionErrorCode::WorkerExitedUnexpectedly
                    .as_str()
                    .into(),
                format!("Worker 异常退出（退出码 {code}）"),
            )
        })
}

/// Worker 消息处理的上下文（同一 job 的共享状态引用）。
pub(crate) struct WorkerMessageContext<'a> {
    pub inner: &'a Mutex<Option<ActiveJob>>,
    pub on_event: &'a EventFn,
    pub last_code: &'a Mutex<Option<String>>,
    pub last_msg: &'a Mutex<Option<String>>,
    pub saw_result: &'a AtomicBool,
    pub job_id: &'a str,
    pub expected_request_id: &'a str,
}

/// 处理一条 Worker 消息（ready/stage/result/error；未知类型忽略）。
pub(crate) fn handle_worker_msg(context: &WorkerMessageContext<'_>, msg: WorkerMsg) {
    if msg.schema_version != Some(1)
        || msg.request_id.as_deref() != Some(context.expected_request_id)
    {
        *context.last_code.lock().unwrap() =
            Some(TranscriptionErrorCode::WorkerProtocolError.as_str().into());
        *context.last_msg.lock().unwrap() = Some("Worker schema_version 或 request_id 无效".into());
        return;
    }
    match msg.msg_type.as_str() {
        "ready" => {
            if let Some(v) = msg.worker_version {
                let mut guard = context.inner.lock().unwrap();
                if let Some(a) = guard.as_mut() {
                    if a.job.job_id == context.job_id {
                        a.job.message = format!("Worker {v} 就绪");
                    }
                }
            }
        }
        "stage" => {
            let stage = msg.stage.unwrap_or_default();
            let message = msg.message.unwrap_or_default();
            let status = match stage.as_str() {
                "validating_input" => JobStatus::ValidatingInput,
                "loading_model" => JobStatus::LoadingModel,
                "transcribing" => JobStatus::Transcribing,
                "writing_midi" => JobStatus::WritingMidi,
                _ => JobStatus::Starting,
            };
            {
                let mut guard = context.inner.lock().unwrap();
                if let Some(a) = guard.as_mut() {
                    if a.job.job_id == context.job_id {
                        a.job.status = status;
                        a.job.stage = stage.clone();
                        a.job.message = message.clone();
                    }
                }
            }
            (context.on_event)(TranscriptionEvent::Stage {
                job_id: context.job_id.into(),
                stage,
                message,
            });
        }
        "result" => {
            let mut guard = context.inner.lock().unwrap();
            if let Some(a) = guard.as_mut() {
                if a.job.job_id == context.job_id {
                    let paths_match = msg.midi_path.as_deref() == a.job.midi_path.as_deref()
                        && msg.metadata_path.as_deref() == a.job.metadata_path.as_deref();
                    if !paths_match || msg.elapsed_ms.is_none() || msg.note_count.is_none() {
                        *context.last_code.lock().unwrap() =
                            Some(TranscriptionErrorCode::WorkerProtocolError.as_str().into());
                        *context.last_msg.lock().unwrap() =
                            Some("Worker result 字段缺失或输出路径不匹配".into());
                        return;
                    }
                    a.job.note_count = msg.note_count;
                    a.job.elapsed_ms = msg.elapsed_ms.unwrap_or(0);
                    context
                        .saw_result
                        .store(true, std::sync::atomic::Ordering::Release);
                }
            }
        }
        "error" => {
            *context.last_code.lock().unwrap() = Some(
                msg.code
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| TranscriptionErrorCode::WorkerProtocolError.as_str().into()),
            );
            *context.last_msg.lock().unwrap() = msg
                .message
                .clone()
                .or(msg.detail.clone())
                .or_else(|| Some("Worker 返回了未说明的错误".into()));
            tracing::warn!(
                job_id = context.job_id,
                "worker-error: {:?} {:?}",
                msg.code,
                msg.message
            );
        }
        other => {
            tracing::debug!(job_id = context.job_id, "忽略未知 Worker 消息类型: {other}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ready() {
        let m = WorkerMsg::parse_line(
            r#"{"schema_version":1,"type":"ready","request_id":"r1","timestamp_ms":1,"worker_version":"0.1.0"}"#,
        )
        .unwrap();
        assert_eq!(m.msg_type, "ready");
        assert_eq!(m.request_id.as_deref(), Some("r1"));
        assert_eq!(m.worker_version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn parses_stage() {
        let m = WorkerMsg::parse_line(
            r#"{"type":"stage","request_id":"r1","stage":"transcribing","message":"正在识别音符"}"#,
        )
        .unwrap();
        assert_eq!(m.stage.as_deref(), Some("transcribing"));
        assert!(m.message.as_deref().unwrap().contains("识别"));
    }

    #[test]
    fn parses_result() {
        let m = WorkerMsg::parse_line(
            r#"{"type":"result","midi_path":"a.mid","metadata_path":"b.json","elapsed_ms":123,"note_count":7}"#,
        )
        .unwrap();
        assert_eq!(m.note_count, Some(7));
        assert_eq!(m.elapsed_ms, Some(123));
    }

    #[test]
    fn ignores_unknown_fields() {
        let m = WorkerMsg::parse_line(
            r#"{"type":"future_type","request_id":"r1","future_field":{"x":1},"stage":"x"}"#,
        )
        .unwrap();
        assert_eq!(m.msg_type, "future_type");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(WorkerMsg::parse_line("not json").is_err());
        assert!(WorkerMsg::parse_line("").is_err());
    }
}
