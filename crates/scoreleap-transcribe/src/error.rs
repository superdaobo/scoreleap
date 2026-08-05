use scoreleap_audio::AudioError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidAudioPath,
    UnsupportedAudioFormat,
    AudioFileTooLarge,
    AudioTooLong,
    AudioDecodeFailed,
    ModelLoadFailed,
    InferenceFailed,
    MidiWriteFailed,
    MidiValidationFailed,
    WorkerProtocolError,
    InternalError,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAudioPath => "INVALID_AUDIO_PATH",
            Self::UnsupportedAudioFormat => "UNSUPPORTED_AUDIO_FORMAT",
            Self::AudioFileTooLarge => "AUDIO_FILE_TOO_LARGE",
            Self::AudioTooLong => "AUDIO_TOO_LONG",
            Self::AudioDecodeFailed => "AUDIO_DECODE_FAILED",
            Self::ModelLoadFailed => "MODEL_LOAD_FAILED",
            Self::InferenceFailed => "INFERENCE_FAILED",
            Self::MidiWriteFailed => "MIDI_WRITE_FAILED",
            Self::MidiValidationFailed => "MIDI_VALIDATION_FAILED",
            Self::WorkerProtocolError => "WORKER_PROTOCOL_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranscribeError {
    #[error("转录参数无效: {0}")]
    InvalidOptions(String),
    #[error("音频路径无效: {0}")]
    InvalidAudioPath(String),
    #[error("音频处理失败: {0}")]
    Audio(#[from] AudioError),
    #[error("ONNX Runtime 初始化失败: {0}")]
    RuntimeInitialization(String),
    #[error("Basic Pitch 模型加载失败: {0}")]
    ModelLoad(String),
    #[error("Basic Pitch 模型接口不兼容: {0}")]
    ModelInterface(String),
    #[error("ONNX 推理失败: {0}")]
    Inference(String),
    #[error("模型输出无效: {0}")]
    InvalidOutput(String),
    #[error("MIDI 写入失败: {0}")]
    MidiWrite(String),
    #[error("MIDI 验证失败: {0}")]
    MidiValidation(String),
    #[error("元数据写入失败: {0}")]
    MetadataWrite(String),
}

impl TranscribeError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidOptions(_) => ErrorCode::WorkerProtocolError,
            Self::InvalidAudioPath(_) => ErrorCode::InvalidAudioPath,
            Self::Audio(error) => match error {
                AudioError::Metadata { .. }
                | AudioError::NotRegularFile { .. }
                | AudioError::EmptyFile { .. }
                | AudioError::Open { .. } => ErrorCode::InvalidAudioPath,
                AudioError::UnsupportedExtension { .. } => ErrorCode::UnsupportedAudioFormat,
                AudioError::FileTooLarge { .. } => ErrorCode::AudioFileTooLarge,
                AudioError::DurationExceeded { .. } => ErrorCode::AudioTooLong,
                _ => ErrorCode::AudioDecodeFailed,
            },
            Self::RuntimeInitialization(_) | Self::ModelLoad(_) | Self::ModelInterface(_) => {
                ErrorCode::ModelLoadFailed
            }
            Self::Inference(_) | Self::InvalidOutput(_) => ErrorCode::InferenceFailed,
            Self::MidiWrite(_) => ErrorCode::MidiWriteFailed,
            Self::MidiValidation(_) => ErrorCode::MidiValidationFailed,
            Self::MetadataWrite(_) => ErrorCode::InternalError,
        }
    }
}
