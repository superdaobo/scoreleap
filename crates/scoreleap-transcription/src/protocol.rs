//! Worker 协议：JSON Lines 解析（未知字段忽略，向前兼容）。

use serde::Deserialize;

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
