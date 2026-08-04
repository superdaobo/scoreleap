"""ScoreLeap 转录 Worker：错误码与退出码。"""

# 退出码（对外契约，Rust 端依赖）
EXIT_SUCCESS = 0
EXIT_ARGS = 2          # 参数错误
EXIT_INPUT = 3         # 输入文件错误
EXIT_DECODE = 4        # 音频解码失败
EXIT_MODEL = 5         # 模型加载失败
EXIT_INFERENCE = 6     # 推理失败
EXIT_MIDI_WRITE = 7    # MIDI 写入失败
EXIT_CANCELLED = 8     # 任务取消
EXIT_INTERNAL = 9      # 未知内部错误

# 结构化错误码（JSON error.type，与 Rust 端约定一致）
ERR_INVALID_ARGS = "INVALID_ARGS"
ERR_INVALID_AUDIO_PATH = "INVALID_AUDIO_PATH"
ERR_UNSUPPORTED_FORMAT = "UNSUPPORTED_AUDIO_FORMAT"
ERR_FILE_TOO_LARGE = "AUDIO_FILE_TOO_LARGE"
ERR_AUDIO_TOO_LONG = "AUDIO_TOO_LONG"
ERR_DECODE_FAILED = "AUDIO_DECODE_FAILED"
ERR_MODEL_LOAD_FAILED = "MODEL_LOAD_FAILED"
ERR_INFERENCE_FAILED = "INFERENCE_FAILED"
ERR_MIDI_WRITE_FAILED = "MIDI_WRITE_FAILED"
ERR_JOB_CANCELLED = "JOB_CANCELLED"
ERR_INTERNAL = "INTERNAL_ERROR"


class TranscriptionError(Exception):
    """带结构化错误码与退出码的转录异常。"""

    def __init__(self, code: str, message: str, exit_code: int, detail: str = ""):
        super().__init__(message)
        self.code = code
        self.message = message
        self.detail = detail
        self.exit_code = exit_code


def args_error(message: str) -> TranscriptionError:
    return TranscriptionError(ERR_INVALID_ARGS, message, EXIT_ARGS)


def input_error(code: str, message: str, detail: str = "") -> TranscriptionError:
    return TranscriptionError(code, message, EXIT_INPUT, detail)


def decode_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_DECODE_FAILED, "无法解码音频", EXIT_DECODE, detail)


def model_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_MODEL_LOAD_FAILED, "模型加载失败", EXIT_MODEL, detail)


def inference_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_INFERENCE_FAILED, "音符识别失败", EXIT_INFERENCE, detail)


def midi_write_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_MIDI_WRITE_FAILED, "MIDI 写入失败", EXIT_MIDI_WRITE, detail)


def cancelled_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_JOB_CANCELLED, "任务已取消", EXIT_CANCELLED, detail)


def internal_error(detail: str = "") -> TranscriptionError:
    return TranscriptionError(ERR_INTERNAL, "未知内部错误", EXIT_INTERNAL, detail)
