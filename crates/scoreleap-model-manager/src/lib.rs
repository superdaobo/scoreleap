//! ScoreLeap 模型清单校验、完整性验证、安全解包与原子安装。

mod archive;
mod download;
mod error;
mod manager;
mod manifest;

pub use download::{
    AttemptState, CancellationToken, DownloadAttempt, DownloadPhase, DownloadPlan,
    DownloadProgress, DownloadStatus, HttpDownloadConfig, HttpSourceDownloader, ProgressObserver,
    SourceDownloader,
};
pub use error::ModelManagerError;
pub use manager::{ExtractionLimits, InstallOutcome, InstallReport, ModelManager, ModelVersionRef};
pub use manifest::{
    catalog_signing_bytes, manifest_signing_bytes, sha256_reader, verify_file,
    verify_signed_catalog, verify_signed_manifest, ArtifactDescriptor, EngineCompatibility,
    LicenseDescriptor, ModelCatalog, ModelManifest, ModelSource, PackageDescriptor,
    SignatureEnvelope, SignedModelCatalog, SignedModelManifest, SourceKind,
    SUPPORTED_SCHEMA_VERSION,
};

/// 从应用显式提供的 32 字节 Ed25519 公钥构造信任根。
pub fn verifying_key_from_hex(
    value: &str,
) -> Result<ed25519_dalek::VerifyingKey, ModelManagerError> {
    let bytes = hex::decode(value.trim())
        .map_err(|error| ModelManagerError::InvalidSignature(error.to_string()))?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
        ModelManagerError::InvalidSignature("Ed25519 公钥必须为 32 字节十六进制".into())
    })?;
    ed25519_dalek::VerifyingKey::from_bytes(&bytes)
        .map_err(|error| ModelManagerError::InvalidSignature(error.to_string()))
}

#[cfg(test)]
mod tests;
