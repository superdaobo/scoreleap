use crate::manifest::safe_relative_path;
use crate::{ArtifactDescriptor, CancellationToken, ModelManagerError};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy)]
pub struct ExtractionLimits {
    pub max_files: usize,
    pub max_single_file_bytes: u64,
    pub max_total_uncompressed_bytes: u64,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_files: 32,
            max_single_file_bytes: 4 * 1024 * 1024 * 1024,
            max_total_uncompressed_bytes: 6 * 1024 * 1024 * 1024,
        }
    }
}

/// 只解压清单声明的普通文件；路径、数量、展开大小和摘要任一不符即终止。
pub(crate) fn extract_verified_archive(
    archive_path: &Path,
    destination: &Path,
    artifacts: &[ArtifactDescriptor],
    limits: ExtractionLimits,
    cancellation: &CancellationToken,
) -> Result<(), ModelManagerError> {
    if artifacts.len() > limits.max_files {
        return Err(ModelManagerError::ExtractionLimit("声明文件数超限".into()));
    }
    let mut expected = HashMap::new();
    let mut allowed_dirs = HashSet::new();
    for artifact in artifacts {
        let path = safe_relative_path(&artifact.path)?;
        let mut parent = path.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            allowed_dirs.insert(value.to_path_buf());
            parent = value.parent();
        }
        expected.insert(path, artifact);
    }

    let mut archive = ZipArchive::new(File::open(archive_path)?)?;
    if archive.len() > limits.max_files + allowed_dirs.len() {
        return Err(ModelManagerError::ExtractionLimit("ZIP 项目数超限".into()));
    }
    let mut seen = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        if cancellation.is_cancelled() {
            return Err(ModelManagerError::Cancelled);
        }
        let mut entry = archive.by_index(index)?;
        let raw_name = entry.name().to_owned();
        let relative = safe_relative_path(raw_name.trim_end_matches('/'))?;
        if entry.is_dir() {
            if !allowed_dirs.contains(&relative) {
                return Err(ModelManagerError::UnexpectedArchiveEntry(raw_name));
            }
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(ModelManagerError::UnsafeArchivePath(raw_name));
        }
        let artifact = expected
            .get(&relative)
            .ok_or_else(|| ModelManagerError::UnexpectedArchiveEntry(raw_name.clone()))?;
        if !seen.insert(relative.clone()) {
            return Err(ModelManagerError::DuplicateArchiveEntry(raw_name));
        }
        if entry.size() != artifact.size_bytes {
            return Err(ModelManagerError::SizeMismatch {
                expected: artifact.size_bytes,
                actual: entry.size(),
            });
        }
        if entry.size() > limits.max_single_file_bytes {
            return Err(ModelManagerError::ExtractionLimit(format!(
                "单文件超限: {}",
                artifact.path
            )));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| ModelManagerError::ExtractionLimit("总大小溢出".into()))?;
        if total > limits.max_total_uncompressed_bytes {
            return Err(ModelManagerError::ExtractionLimit("总解压大小超限".into()));
        }

        let output_path = destination.join(&relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&output_path)?;
        let mut digest = Sha256::new();
        let mut written = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            if cancellation.is_cancelled() {
                return Err(ModelManagerError::Cancelled);
            }
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            written = written
                .checked_add(count as u64)
                .ok_or_else(|| ModelManagerError::ExtractionLimit("文件大小溢出".into()))?;
            if written > artifact.size_bytes {
                return Err(ModelManagerError::ExtractionLimit(format!(
                    "实际内容超过声明: {}",
                    artifact.path
                )));
            }
            digest.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        output.sync_all()?;
        let actual_hash = hex::encode(digest.finalize());
        if written != artifact.size_bytes {
            return Err(ModelManagerError::SizeMismatch {
                expected: artifact.size_bytes,
                actual: written,
            });
        }
        if !actual_hash.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(ModelManagerError::HashMismatch {
                expected: artifact.sha256.clone(),
                actual: actual_hash,
            });
        }
    }
    for artifact in artifacts {
        let relative = safe_relative_path(&artifact.path)?;
        if !seen.contains(&relative) {
            return Err(ModelManagerError::MissingArtifact(artifact.path.clone()));
        }
    }
    Ok(())
}
