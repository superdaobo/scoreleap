use std::path::PathBuf;
use thiserror::Error;

/// 音频探测、解码和重采样的结构化错误。
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("音频配置无效：{field}（{reason}）")]
    InvalidConfig {
        field: &'static str,
        reason: &'static str,
    },
    #[error("无法读取音频文件元数据：{path}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("输入路径不是普通文件：{path}")]
    NotRegularFile { path: PathBuf },
    #[error("不支持的音频扩展名：{extension}")]
    UnsupportedExtension { extension: String },
    #[error("音频文件为空：{path}")]
    EmptyFile { path: PathBuf },
    #[error("音频文件超过大小限制：{actual_bytes} > {max_bytes} 字节")]
    FileTooLarge { actual_bytes: u64, max_bytes: u64 },
    #[error("无法打开音频文件：{path}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("无法识别音频容器：{message}")]
    Probe { message: String },
    #[error("音频容器没有默认轨道")]
    MissingDefaultTrack,
    #[error("默认音轨缺少采样率")]
    MissingSampleRate,
    #[error("默认音轨缺少声道信息")]
    MissingChannels,
    #[error("音频没有可解码样本")]
    EmptyAudio,
    #[error("音频时长超过限制：{actual_seconds:.3} > {max_seconds:.3} 秒")]
    DurationExceeded {
        actual_seconds: f64,
        max_seconds: f64,
    },
    #[error("无法创建音频解码器：{message}")]
    DecoderCreation { message: String },
    #[error("音频解码失败：{message}")]
    Decode { message: String },
    #[error("音频流参数在解码期间发生变化")]
    StreamParametersChanged,
    #[error("音频包含非有限样本，位置：{sample_index}")]
    NonFiniteSample { sample_index: usize },
    #[error("重采样器初始化失败：{message}")]
    ResamplerCreation { message: String },
    #[error("重采样失败：{message}")]
    Resample { message: String },
    #[error("音频帧数计算溢出")]
    FrameCountOverflow,
    #[error("输出帧数超过安全上限：{actual_frames} > {max_frames}")]
    OutputFrameLimitExceeded {
        actual_frames: u128,
        max_frames: usize,
    },
}
