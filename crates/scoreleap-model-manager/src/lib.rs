//! ScoreLeap 模型清单校验、完整性验证、安全解包与原子安装。

mod archive;
mod download;
mod error;
mod manager;
mod manifest;

pub use download::{
    AttemptState, CancellationToken, DownloadAttempt, DownloadPlan, DownloadStatus,
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

#[cfg(test)]
mod tests;
