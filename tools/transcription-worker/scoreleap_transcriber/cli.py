"""scoreleap-transcriber CLI。

stdout 仅输出 JSON Lines；日志走 stderr；退出码见 errors.py。
"""

import argparse
import os
import sys
from typing import List, Optional

from .errors import (
    EXIT_ARGS,
    EXIT_SUCCESS,
    TranscriptionError,
    args_error,
    input_error,
)
from .protocol import MessageWriter, WORKER_VERSION

# 输入限制（MVP）
MAX_FILE_BYTES = 200 * 1024 * 1024   # 200 MB
MAX_DURATION_MS = 10 * 60 * 1000     # 10 分钟
ALLOWED_EXT = {".mp3"}


def _reconfigure_stdio() -> None:
    """中文 Windows 控制台默认 GBK：必须 UTF-8，否则 emoji/中文输出崩溃（Spike 发现）。"""
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:  # noqa: BLE001 - 非标准流（如已被重定向）时忽略
            pass


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="scoreleap-transcriber")
    sub = p.add_subparsers(dest="command", required=True)
    t = sub.add_parser("transcribe", help="转录音频为 MIDI")
    t.add_argument("--request-id", required=True, help="任务请求 ID（UUID）")
    t.add_argument("--input", required=True, help="输入 MP3 绝对路径")
    t.add_argument("--output-midi", required=True, help="输出 MIDI 绝对路径（任务目录内）")
    t.add_argument("--output-metadata", required=True, help="输出 metadata.json 绝对路径")
    t.add_argument("--onset-threshold", type=float, default=None)
    t.add_argument("--frame-threshold", type=float, default=None)
    t.add_argument("--minimum-note-length-ms", type=float, default=None)
    t.add_argument("--minimum-frequency", type=float, default=None)
    t.add_argument("--maximum-frequency", type=float, default=None)
    return p


def validate_input(input_path: str) -> None:
    """输入验证：存在、普通文件、.mp3、规范化、非空、≤200MB。"""
    p = os.path.abspath(os.path.normpath(input_path))
    if not os.path.exists(p):
        raise input_error("INVALID_AUDIO_PATH", "输入文件不存在", p)
    if not os.path.isfile(p):
        raise input_error("INVALID_AUDIO_PATH", "输入不是普通文件", p)
    ext = os.path.splitext(p)[1].lower()
    if ext not in ALLOWED_EXT:
        raise input_error(
            "UNSUPPORTED_AUDIO_FORMAT", f"仅支持 MP3，收到 {ext or '无扩展名'}", p
        )
    size = os.path.getsize(p)
    if size == 0:
        raise input_error("INVALID_AUDIO_PATH", "输入文件为空", p)
    if size > MAX_FILE_BYTES:
        raise input_error("AUDIO_FILE_TOO_LARGE", f"文件超过 {MAX_FILE_BYTES // (1024*1024)}MB 上限", p)


def validate_outputs(output_midi: str, output_metadata: str) -> None:
    """输出验证：目录存在、不覆盖已存在文件。"""
    for out in (output_midi, output_metadata):
        d = os.path.dirname(os.path.abspath(out))
        if not os.path.isdir(d):
            raise args_error(f"输出目录不存在: {d}")
        if os.path.exists(out):
            raise args_error(f"输出文件已存在（拒绝覆盖）: {out}")


def measure_duration_ms(input_path: str) -> int:
    """读取音频时长（librosa）；解码失败抛 AUDIO_DECODE_FAILED。"""
    try:
        import librosa

        dur = librosa.get_duration(path=input_path)
        return int(dur * 1000)
    except Exception as e:  # noqa: BLE001
        from .errors import decode_error

        raise decode_error(str(e)) from e


def main(argv: Optional[List[str]] = None) -> int:
    _reconfigure_stdio()
    writer = MessageWriter()
    parser = build_parser()
    args = parser.parse_args(argv)
    request_id = args.request_id
    writer.ready(request_id)

    try:
        if args.command != "transcribe":
            raise args_error(f"未知命令: {args.command}")

        validate_input(args.input)
        validate_outputs(args.output_midi, args.output_metadata)
        fake = os.environ.get("SCORELEAP_FAKE_PREDICTOR") == "1"
        # Fake 模式（测试）不解码真实音频，跳过时长测量
        duration_ms = 0 if fake else measure_duration_ms(args.input)
        if duration_ms > MAX_DURATION_MS:
            from .errors import input_error as _ie

            raise _ie(
                "AUDIO_TOO_LONG",
                f"音频超过 {MAX_DURATION_MS // 60000} 分钟上限（{duration_ms / 1000:.0f}s）",
            )

        # 使用 FakePredictor 还是真实模型：环境变量开关（测试用），默认真实
        if fake:
            from .transcriber import FakePredictor

            predictor = FakePredictor()
        else:
            from .transcriber import BasicPitchPredictor

            predictor = BasicPitchPredictor()

        from .transcriber import Transcriber

        tx = Transcriber(
            predictor=predictor,
            writer=writer,
            request_id=request_id,
        )
        return tx.run(
            input_path=args.input,
            output_midi=args.output_midi,
            output_metadata=args.output_metadata,
            source_size_bytes=os.path.getsize(args.input),
            source_duration_ms=duration_ms,
            warnings=["MVP 限制：完整歌曲可能出现杂音符（无音源分离）"],
        )
    except TranscriptionError as e:
        writer.error(request_id, e.code, e.message, e.detail)
        return e.exit_code
    except Exception as e:  # noqa: BLE001 - 兜底内部错误
        writer.error(request_id, "INTERNAL_ERROR", "未知内部错误", str(e))
        return 9

