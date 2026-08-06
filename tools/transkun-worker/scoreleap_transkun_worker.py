"""ScoreLeap 高质量钢琴转录 Worker。

运行时由 PyInstaller onedir 完整封装；最终用户不需要安装 Python、PyTorch、
CUDA、ffmpeg 或其他系统组件。stdout 只输出 JSON Lines 协议，依赖库日志重定向到 stderr。
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.resources
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import time
import traceback
import uuid
from dataclasses import dataclass
from typing import Any, NoReturn, Sequence

SCHEMA_VERSION = 1
WORKER_VERSION = "1.0.0"
TARGET_SAMPLE_RATE = 44_100
MAX_FILE_BYTES = 200 * 1024 * 1024
MAX_DURATION_SECONDS = 600.0
ALLOWED_EXTENSIONS = {".mp3", ".wav", ".flac"}


@dataclass(slots=True)
class WorkerFailure(Exception):
    code: str
    message: str
    exit_code: int

    def __str__(self) -> str:
        return self.message


def configure_console() -> None:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8", errors="replace", line_buffering=True)


def now_ms() -> int:
    return int(time.time() * 1000)


def emit(message_type: str, request_id: str, **payload: Any) -> None:
    message = {
        "schema_version": SCHEMA_VERSION,
        "type": message_type,
        "request_id": request_id,
        "timestamp_ms": now_ms(),
        **payload,
    }
    sys.stdout.write(json.dumps(message, ensure_ascii=False, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def emit_stage(request_id: str, stage: str, message: str, progress: float) -> None:
    emit(
        "stage",
        request_id,
        stage=stage,
        message=message,
        progress=max(0.0, min(1.0, float(progress))),
    )


def fail(code: str, message: str, exit_code: int) -> NoReturn:
    raise WorkerFailure(code, message, exit_code)


def parse_transcribe_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="scoreleap-transkun-worker")
    parser.add_argument("command", choices=["transcribe"])
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output-midi", required=True)
    parser.add_argument("--output-metadata", required=True)
    # 与 Rust 服务的公共 Worker 协议兼容。Transkun 自包含模型，不使用前两个参数。
    parser.add_argument("--model")
    parser.add_argument("--onnx-runtime")
    parser.add_argument("--preset", default="piano_balanced")
    parser.add_argument("--onset-threshold", type=float)
    parser.add_argument("--frame-threshold", type=float)
    parser.add_argument("--minimum-note-length-ms", type=float)
    return parser.parse_args(list(argv))


def validate_input(path: Path) -> None:
    if not path.is_file():
        fail("INVALID_AUDIO_PATH", f"音频不存在或不是普通文件: {path}", 3)
    if path.suffix.lower() not in ALLOWED_EXTENSIONS:
        fail("UNSUPPORTED_AUDIO_FORMAT", "仅支持 MP3、WAV 或 FLAC", 3)
    size = path.stat().st_size
    if size <= 0:
        fail("INVALID_AUDIO_PATH", "音频文件为空", 3)
    if size > MAX_FILE_BYTES:
        fail("AUDIO_FILE_TOO_LARGE", "音频文件超过 200MB 上限", 3)


def decode_audio(path: Path) -> tuple[Any, float]:
    try:
        with contextlib.redirect_stdout(sys.stderr):
            import miniaudio
            import numpy as np

            decoded = miniaudio.decode_file(
                str(path),
                output_format=miniaudio.SampleFormat.FLOAT32,
                nchannels=1,
                sample_rate=TARGET_SAMPLE_RATE,
            )
            audio = np.asarray(decoded.samples, dtype=np.float32).reshape(-1, 1)
    except Exception as error:  # noqa: BLE001 - 转换为稳定 Worker 错误契约
        fail("AUDIO_DECODE_FAILED", f"音频解码失败: {error}", 4)

    if audio.size == 0:
        fail("AUDIO_DECODE_FAILED", "音频没有可用采样", 4)
    if not bool(np.isfinite(audio).all()):
        fail("AUDIO_DECODE_FAILED", "音频包含 NaN 或无穷值", 4)
    duration_seconds = float(audio.shape[0]) / TARGET_SAMPLE_RATE
    if duration_seconds > MAX_DURATION_SECONDS:
        fail("AUDIO_TOO_LONG", "音频超过 10 分钟上限", 3)
    return audio, duration_seconds


def bundled_model_paths() -> tuple[Path, Path]:
    try:
        package_root = importlib.resources.files("transkun")
        weight = package_root.joinpath("pretrained", "2.0.pt")
        config = package_root.joinpath("pretrained", "2.0.conf")
        # PyInstaller onedir 中 Traversable 对应真实文件；开发环境亦是普通 Path。
        weight_path = Path(str(weight))
        config_path = Path(str(config))
    except Exception as error:  # noqa: BLE001
        fail("MODEL_MISSING", f"无法定位 Transkun 内置模型: {error}", 5)
    if not weight_path.is_file() or not config_path.is_file():
        fail("MODEL_MISSING", "安装包缺少 Transkun 2.0 模型或配置", 5)
    return weight_path, config_path


def load_model() -> tuple[Any, Any, Any, str]:
    weight_path, config_path = bundled_model_paths()
    try:
        # 部分上游模块会打印诊断信息；stdout 必须保持纯 JSONL。
        with contextlib.redirect_stdout(sys.stderr):
            import moduleconf
            import torch
            import transkun.ModelTransformer  # noqa: F401 - 供 moduleconf 动态导入
            from transkun.Data import writeMidi

            torch.set_num_threads(max(1, min(4, os.cpu_count() or 1)))
            torch.set_grad_enabled(False)
            conf_manager = moduleconf.parseFromFile(str(config_path))
            transkun_class = conf_manager["Model"].module.TransKun
            conf = conf_manager["Model"].config
            try:
                checkpoint = torch.load(
                    str(weight_path),
                    map_location="cpu",
                    weights_only=False,
                )
            except TypeError:
                # 兼容 PyTorch 2.5 及更旧版本；正式打包固定使用支持 weights_only 的版本。
                checkpoint = torch.load(str(weight_path), map_location="cpu")
            model = transkun_class(conf=conf).to("cpu")
            state = checkpoint.get("best_state_dict", checkpoint.get("state_dict"))
            if state is None:
                fail("MODEL_LOAD_FAILED", "Transkun checkpoint 缺少 state_dict", 5)
            model.load_state_dict(state, strict=False)
            model.eval()
    except WorkerFailure:
        raise
    except Exception as error:  # noqa: BLE001
        fail("MODEL_LOAD_FAILED", f"Transkun 模型加载失败: {error}", 5)
    return torch, model, writeMidi, weight_path.name


def clean_notes(notes: Sequence[Any], minimum_note_length_ms: float | None) -> list[Any]:
    minimum_seconds = max(0.0, float(minimum_note_length_ms or 0.0) / 1000.0)
    cleaned: list[Any] = []
    for note in notes:
        pitch = int(getattr(note, "pitch", -1))
        start = float(getattr(note, "start", math.nan))
        end = float(getattr(note, "end", math.nan))
        velocity = int(round(float(getattr(note, "velocity", 0))))
        # 游戏曲谱只保留 A0-C8 钢琴音符；踏板等负 pitch 控制事件不写入结果。
        if not 21 <= pitch <= 108:
            continue
        if not math.isfinite(start) or not math.isfinite(end) or end <= start:
            continue
        if end - start < minimum_seconds:
            continue
        note.pitch = pitch
        note.start = max(0.0, start)
        note.end = end
        note.velocity = max(1, min(127, velocity))
        cleaned.append(note)
    cleaned.sort(key=lambda item: (item.start, item.pitch, item.end))
    return cleaned


def transcribe(model: Any, torch: Any, audio: Any) -> list[Any]:
    try:
        with contextlib.redirect_stdout(sys.stderr), torch.inference_mode():
            tensor = torch.from_numpy(audio).to("cpu")
            notes = model.transcribe(
                tensor,
                stepInSecond=None,
                segmentSizeInSecond=None,
                discardSecondHalf=False,
            )
        return list(notes)
    except Exception as error:  # noqa: BLE001
        fail("INFERENCE_FAILED", f"Transkun 推理失败: {error}", 6)


def write_outputs(
    midi_path: Path,
    metadata_path: Path,
    write_midi: Any,
    notes: Sequence[Any],
    metadata: dict[str, Any],
) -> None:
    midi_path.parent.mkdir(parents=True, exist_ok=True)
    metadata_path.parent.mkdir(parents=True, exist_ok=True)
    midi_temp = midi_path.with_name(f"{midi_path.name}.tmp-{uuid.uuid4().hex}.mid")
    metadata_temp = metadata_path.with_name(f"{metadata_path.name}.tmp-{uuid.uuid4().hex}")
    try:
        with contextlib.redirect_stdout(sys.stderr):
            output = write_midi(notes)
            output.write(str(midi_temp))
        header = midi_temp.read_bytes()[:4]
        if header != b"MThd":
            fail("MIDI_VALIDATION_FAILED", "Transkun 生成的文件不是有效 MIDI", 7)
        metadata_temp.write_text(
            json.dumps(metadata, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        os.replace(midi_temp, midi_path)
        os.replace(metadata_temp, metadata_path)
    except WorkerFailure:
        raise
    except Exception as error:  # noqa: BLE001
        fail("MIDI_WRITE_FAILED", f"MIDI 写入失败: {error}", 7)
    finally:
        for temporary in (midi_temp, metadata_temp):
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass


def run_transcription(args: argparse.Namespace) -> None:
    request_id = args.request_id
    input_path = Path(args.input)
    output_midi = Path(args.output_midi)
    output_metadata = Path(args.output_metadata)
    started = time.perf_counter()

    emit_stage(request_id, "validating_input", "正在验证纯钢琴音频", 0.0)
    validate_input(input_path)

    emit_stage(request_id, "loading_model", "正在加载 Transkun v2 高质量钢琴模型", 0.05)
    torch, model, write_midi, model_file = load_model()

    emit_stage(request_id, "decoding_audio", "正在本地解码并转换为 44.1kHz 单声道", 0.12)
    audio, duration_seconds = decode_audio(input_path)

    emit_stage(request_id, "transcribing", "正在进行高质量钢琴音符识别", 0.20)
    raw_notes = transcribe(model, torch, audio)
    notes = clean_notes(raw_notes, args.minimum_note_length_ms)

    emit_stage(request_id, "writing_midi", "正在写入并验证 MIDI", 0.94)
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    metadata = {
        "schema_version": SCHEMA_VERSION,
        "engine": "scoreleap-transkun-v2",
        "engine_version": WORKER_VERSION,
        "model_file": model_file,
        "sample_rate": TARGET_SAMPLE_RATE,
        "duration_seconds": duration_seconds,
        "note_count": len(notes),
        "elapsed_ms": elapsed_ms,
        "preset_compat": args.preset,
        "notes": [
            {
                "start_seconds": float(note.start),
                "end_seconds": float(note.end),
                "pitch": int(note.pitch),
                "velocity": int(note.velocity),
            }
            for note in notes
        ],
    }
    write_outputs(output_midi, output_metadata, write_midi, notes, metadata)
    emit(
        "result",
        request_id,
        midi_path=str(output_midi),
        metadata_path=str(output_metadata),
        elapsed_ms=elapsed_ms,
        note_count=len(notes),
        progress=1.0,
    )


def run_self_test() -> int:
    request_id = "self-test"
    required_modules = ("torch", "torchaudio", "transkun", "moduleconf", "miniaudio", "pretty_midi")
    missing = [name for name in required_modules if importlib.util.find_spec(name) is None]
    if missing:
        emit("error", request_id, code="RUNTIME_MISSING", message=f"缺少模块: {', '.join(missing)}")
        return 1
    try:
        weight, config = bundled_model_paths()
    except WorkerFailure as error:
        emit("error", request_id, code=error.code, message=error.message)
        return error.exit_code
    emit(
        "result",
        request_id,
        worker_version=WORKER_VERSION,
        model_file=weight.name,
        config_file=config.name,
        cpu_only=True,
        ffmpeg_required=False,
        python_install_required=False,
    )
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    configure_console()
    arguments = list(sys.argv[1:] if argv is None else argv)
    if arguments == ["self-test"]:
        return run_self_test()

    request_id = "unknown"
    try:
        args = parse_transcribe_args(arguments)
        request_id = args.request_id
        emit("ready", request_id, worker_version=WORKER_VERSION)
        run_transcription(args)
        return 0
    except SystemExit:
        emit("error", request_id, code="WORKER_PROTOCOL_ERROR", message="Worker 参数无效")
        return 2
    except WorkerFailure as error:
        emit("error", request_id, code=error.code, message=error.message, detail=error.message)
        return error.exit_code
    except Exception as error:  # noqa: BLE001 - 最终崩溃边界
        traceback.print_exc(file=sys.stderr)
        emit("error", request_id, code="INTERNAL_ERROR", message=f"高质量转录内部错误: {error}")
        return 9


if __name__ == "__main__":
    raise SystemExit(main())
