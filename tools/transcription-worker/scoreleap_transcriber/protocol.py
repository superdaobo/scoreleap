"""JSON Lines 通信协议。

stdout 只允许输出 JSON Lines（每行一个完整 JSON 对象），日志一律走 stderr。
Rust 端按行解析；未知字段必须被忽略（向前兼容）。
"""

import json
import sys
import time
from typing import Any, Dict, Optional

SCHEMA_VERSION = 1
WORKER_VERSION = "0.1.0"
ENGINE_NAME = "basic-pitch"


def _ts() -> int:
    return int(time.time() * 1000)


def _msg(msg_type: str, request_id: str, **extra: Any) -> Dict[str, Any]:
    m: Dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "type": msg_type,
        "request_id": request_id,
        "timestamp_ms": _ts(),
    }
    m.update(extra)
    return m


class MessageWriter:
    """向 stdout 输出 JSON Lines 消息（线程安全由调用方保证）。"""

    def __init__(self, stream=None):
        self._stream = stream if stream is not None else sys.stdout

    def write(self, obj: Dict[str, Any]) -> None:
        line = json.dumps(obj, ensure_ascii=False, separators=(",", ":"))
        self._stream.write(line + "\n")
        self._stream.flush()

    def ready(self, request_id: str) -> None:
        self.write(_msg("ready", request_id, worker_version=WORKER_VERSION))

    def stage(self, request_id: str, stage: str, message: str) -> None:
        self.write(_msg("stage", request_id, stage=stage, message=message))

    def result(
        self,
        request_id: str,
        midi_path: str,
        metadata_path: str,
        elapsed_ms: int,
        note_count: int,
    ) -> None:
        self.write(
            _msg(
                "result",
                request_id,
                midi_path=midi_path,
                metadata_path=metadata_path,
                elapsed_ms=elapsed_ms,
                note_count=note_count,
            )
        )

    def error(self, request_id: str, code: str, message: str, detail: str = "") -> None:
        m = _msg("error", request_id, code=code, message=message)
        if detail:
            m["detail"] = detail
        self.write(m)


def parse_line(line: str) -> Optional[Dict[str, Any]]:
    """解析单行 JSON（供测试与 Rust 端参考实现对照）；非法 JSON 返回 None。"""
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return None
