use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelManagerError {
    #[error("I/O 失败: {0}")]
    Io(#[from] io::Error),
    #[error("JSON 失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP 失败: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("清单无效: {0}")]
    InvalidManifest(String),
    #[error("签名无效: {0}")]
    InvalidSignature(String),
    #[error("模型与当前引擎不兼容: {0}")]
    IncompatibleEngine(String),
    #[error("操作已取消")]
    Cancelled,
    #[error("所有下载源均失败: {0:?}")]
    AllSourcesFailed(Vec<String>),
    #[error("大小不匹配，期望 {expected}，实际 {actual}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("SHA-256 不匹配，期望 {expected}，实际 {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("ZIP 包含不安全路径: {0}")]
    UnsafeArchivePath(String),
    #[error("ZIP 包含未声明项目: {0}")]
    UnexpectedArchiveEntry(String),
    #[error("ZIP 包含重复项目: {0}")]
    DuplicateArchiveEntry(String),
    #[error("ZIP 缺少声明文件: {0}")]
    MissingArtifact(String),
    #[error("ZIP 解压限制被触发: {0}")]
    ExtractionLimit(String),
    #[error("模型 {0} 正由另一个进程管理")]
    InstallLocked(String),
    #[error("模型缓存不存在: {0}/{1}")]
    CacheMissing(String, String),
    #[error("模型缓存无效: {0}")]
    CacheInvalid(String),
    #[error("没有可回滚版本: {0}")]
    NoRollbackVersion(String),
    #[error("模型信任配置缺失: {0}")]
    TrustConfigurationMissing(String),
    #[error("下载配置无效: {0}")]
    InvalidDownloadConfiguration(String),
}
