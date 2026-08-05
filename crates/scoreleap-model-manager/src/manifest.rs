use crate::ModelManagerError;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineCompatibility {
    pub engine_id: String,
    pub min_version: String,
    pub max_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Cdn,
    GithubRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSource {
    pub kind: SourceKind,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageDescriptor {
    pub size_bytes: u64,
    pub sha256: String,
    /// 按声明顺序尝试，通常 CDN 在前、GitHub Releases 在后。
    pub sources: Vec<ModelSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseDescriptor {
    pub spdx_id: String,
    pub name: String,
    pub url: String,
    pub redistribution_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub model_id: String,
    pub version: String,
    pub engine_compat: EngineCompatibility,
    pub artifacts: Vec<ArtifactDescriptor>,
    pub package: PackageDescriptor,
    pub license: LicenseDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignatureEnvelope {
    pub algorithm: String,
    pub key_id: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedModelManifest {
    pub manifest: ModelManifest,
    pub signature: SignatureEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCatalog {
    pub schema_version: u32,
    pub models: Vec<SignedModelManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedModelCatalog {
    pub catalog: ModelCatalog,
    pub signature: SignatureEnvelope,
}

pub fn manifest_signing_bytes(manifest: &ModelManifest) -> Result<Vec<u8>, ModelManagerError> {
    Ok(serde_json::to_vec(manifest)?)
}

pub fn catalog_signing_bytes(catalog: &ModelCatalog) -> Result<Vec<u8>, ModelManagerError> {
    Ok(serde_json::to_vec(catalog)?)
}

fn verify_signature(
    bytes: &[u8],
    envelope: &SignatureEnvelope,
    key: &VerifyingKey,
) -> Result<(), ModelManagerError> {
    if envelope.algorithm != "ed25519" {
        return Err(ModelManagerError::InvalidSignature(format!(
            "不支持算法 {}",
            envelope.algorithm
        )));
    }
    let raw = hex::decode(&envelope.signature_hex)
        .map_err(|error| ModelManagerError::InvalidSignature(error.to_string()))?;
    let signature = Signature::from_slice(&raw)
        .map_err(|error| ModelManagerError::InvalidSignature(error.to_string()))?;
    key.verify_strict(bytes, &signature)
        .map_err(|error| ModelManagerError::InvalidSignature(error.to_string()))
}

pub fn verify_signed_manifest(
    signed: &SignedModelManifest,
    key: &VerifyingKey,
) -> Result<(), ModelManagerError> {
    validate_manifest(&signed.manifest)?;
    verify_signature(
        &manifest_signing_bytes(&signed.manifest)?,
        &signed.signature,
        key,
    )
}

pub fn verify_signed_catalog(
    signed: &SignedModelCatalog,
    key: &VerifyingKey,
) -> Result<(), ModelManagerError> {
    if signed.catalog.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(ModelManagerError::InvalidManifest(format!(
            "不支持目录 schema_version {}",
            signed.catalog.schema_version
        )));
    }
    verify_signature(
        &catalog_signing_bytes(&signed.catalog)?,
        &signed.signature,
        key,
    )?;
    for model in &signed.catalog.models {
        verify_signed_manifest(model, key)?;
    }
    Ok(())
}

/// 固定 64 KiB 缓冲区，避免大模型校验时将文件整体载入内存。
pub fn sha256_reader(mut reader: impl Read) -> Result<(u64, String), ModelManagerError> {
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| ModelManagerError::ExtractionLimit("文件大小溢出".into()))?;
        digest.update(&buffer[..count]);
    }
    Ok((size, hex::encode(digest.finalize())))
}

pub fn verify_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), ModelManagerError> {
    let (actual_size, actual_sha256) = sha256_reader(File::open(path)?)?;
    if actual_size != expected_size {
        return Err(ModelManagerError::SizeMismatch {
            expected: expected_size,
            actual: actual_size,
        });
    }
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(ModelManagerError::HashMismatch {
            expected: expected_sha256.to_ascii_lowercase(),
            actual: actual_sha256,
        });
    }
    Ok(())
}

pub(crate) fn validate_manifest(manifest: &ModelManifest) -> Result<(), ModelManagerError> {
    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(ModelManagerError::InvalidManifest(format!(
            "不支持 schema_version {}",
            manifest.schema_version
        )));
    }
    validate_component(&manifest.model_id, "model_id")?;
    validate_component(&manifest.version, "version")?;
    validate_component(&manifest.engine_compat.engine_id, "engine_id")?;
    parse_version(&manifest.engine_compat.min_version)?;
    if let Some(maximum) = &manifest.engine_compat.max_version {
        parse_version(maximum)?;
        if compare_version(&manifest.engine_compat.min_version, maximum)?.is_gt() {
            return Err(ModelManagerError::InvalidManifest(
                "引擎版本范围反向".into(),
            ));
        }
    }
    if manifest.artifacts.is_empty() || manifest.package.sources.is_empty() {
        return Err(ModelManagerError::InvalidManifest(
            "artifacts 和 sources 不得为空".into(),
        ));
    }
    if manifest.package.size_bytes == 0 {
        return Err(ModelManagerError::InvalidManifest(
            "包大小必须大于零".into(),
        ));
    }
    validate_sha256(&manifest.package.sha256)?;
    let mut paths = HashSet::new();
    for artifact in &manifest.artifacts {
        let path = safe_relative_path(&artifact.path)?;
        if artifact.size_bytes == 0 || !paths.insert(path) {
            return Err(ModelManagerError::InvalidManifest(format!(
                "artifact 无效或重复: {}",
                artifact.path
            )));
        }
        validate_sha256(&artifact.sha256)?;
    }
    for source in &manifest.package.sources {
        if !source.url.starts_with("https://") {
            return Err(ModelManagerError::InvalidManifest(format!(
                "下载源必须使用 HTTPS: {}",
                source.url
            )));
        }
    }
    if manifest.license.spdx_id.trim().is_empty()
        || manifest.license.name.trim().is_empty()
        || !manifest.license.url.starts_with("https://")
    {
        return Err(ModelManagerError::InvalidManifest(
            "许可证信息不完整".into(),
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), ModelManagerError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ModelManagerError::InvalidManifest(format!(
            "SHA-256 格式无效: {value}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_component(value: &str, field: &str) -> Result<(), ModelManagerError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ModelManagerError::InvalidManifest(format!(
            "{field} 含非法字符"
        )));
    }
    Ok(())
}

pub(crate) fn safe_relative_path(raw: &str) -> Result<PathBuf, ModelManagerError> {
    if raw.is_empty() || raw.contains('\\') {
        return Err(ModelManagerError::UnsafeArchivePath(raw.into()));
    }
    let mut clean = PathBuf::new();
    for component in Path::new(raw).components() {
        match component {
            Component::Normal(value) => clean.push(value),
            _ => return Err(ModelManagerError::UnsafeArchivePath(raw.into())),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(ModelManagerError::UnsafeArchivePath(raw.into()));
    }
    Ok(clean)
}

fn parse_version(value: &str) -> Result<Vec<u64>, ModelManagerError> {
    if value.is_empty() {
        return Err(ModelManagerError::InvalidManifest("版本为空".into()));
    }
    value
        .split('.')
        .map(|part| {
            part.parse::<u64>().map_err(|_| {
                ModelManagerError::InvalidManifest(format!("版本不是纯数字点分格式: {value}"))
            })
        })
        .collect()
}

pub(crate) fn compare_version(
    left: &str,
    right: &str,
) -> Result<std::cmp::Ordering, ModelManagerError> {
    let mut left = parse_version(left)?;
    let mut right = parse_version(right)?;
    let length = left.len().max(right.len());
    left.resize(length, 0);
    right.resize(length, 0);
    Ok(left.cmp(&right))
}
