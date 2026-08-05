use crate::{verify_file, ModelManagerError, ModelSource, PackageDescriptor};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// 网络层仅负责把指定源写入 sink；重试顺序、取消和完整性由核心统一控制。
pub trait SourceDownloader: Send + Sync {
    fn download(
        &self,
        source: &ModelSource,
        destination: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Pending,
    Downloading,
    Failed,
    Succeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadAttempt {
    pub source: ModelSource,
    pub state: AttemptState,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Verifying,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadPlan {
    pub status: DownloadStatus,
    pub attempts: Vec<DownloadAttempt>,
}

impl DownloadPlan {
    pub fn new(sources: &[ModelSource]) -> Self {
        Self {
            status: DownloadStatus::Pending,
            attempts: sources
                .iter()
                .cloned()
                .map(|source| DownloadAttempt {
                    source,
                    state: AttemptState::Pending,
                    error: None,
                })
                .collect(),
        }
    }

    pub fn execute(
        &mut self,
        downloader: &dyn SourceDownloader,
        part_path: &Path,
        package: &PackageDescriptor,
        cancellation: &CancellationToken,
    ) -> Result<(), ModelManagerError> {
        let mut failures = Vec::new();
        for attempt in &mut self.attempts {
            if cancellation.is_cancelled() {
                self.status = DownloadStatus::Cancelled;
                let _ = fs::remove_file(part_path);
                return Err(ModelManagerError::Cancelled);
            }
            self.status = DownloadStatus::Downloading;
            attempt.state = AttemptState::Downloading;
            let result = (|| -> Result<(), ModelManagerError> {
                let mut destination = File::create(part_path)?;
                downloader
                    .download(&attempt.source, &mut destination, cancellation)
                    .map_err(|message| ModelManagerError::Io(std::io::Error::other(message)))?;
                destination.flush()?;
                destination.sync_all()?;
                if cancellation.is_cancelled() {
                    return Err(ModelManagerError::Cancelled);
                }
                self.status = DownloadStatus::Verifying;
                verify_file(part_path, package.size_bytes, &package.sha256)
            })();
            match result {
                Ok(()) => {
                    attempt.state = AttemptState::Succeeded;
                    self.status = DownloadStatus::Completed;
                    return Ok(());
                }
                Err(ModelManagerError::Cancelled) => {
                    attempt.state = AttemptState::Failed;
                    attempt.error = Some("cancelled".into());
                    self.status = DownloadStatus::Cancelled;
                    let _ = fs::remove_file(part_path);
                    return Err(ModelManagerError::Cancelled);
                }
                Err(error) => {
                    attempt.state = AttemptState::Failed;
                    attempt.error = Some(error.to_string());
                    failures.push(format!("{}: {error}", attempt.source.url));
                    let _ = fs::remove_file(part_path);
                }
            }
        }
        self.status = DownloadStatus::Failed;
        Err(ModelManagerError::AllSourcesFailed(failures))
    }
}
