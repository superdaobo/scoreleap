use std::path::{Path, PathBuf};
use std::sync::Mutex;

use scoreleap_model_manager::{
<<<<<<< HEAD
    verifying_key_from_hex, CancellationToken, DownloadPhase, HttpDownloadConfig,
    HttpSourceDownloader, ModelManager, ModelManagerError, SignedModelCatalog, SignedModelManifest,
    SourceKind,
=======
    compare_version, verifying_key_from_hex, CancellationToken, DownloadPhase, ExtractionLimits,
    HttpDownloadConfig, HttpSourceDownloader, ModelManager, ModelManagerError, SignedModelCatalog,
    SignedModelManifest, SourceKind,
>>>>>>> feat/onnx-product-integration
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const MODEL_ID: &str = "basic-pitch";
const ENGINE_ID: &str = "scoreleap-onnx";
<<<<<<< HEAD
=======
const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MODEL_PACKAGE_BYTES: u64 = 1024 * 1024 * 1024;
>>>>>>> feat/onnx-product-integration

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatusView {
    pub status: String,
    pub configured: bool,
    pub model_id: String,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub size_bytes: Option<u64>,
    pub source: Option<String>,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub can_rollback: bool,
}

impl ModelStatusView {
    fn missing_configuration(message: String) -> Self {
        Self {
            status: "configuration_missing".into(),
            configured: false,
            model_id: MODEL_ID.into(),
            installed_version: None,
            latest_version: None,
            size_bytes: None,
            source: None,
            received_bytes: 0,
            total_bytes: None,
            error: Some(message),
            can_rollback: false,
        }
    }
}

#[derive(Default)]
struct RuntimeModelState {
    downloading: bool,
    cancellation: Option<CancellationToken>,
    received_bytes: u64,
    total_bytes: Option<u64>,
    source: Option<String>,
    error: Option<String>,
}

pub struct ModelState(Mutex<RuntimeModelState>);

impl Default for ModelState {
    fn default() -> Self {
        Self(Mutex::new(RuntimeModelState::default()))
    }
}

struct ModelContext {
    manager: ModelManager,
    latest: SignedModelManifest,
    model_root: PathBuf,
}

fn source_label(kind: &SourceKind) -> String {
    match kind {
        SourceKind::Cdn => "CDN".into(),
        SourceKind::GithubRelease => "GitHub Releases".into(),
    }
}

fn trusted_config_paths(app: &AppHandle) -> Result<(PathBuf, PathBuf), ModelManagerError> {
    // 开发环境仅在显式设置时接受外部路径；发布版始终使用安装资源中的信任配置。
    #[cfg(debug_assertions)]
    {
        let catalog = std::env::var("SCORELEAP_MODEL_CATALOG_PATH").ok();
        let public_key = std::env::var("SCORELEAP_MODEL_PUBLIC_KEY_PATH").ok();
        match (catalog, public_key) {
            (Some(catalog), Some(public_key)) => {
                return Ok((PathBuf::from(catalog), PathBuf::from(public_key)));
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(ModelManagerError::TrustConfigurationMissing(
                    "开发覆盖必须同时设置 SCORELEAP_MODEL_CATALOG_PATH 和 SCORELEAP_MODEL_PUBLIC_KEY_PATH"
                        .into(),
                ));
            }
            (None, None) => {}
        }
    }
    let resource = app.path().resource_dir().map_err(|error| {
        ModelManagerError::TrustConfigurationMissing(format!("无法解析应用资源目录: {error}"))
    })?;
<<<<<<< HEAD
    // 兼容两种布局：exe 目录直接放资源（开发/单文件），或 resources/ 子目录（安装包）。
    let candidates = [
        resource.join("scoreleap-model"),
        resource.join("resources/scoreleap-model"),
    ];
    let model_dir = candidates
        .into_iter()
        .find(|dir| dir.join("catalog.signed.json").is_file() && dir.join("public-key.hex").is_file())
        .ok_or_else(|| {
            ModelManagerError::TrustConfigurationMissing(
                "缺少 catalog.signed.json 或 public-key.hex；发布流程必须注入可信配置".into(),
            )
        })?;
    Ok((model_dir.join("catalog.signed.json"), model_dir.join("public-key.hex")))
=======
    Ok((
        resource.join("scoreleap-model/catalog.signed.json"),
        resource.join("scoreleap-model/public-key.hex"),
    ))
>>>>>>> feat/onnx-product-integration
}

fn load_context(app: &AppHandle) -> Result<ModelContext, ModelManagerError> {
    let (catalog_path, public_key_path) = trusted_config_paths(app)?;
    if !catalog_path.is_file() || !public_key_path.is_file() {
        return Err(ModelManagerError::TrustConfigurationMissing(
            "缺少 catalog.signed.json 或 public-key.hex；发布流程必须注入可信配置".into(),
        ));
    }
<<<<<<< HEAD
=======
    if std::fs::metadata(&catalog_path)?.len() > MAX_CATALOG_BYTES {
        return Err(ModelManagerError::InvalidManifest(
            "签名模型目录超过 4MB 安全上限".into(),
        ));
    }
>>>>>>> feat/onnx-product-integration
    let public_key = std::fs::read_to_string(public_key_path)?;
    let verifying_key = verifying_key_from_hex(&public_key)?;
    let catalog: SignedModelCatalog = serde_json::from_reader(std::fs::File::open(catalog_path)?)?;
    let model_root = app
        .path()
        .app_data_dir()
        .map_err(|error| ModelManagerError::CacheInvalid(error.to_string()))?
        .join("models");
    let manager = ModelManager::new(
        &model_root,
        verifying_key,
        ENGINE_ID,
        env!("CARGO_PKG_VERSION"),
<<<<<<< HEAD
    );
=======
    )
    .with_limits(ExtractionLimits {
        max_files: 32,
        max_single_file_bytes: MAX_MODEL_PACKAGE_BYTES,
        max_total_uncompressed_bytes: MAX_MODEL_PACKAGE_BYTES,
    });
>>>>>>> feat/onnx-product-integration
    let latest = manager
        .latest_from_catalog(&catalog, MODEL_ID)?
        .ok_or_else(|| {
            ModelManagerError::InvalidManifest("目录中没有与当前引擎兼容的模型".into())
        })?;
<<<<<<< HEAD
=======
    if latest.manifest.package.size_bytes > MAX_MODEL_PACKAGE_BYTES {
        return Err(ModelManagerError::InvalidManifest(
            "模型包超过 1GB 安全上限".into(),
        ));
    }
>>>>>>> feat/onnx-product-integration
    Ok(ModelContext {
        manager,
        latest,
        model_root,
    })
}

fn base_status(context: &ModelContext) -> Result<ModelStatusView, ModelManagerError> {
    let latest = &context.latest.manifest;
    let active = context.manager.active_version(MODEL_ID)?;
    let installed_version = match active.as_ref() {
        Some(active) => {
            context.manager.validate_cached(MODEL_ID, &active.version)?;
            Some(active.version.clone())
        }
        None => None,
    };
    let can_rollback = context
        .manager
        .last_good_version(MODEL_ID)?
        .is_some_and(|version| {
            context
                .manager
                .validate_cached(MODEL_ID, &version.version)
                .is_ok()
        });
    let status = match &installed_version {
        None => "not_installed",
<<<<<<< HEAD
        Some(version) if version != &latest.version => "update_available",
=======
        Some(version) if catalog_version_is_newer(version, &latest.version)? => "update_available",
>>>>>>> feat/onnx-product-integration
        Some(_) => "ready",
    };
    Ok(ModelStatusView {
        status: status.into(),
        configured: true,
        model_id: MODEL_ID.into(),
        installed_version,
        latest_version: Some(latest.version.clone()),
        size_bytes: Some(latest.package.size_bytes),
        source: latest
            .package
            .sources
            .first()
            .map(|source| source_label(&source.kind)),
        received_bytes: 0,
        total_bytes: Some(latest.package.size_bytes),
        error: None,
        can_rollback,
    })
}

<<<<<<< HEAD
=======
fn catalog_version_is_newer(
    installed_version: &str,
    catalog_version: &str,
) -> Result<bool, ModelManagerError> {
    Ok(compare_version(installed_version, catalog_version)?.is_lt())
}

>>>>>>> feat/onnx-product-integration
pub fn model_status(app: &AppHandle, state: &ModelState) -> ModelStatusView {
    let mut status = match load_context(app) {
        Ok(context) => match base_status(&context) {
            Ok(status) => status,
            Err(error) => {
                let mut status = ModelStatusView::missing_configuration(error.to_string());
                status.status = "failed".into();
                status.configured = true;
<<<<<<< HEAD
                status.latest_version = Some(context.latest.manifest.version.clone());
                status.size_bytes = Some(context.latest.manifest.package.size_bytes);
=======
                status.installed_version = context
                    .manager
                    .active_version(MODEL_ID)
                    .ok()
                    .flatten()
                    .map(|version| version.version);
                status.latest_version = Some(context.latest.manifest.version.clone());
                status.size_bytes = Some(context.latest.manifest.package.size_bytes);
                status.can_rollback = context
                    .manager
                    .last_good_version(MODEL_ID)
                    .ok()
                    .flatten()
                    .is_some_and(|version| {
                        context
                            .manager
                            .validate_cached(MODEL_ID, &version.version)
                            .is_ok()
                    });
>>>>>>> feat/onnx-product-integration
                status
            }
        },
        Err(error) => ModelStatusView::missing_configuration(error.to_string()),
    };
    let runtime = state.0.lock().unwrap();
    if runtime.downloading {
        status.status = "downloading".into();
    } else if runtime.error.is_some() && status.configured {
        status.status = "failed".into();
    }
    status.received_bytes = runtime.received_bytes;
    status.total_bytes = runtime.total_bytes.or(status.total_bytes);
    status.source = runtime.source.clone().or(status.source);
    status.error = runtime.error.clone().or(status.error);
<<<<<<< HEAD
    tracing::info!(
        status = %status.status,
        configured = status.configured,
        installed = ?status.installed_version,
        latest = ?status.latest_version,
        model_error = ?status.error,
        "模型状态"
    );
=======
>>>>>>> feat/onnx-product-integration
    status
}

pub fn start_download(app: AppHandle, state: &ModelState) -> Result<(), String> {
    let context = load_context(&app).map_err(|error| error.to_string())?;
<<<<<<< HEAD
=======
    if let Some(active) = context
        .manager
        .active_version(MODEL_ID)
        .map_err(|error| error.to_string())?
    {
        let active_is_valid = context
            .manager
            .validate_cached(MODEL_ID, &active.version)
            .is_ok();
        let latest_is_newer =
            catalog_version_is_newer(&active.version, &context.latest.manifest.version)
                .map_err(|error| error.to_string())?;
        if active_is_valid && !latest_is_newer {
            return Err("当前已安装相同或更新的有效模型，无需下载".into());
        }
    }
>>>>>>> feat/onnx-product-integration
    let cancellation = CancellationToken::default();
    {
        let mut runtime = state.0.lock().unwrap();
        if runtime.downloading {
            return Err("模型下载已在进行中".into());
        }
        runtime.downloading = true;
        runtime.cancellation = Some(cancellation.clone());
        runtime.received_bytes = 0;
        runtime.total_bytes = Some(context.latest.manifest.package.size_bytes);
        runtime.source = None;
        runtime.error = None;
    }
    let _ = app.emit("model://state", model_status(&app, state));
    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let progress_app = app_for_thread.clone();
        let observer =
            std::sync::Arc::new(move |progress: scoreleap_model_manager::DownloadProgress| {
                let state = progress_app.state::<ModelState>();
                {
                    let mut runtime = state.0.lock().unwrap();
                    runtime.received_bytes = progress.received_bytes;
                    runtime.total_bytes = progress.total_bytes.or(runtime.total_bytes);
                    runtime.source = Some(source_label(&progress.source_kind));
                }
                let payload = serde_json::json!({
                    "phase": match progress.phase {
                        DownloadPhase::Connecting => "connecting",
                        DownloadPhase::Receiving => "receiving",
                        DownloadPhase::Completed => "completed",
                    },
                    "received_bytes": progress.received_bytes,
                    "total_bytes": progress.total_bytes,
                    "source": source_label(&progress.source_kind),
                });
                let _ = progress_app.emit("model://progress", payload);
            });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let downloader = HttpSourceDownloader::new(
                HttpDownloadConfig {
                    max_response_bytes: context.latest.manifest.package.size_bytes,
                    ..HttpDownloadConfig::default()
                },
                Some(observer),
            )
            .map_err(|error| error.to_string())?;
            context
                .manager
                .install(&context.latest, &downloader, &cancellation)
                .map_err(|error| error.to_string())?;
<<<<<<< HEAD
=======
            if cancellation.is_cancelled() {
                return Err(ModelManagerError::Cancelled.to_string());
            }
>>>>>>> feat/onnx-product-integration
            context
                .manager
                .activate(MODEL_ID, &context.latest.manifest.version)
                .map_err(|error| error.to_string())
        }))
        .unwrap_or_else(|_| Err("模型下载线程异常终止".into()));
        let state = app_for_thread.state::<ModelState>();
        {
            let mut runtime = state.0.lock().unwrap();
            runtime.downloading = false;
            runtime.cancellation = None;
            runtime.error = if cancellation.is_cancelled() {
                None
            } else {
                result.err()
            };
        }
        let _ = app_for_thread.emit("model://state", model_status(&app_for_thread, &state));
    });
    Ok(())
}

pub fn cancel_download(state: &ModelState) -> Result<(), String> {
    let runtime = state.0.lock().unwrap();
    let cancellation = runtime
        .cancellation
        .as_ref()
        .ok_or_else(|| "当前没有模型下载任务".to_string())?;
    cancellation.cancel();
    Ok(())
}

pub fn rollback(app: &AppHandle, state: &ModelState) -> Result<ModelStatusView, String> {
<<<<<<< HEAD
    if state.0.lock().unwrap().downloading {
        return Err("模型下载期间不能回滚".into());
    }
=======
    // 在检查状态到提交回滚期间持续持有操作锁，避免下载激活与回滚交叉写指针。
    let mut runtime = acquire_rollback_operation(state)?;
>>>>>>> feat/onnx-product-integration
    let context = load_context(app).map_err(|error| error.to_string())?;
    context
        .manager
        .rollback(MODEL_ID)
        .map_err(|error| error.to_string())?;
<<<<<<< HEAD
    state.0.lock().unwrap().error = None;
    Ok(model_status(app, state))
}

pub fn resolve_active_model(app: &AppHandle) -> Result<PathBuf, String> {
    let context = load_context(app).map_err(|error| error.to_string())?;
    let active = context
        .manager
        .active_version(MODEL_ID)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "尚未安装转录模型".to_string())?;
    let manifest = context
        .manager
        .validate_cached(MODEL_ID, &active.version)
        .map_err(|error| error.to_string())?;
=======
    runtime.error = None;
    drop(runtime);
    Ok(model_status(app, state))
}

fn acquire_rollback_operation(
    state: &ModelState,
) -> Result<std::sync::MutexGuard<'_, RuntimeModelState>, String> {
    let runtime = state.0.lock().unwrap();
    if runtime.downloading {
        return Err("模型下载期间不能回滚".into());
    }
    Ok(runtime)
}

pub fn resolve_active_model(app: &AppHandle) -> Result<PathBuf, ModelManagerError> {
    let context = load_context(app)?;
    let active = context
        .manager
        .active_version(MODEL_ID)?
        .ok_or_else(|| ModelManagerError::CacheMissing(MODEL_ID.into(), "active".into()))?;
    let manifest = context.manager.validate_cached(MODEL_ID, &active.version)?;
>>>>>>> feat/onnx-product-integration
    let relative = manifest
        .manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.path.to_ascii_lowercase().ends_with(".onnx"))
<<<<<<< HEAD
        .ok_or_else(|| "模型包缺少 ONNX 文件".to_string())?;
=======
        .ok_or_else(|| ModelManagerError::CacheInvalid("模型包缺少 ONNX 文件".into()))?;
>>>>>>> feat/onnx-product-integration
    let path = context
        .manager
        .version_path(MODEL_ID, &active.version)
        .join(&relative.path);
<<<<<<< HEAD
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    let root = context
        .model_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err("模型路径越出应用数据目录".into());
=======
    let canonical = path.canonicalize()?;
    let root = context.model_root.canonicalize()?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(ModelManagerError::CacheInvalid(
            "模型路径越出应用数据目录".into(),
        ));
>>>>>>> feat/onnx-product-integration
    }
    Ok(canonical)
}

pub fn resolve_packaged_file(
    app: &AppHandle,
    relative: &Path,
    development_variable: &str,
) -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Ok(path) = std::env::var(development_variable) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path.canonicalize().ok();
        }
    }
<<<<<<< HEAD
    let resource = app.path().resource_dir().ok()?;
    let root = resource.canonicalize().unwrap_or_else(|_| resource.clone());
    for base in [&resource, &resource.join("resources")] {
        // 注意：不能对 canonicalize 用 `?`（第一个 base 不存在时会提前返回，
        // 永远到不了 resources/ 子目录）。逐个尝试，跳过不存在的 base。
        let candidate = base.join(relative);
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.starts_with(&root) && canonical.is_file() {
                return Some(canonical);
            }
        }
    }
    None
=======
    let root = app.path().resource_dir().ok()?.canonicalize().ok()?;
    let candidate = root.join(relative).canonicalize().ok()?;
    (candidate.starts_with(&root) && candidate.is_file()).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_comparison_never_treats_same_or_older_version_as_update() {
        assert!(catalog_version_is_newer("1.2.9", "1.10.0").unwrap());
        assert!(!catalog_version_is_newer("1.10.0", "1.10.0").unwrap());
        assert!(!catalog_version_is_newer("2.0.0", "1.10.0").unwrap());
    }

    #[test]
    fn rollback_operation_rejects_download_and_holds_exclusive_state_lock() {
        let downloading = ModelState::default();
        downloading.0.lock().unwrap().downloading = true;
        assert!(acquire_rollback_operation(&downloading).is_err());

        let idle = ModelState::default();
        let guard = acquire_rollback_operation(&idle).unwrap();
        assert!(matches!(
            idle.0.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        drop(guard);
        assert!(idle.0.try_lock().is_ok());
    }
>>>>>>> feat/onnx-product-integration
}
