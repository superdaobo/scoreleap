"""CLI 测试：参数、输入验证、错误码、退出码、stdout 纯净（FakePredictor）。"""

import io
import json
import os
import sys

import pytest

from scoreleap_transcriber.cli import main
from scoreleap_transcriber.errors import (
    EXIT_ARGS,
    EXIT_INPUT,
    EXIT_SUCCESS,
)

FAKE_ENV = {"SCORELEAP_FAKE_PREDICTOR": "1"}


@pytest.fixture()
def workdir(tmp_path):
    src = tmp_path / "in.mp3"
    src.write_bytes(b"\x00" * 1024)  # 假 MP3（FakePredictor 不解码）
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    return tmp_path, str(src), str(out_dir / "generated.mid"), str(out_dir / "metadata.json")


def run_main(argv, env=None):
    """捕获 stdout 运行 main（注入 fake predictor 环境变量）。"""
    import importlib

    for mod in list(sys.modules):
        if mod.startswith("scoreleap_transcriber"):
            del sys.modules[mod]
    old_environ = dict(os.environ)
    old_stdout = sys.stdout
    if env:
        os.environ.update(env)
    buf = io.StringIO()
    try:
        sys.stdout = buf
        from scoreleap_transcriber import cli as _cli

        code = _cli.main(argv)
    finally:
        sys.stdout = old_stdout
        os.environ.clear()
        os.environ.update(old_environ)
    return code, buf.getvalue()


def test_success_flow(workdir):
    _, src, midi, meta = workdir
    code, out = run_main(
        ["transcribe", "--request-id", "req-1", "--input", src, "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_SUCCESS
    lines = [json.loads(l) for l in out.strip().splitlines()]
    types = [l["type"] for l in lines]
    assert types == ["ready", "stage", "stage", "stage", "result"]
    assert lines[-1]["note_count"] == 3
    assert os.path.exists(midi) and os.path.getsize(midi) > 0
    assert os.path.exists(meta)


def test_stdout_contains_only_json_lines(workdir):
    _, src, midi, meta = workdir
    code, out = run_main(
        ["transcribe", "--request-id", "r", "--input", src, "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_SUCCESS
    for line in out.strip().splitlines():
        json.loads(line)  # 任何非 JSON 行都会抛异常


def test_missing_input(workdir):
    _, _, midi, meta = workdir
    code, out = run_main(
        ["transcribe", "--request-id", "r", "--input", "C:/nope.mp3", "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_INPUT
    err = json.loads(out.strip().splitlines()[-1])
    assert err["type"] == "error"
    assert err["code"] == "INVALID_AUDIO_PATH"


def test_unsupported_extension(workdir):
    tmp, _, midi, meta = workdir
    wav = tmp / "a.wav"
    wav.write_bytes(b"x")
    code, out = run_main(
        ["transcribe", "--request-id", "r", "--input", str(wav), "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_INPUT
    err = json.loads(out.strip().splitlines()[-1])
    assert err["code"] == "UNSUPPORTED_AUDIO_FORMAT"


def test_empty_input(workdir):
    tmp, _, midi, meta = workdir
    empty = tmp / "empty.mp3"
    empty.write_bytes(b"")
    code, out = run_main(
        ["transcribe", "--request-id", "r", "--input", str(empty), "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_INPUT


def test_output_exists_rejected(workdir):
    _, src, midi, meta = workdir
    with open(midi, "wb") as f:
        f.write(b"existing")
    code, out = run_main(
        ["transcribe", "--request-id", "r", "--input", src, "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_ARGS
    err = json.loads(out.strip().splitlines()[-1])
    assert err["code"] == "INVALID_ARGS"


def test_metadata_written(workdir):
    _, src, midi, meta = workdir
    code, out = run_main(
        ["transcribe", "--request-id", "req-meta", "--input", src, "--output-midi", midi, "--output-metadata", meta],
        FAKE_ENV,
    )
    assert code == EXIT_SUCCESS
    with open(meta, encoding="utf-8") as f:
        m = json.load(f)
    assert m["source_type"] == "audio_transcription"
    assert m["request_id"] == "req-meta"
    assert m["source"]["file_name"] == "in.mp3"
    assert m["source"]["size_bytes"] == 1024
    assert len(m["source"]["sha256"]) == 64
    assert m["transcriber"]["engine"] == "basic-pitch"
    assert m["result"]["note_count"] == 3
    assert "warnings" in m


def test_missing_required_arg():
    # argparse 缺参时抛 SystemExit(2)（= EXIT_ARGS）
    with pytest.raises(SystemExit) as exc:
        run_main([], FAKE_ENV)
    assert exc.value.code == EXIT_ARGS

