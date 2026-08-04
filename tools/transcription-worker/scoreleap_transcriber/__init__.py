"""scoreleap-transcriber 包。"""

from .cli import main
from .errors import (
    EXIT_ARGS,
    EXIT_CANCELLED,
    EXIT_DECODE,
    EXIT_INFERENCE,
    EXIT_INPUT,
    EXIT_INTERNAL,
    EXIT_MIDI_WRITE,
    EXIT_MODEL,
    EXIT_SUCCESS,
    TranscriptionError,
)
from .protocol import WORKER_VERSION

__all__ = [
    "main",
    "TranscriptionError",
    "WORKER_VERSION",
    "EXIT_SUCCESS",
    "EXIT_ARGS",
    "EXIT_INPUT",
    "EXIT_DECODE",
    "EXIT_MODEL",
    "EXIT_INFERENCE",
    "EXIT_MIDI_WRITE",
    "EXIT_CANCELLED",
    "EXIT_INTERNAL",
]
