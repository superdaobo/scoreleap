use crate::{verify_file, ModelManagerError, ModelSource, PackageDescriptor, SourceKind};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadPhase {
    Connecting,
    Receiving,
    Completed,
}

/// 进度事件不携带完整 URL，避免查询参数或未来鉴权信息进入日志/UI。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadProgress {
    pub source_kind: SourceKind,
    pub phase: DownloadPhase,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub type ProgressObserver = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct HttpDownloadConfig {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub max_response_bytes: u64,
    pub user_agent: String,
}

impl Default for HttpDownloadConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(15),
            max_response_bytes: 1024 * 1024 * 1024,
            user_agent: format!("ScoreLeap/{} model-manager", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// 生产同步 HTTP 客户端：仅 HTTPS、无自定义鉴权头、固定大小缓冲流式落盘。
pub struct HttpSourceDownloader {
    client: reqwest::blocking::Client,
    config: HttpDownloadConfig,
    observer: Option<ProgressObserver>,
    allow_http_for_tests: bool,
}

impl HttpSourceDownloader {
    pub fn new(
        config: HttpDownloadConfig,
        observer: Option<ProgressObserver>,
    ) -> Result<Self, ModelManagerError> {
        Self::build(config, observer, false)
    }

    fn build(
        config: HttpDownloadConfig,
        observer: Option<ProgressObserver>,
        allow_http_for_tests: bool,
    ) -> Result<Self, ModelManagerError> {
        if config.max_response_bytes == 0 {
            return Err(ModelManagerError::InvalidDownloadConfiguration(
                "max_response_bytes 必须大于零".into(),
            ));
        }
        let redirect = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 10 {
                return attempt.error(std::io::Error::other("重定向次数超过上限"));
            }
            if url_is_allowed(attempt.url(), allow_http_for_tests) {
                attempt.follow()
            } else {
                attempt.error(std::io::Error::other("重定向目标不符合 HTTPS 安全策略"))
            }
        });
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout)
            .https_only(!allow_http_for_tests)
            .redirect(redirect)
            .user_agent(config.user_agent.as_str())
            .build()
            .map_err(|error| ModelManagerError::InvalidDownloadConfiguration(error.to_string()))?;
        Ok(Self {
            client,
            config,
            observer,
            allow_http_for_tests,
        })
    }

    #[cfg(test)]
    fn new_for_local_test(config: HttpDownloadConfig) -> Result<Self, ModelManagerError> {
        Self::build(config, None, true)
    }

    fn emit(&self, source: &ModelSource, phase: DownloadPhase, received: u64, total: Option<u64>) {
        if let Some(observer) = &self.observer {
            observer(DownloadProgress {
                source_kind: source.kind,
                phase,
                received_bytes: received,
                total_bytes: total,
            });
        }
    }

    fn validate_url(&self, raw: &str) -> Result<reqwest::Url, String> {
        let url = reqwest::Url::parse(raw).map_err(|_| "下载地址格式无效".to_string())?;
        let scheme_allowed =
            url.scheme() == "https" || (self.allow_http_for_tests && url.scheme() == "http");
        if !scheme_allowed {
            return Err("下载地址必须使用 HTTPS".into());
        }
        if !url.host_str().is_some_and(|host| !host.is_empty()) {
            return Err("下载地址缺少有效主机名".into());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("下载地址禁止包含鉴权信息".into());
        }
        Ok(url)
    }
}

fn url_is_allowed(url: &reqwest::Url, allow_http_for_tests: bool) -> bool {
    let scheme_allowed =
        url.scheme() == "https" || (allow_http_for_tests && url.scheme() == "http");
    scheme_allowed
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
}

impl SourceDownloader for HttpSourceDownloader {
    fn download(
        &self,
        source: &ModelSource,
        destination: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        if cancellation.is_cancelled() {
            return Err("cancelled".into());
        }
        let url = self.validate_url(&source.url)?;
        self.emit(source, DownloadPhase::Connecting, 0, None);
        let mut response = self
            .client
            .get(url)
            .send()
            .map_err(|_| "连接或读取下载源失败".to_string())?;
        if !response.status().is_success() {
            return Err(format!("下载源返回 HTTP {}", response.status().as_u16()));
        }
        let final_scheme_allowed = response.url().scheme() == "https"
            || (self.allow_http_for_tests && response.url().scheme() == "http");
        if !final_scheme_allowed {
            return Err("重定向后的下载地址不是 HTTPS".into());
        }
        if !response.url().username().is_empty() || response.url().password().is_some() {
            return Err("重定向后的下载地址包含鉴权信息".into());
        }
        let total = response.content_length();
        if total.is_some_and(|size| size > self.config.max_response_bytes) {
            return Err(format!(
                "Content-Length 超过 {} 字节上限",
                self.config.max_response_bytes
            ));
        }
        self.emit(source, DownloadPhase::Receiving, 0, total);
        let mut received = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            if cancellation.is_cancelled() {
                return Err("cancelled".into());
            }
            let count = response
                .read(&mut buffer)
                .map_err(|_| "读取模型响应失败".to_string())?;
            if count == 0 {
                break;
            }
            received = received
                .checked_add(count as u64)
                .ok_or_else(|| "响应大小溢出".to_string())?;
            if received > self.config.max_response_bytes {
                return Err(format!(
                    "响应内容超过 {} 字节上限",
                    self.config.max_response_bytes
                ));
            }
            destination
                .write_all(&buffer[..count])
                .map_err(|error| format!("写入临时文件失败: {error}"))?;
            self.emit(source, DownloadPhase::Receiving, received, total);
        }
        if total.is_some_and(|expected| expected != received) {
            return Err("实际响应大小与 Content-Length 不一致".into());
        }
        self.emit(source, DownloadPhase::Completed, received, total);
        Ok(())
    }
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
                if let Err(message) =
                    downloader.download(&attempt.source, &mut destination, cancellation)
                {
                    if cancellation.is_cancelled() {
                        return Err(ModelManagerError::Cancelled);
                    }
                    return Err(ModelManagerError::Io(std::io::Error::other(message)));
                }
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
                    // 错误摘要只记录源类型，不回显 URL，避免 URL 查询参数泄漏。
                    failures.push(format!("{:?}: {error}", attempt.source.kind));
                    let _ = fs::remove_file(part_path);
                }
            }
        }
        self.status = DownloadStatus::Failed;
        Err(ModelManagerError::AllSourcesFailed(failures))
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    fn mock_server(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        format!("http://{address}/model.zip")
    }

    #[test]
    fn http_downloader_streams_from_local_mock_and_reports_progress() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let mut downloader = HttpSourceDownloader::new_for_local_test(HttpDownloadConfig {
            max_response_bytes: 1024,
            ..HttpDownloadConfig::default()
        })
        .unwrap();
        downloader.observer = Some(Arc::new(move |event| captured.lock().unwrap().push(event)));
        let source = ModelSource {
            kind: SourceKind::Cdn,
            url: mock_server(b"model-package"),
        };
        let mut output = Vec::new();
        downloader
            .download(&source, &mut output, &CancellationToken::default())
            .unwrap();
        assert_eq!(output, b"model-package");
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.phase == DownloadPhase::Completed));
    }

    #[test]
    fn production_downloader_rejects_plain_http_before_network_access() {
        let downloader = HttpSourceDownloader::new(HttpDownloadConfig::default(), None).unwrap();
        let source = ModelSource {
            kind: SourceKind::GithubRelease,
            url: "http://127.0.0.1/model.zip".into(),
        };
        let error = downloader
            .download(&source, &mut Vec::new(), &CancellationToken::default())
            .unwrap_err();
        assert!(error.contains("HTTPS"));
    }

    #[test]
    fn production_downloader_rejects_credentials_in_url() {
        let downloader = HttpSourceDownloader::new(HttpDownloadConfig::default(), None).unwrap();
        let source = ModelSource {
            kind: SourceKind::Cdn,
            url: "https://user:secret@example.com/model.zip".into(),
        };
        let error = downloader
            .download(&source, &mut Vec::new(), &CancellationToken::default())
            .unwrap_err();
        assert!(error.contains("鉴权"));
        assert!(!error.contains("secret"));
    }

    #[test]
    fn redirect_targets_are_checked_before_following() {
        assert!(url_is_allowed(
            &reqwest::Url::parse("https://cdn.example/model.zip").unwrap(),
            false
        ));
        assert!(!url_is_allowed(
            &reqwest::Url::parse("http://cdn.example/model.zip").unwrap(),
            false
        ));
        assert!(!url_is_allowed(
            &reqwest::Url::parse("https://user:secret@cdn.example/model.zip").unwrap(),
            false
        ));
    }

    #[test]
    fn http_downloader_observes_cancellation_after_progress() {
        let cancellation = CancellationToken::default();
        let cancel_from_observer = cancellation.clone();
        let mut downloader = HttpSourceDownloader::new_for_local_test(HttpDownloadConfig {
            max_response_bytes: 1024,
            ..HttpDownloadConfig::default()
        })
        .unwrap();
        downloader.observer = Some(Arc::new(move |event| {
            if event.received_bytes > 0 {
                cancel_from_observer.cancel();
            }
        }));
        let source = ModelSource {
            kind: SourceKind::Cdn,
            url: mock_server(b"model-package"),
        };
        let error = downloader
            .download(&source, &mut Vec::new(), &cancellation)
            .unwrap_err();
        assert_eq!(error, "cancelled");
    }

    #[test]
    fn download_plan_preserves_transport_cancellation_semantics() {
        struct CancellingDownloader;
        impl SourceDownloader for CancellingDownloader {
            fn download(
                &self,
                _source: &ModelSource,
                _destination: &mut dyn Write,
                cancellation: &CancellationToken,
            ) -> Result<(), String> {
                cancellation.cancel();
                Err("cancelled".into())
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("model.zip.part");
        let source = ModelSource {
            kind: SourceKind::Cdn,
            url: "https://cdn.example/model.zip".into(),
        };
        let package = PackageDescriptor {
            size_bytes: 1,
            sha256: "0".repeat(64),
            sources: vec![source.clone()],
        };
        let cancellation = CancellationToken::default();
        let mut plan = DownloadPlan::new(&[source]);
        let error = plan
            .execute(&CancellingDownloader, &part_path, &package, &cancellation)
            .unwrap_err();
        assert!(matches!(error, ModelManagerError::Cancelled));
        assert_eq!(plan.status, DownloadStatus::Cancelled);
        assert!(!part_path.exists());
    }

    #[test]
    fn http_downloader_rejects_oversized_content_length() {
        let downloader = HttpSourceDownloader::new_for_local_test(HttpDownloadConfig {
            max_response_bytes: 4,
            ..HttpDownloadConfig::default()
        })
        .unwrap();
        let source = ModelSource {
            kind: SourceKind::Cdn,
            url: mock_server(b"too-large"),
        };
        let error = downloader
            .download(&source, &mut Vec::new(), &CancellationToken::default())
            .unwrap_err();
        assert!(error.contains("Content-Length"));
    }
}
