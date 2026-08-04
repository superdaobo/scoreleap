"""metadata 构建测试。"""

import hashlib

from scoreleap_transcriber.metadata import build_metadata, sha256_of


def test_sha256_matches_hashlib(tmp_path):
    f = tmp_path / "a.mp3"
    f.write_bytes(b"hello")
    assert sha256_of(str(f)) == hashlib.sha256(b"hello").hexdigest()


def test_metadata_fields(tmp_path):
    src = tmp_path / "song.mp3"
    src.write_bytes(b"data")
    m = build_metadata(
        request_id="req-1",
        source_path=str(src),
        source_size_bytes=4,
        source_duration_ms=15000,
        engine_version="0.4.0",
        worker_version="0.1.0",
        midi_file="generated.mid",
        note_count=42,
        elapsed_ms=10000,
        warnings=["w"],
    )
    assert m["schema_version"] == 1
    assert m["source_type"] == "audio_transcription"
    # 隐私：不包含绝对路径
    assert "source_path" not in m
    assert m["source"]["file_name"] == "song.mp3"
    assert m["source"]["sha256"] == sha256_of(str(src))
    assert m["transcriber"]["engine_version"] == "0.4.0"
    assert m["result"]["midi_file"] == "generated.mid"
    assert m["warnings"] == ["w"]
