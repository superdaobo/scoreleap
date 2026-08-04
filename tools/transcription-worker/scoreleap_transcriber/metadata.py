"""转录元数据。默认不保存原始绝对路径（隐私）；保存文件名与 SHA256。"""

import hashlib
import json
import os
from typing import Any, Dict, List


def sha256_of(path: str, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            data = f.read(chunk)
            if not data:
                break
            h.update(data)
    return h.hexdigest()


def build_metadata(
    request_id: str,
    source_path: str,
    source_size_bytes: int,
    source_duration_ms: int,
    engine_version: str,
    worker_version: str,
    midi_file: str,
    note_count: int,
    elapsed_ms: int,
    warnings: List[str],
) -> Dict[str, Any]:
    """构建 metadata.json 内容（source_type 标识音频转录来源）。"""
    return {
        "schema_version": 1,
        "request_id": request_id,
        "source_type": "audio_transcription",
        "source": {
            "file_name": os.path.basename(source_path),
            "sha256": sha256_of(source_path),
            "size_bytes": source_size_bytes,
            "duration_ms": source_duration_ms,
        },
        "transcriber": {
            "engine": "basic-pitch",
            "engine_version": engine_version,
            "worker_version": worker_version,
        },
        "result": {
            "midi_file": midi_file,
            "note_count": note_count,
            "elapsed_ms": elapsed_ms,
        },
        "warnings": warnings,
    }


def write_metadata(path: str, data: Dict[str, Any]) -> None:
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
