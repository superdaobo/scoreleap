"""协议层测试：JSON Lines 格式与解析。"""

import io
import json

from scoreleap_transcriber.protocol import MessageWriter, parse_line, SCHEMA_VERSION


def test_writer_emits_valid_json_lines():
    buf = io.StringIO()
    w = MessageWriter(buf)
    w.ready("req-1")
    w.stage("req-1", "loading_model", "正在加载本地模型")
    w.result("req-1", "a.mid", "b.json", 1234, 5)
    w.error("req-1", "AUDIO_DECODE_FAILED", "无法解码音频", "detail")
    lines = buf.getvalue().strip().splitlines()
    assert len(lines) == 4
    for line in lines:
        obj = json.loads(line)  # 每行必须独立可解析
        assert obj["schema_version"] == SCHEMA_VERSION
        assert obj["request_id"] == "req-1"
        assert "timestamp_ms" in obj


def test_ready_message_fields():
    buf = io.StringIO()
    MessageWriter(buf).ready("req-9")
    obj = json.loads(buf.getvalue())
    assert obj["type"] == "ready"
    assert obj["worker_version"]


def test_stage_message_fields():
    buf = io.StringIO()
    MessageWriter(buf).stage("r", "transcribing", "正在识别音符")
    obj = json.loads(buf.getvalue())
    assert obj["type"] == "stage"
    assert obj["stage"] == "transcribing"
    assert obj["message"] == "正在识别音符"


def test_error_message_optional_detail():
    buf = io.StringIO()
    MessageWriter(buf).error("r", "MODEL_LOAD_FAILED", "模型加载失败")
    obj = json.loads(buf.getvalue())
    assert obj["type"] == "error"
    assert "detail" not in obj
    buf2 = io.StringIO()
    MessageWriter(buf2).error("r", "MODEL_LOAD_FAILED", "模型加载失败", "x")
    assert json.loads(buf2.getvalue())["detail"] == "x"


def test_parse_line_rejects_invalid_json():
    assert parse_line("not json") is None
    assert parse_line("") is None


def test_parse_line_roundtrip():
    buf = io.StringIO()
    MessageWriter(buf).stage("r", "writing_midi", "正在生成 MIDI")
    obj = parse_line(buf.getvalue().strip())
    assert obj is not None
    assert obj["stage"] == "writing_midi"


def test_unknown_fields_preserved_for_forward_compat():
    buf = io.StringIO()
    w = MessageWriter(buf)
    w.write({"type": "future", "request_id": "r", "schema_version": 1, "timestamp_ms": 0, "extra": 42})
    obj = json.loads(buf.getvalue())
    assert obj["extra"] == 42  # 未知字段必须保留（Rust 端忽略而非报错）
