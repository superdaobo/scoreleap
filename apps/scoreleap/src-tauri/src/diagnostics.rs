use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

use crate::model::{self, ModelState};

/// 转录环境诊断：一次返回所有关键路径与状态，前端/日志直接可读。
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionDiagnostics {
    pub model_status: String,
    pub model_configured: bool,
    pub model_installed_version: Option<String>,
    pub model_path: Option<String>,
    pub model_file_exists: bool,
    pub sidecar_exe_path: Option<String>,
    pub sidecar_exe_exists: bool,
    pub onnx_runtime_path: Option<String>,
    pub onnx_runtime_exists: bool,
    pub onnx_runtime_version: Option<String>,
    pub jobs_dir: Option<String>,
    pub jobs_dir_writable: bool,
    pub app_data_dir: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn diagnose_transcription(
    app: AppHandle,
    state: State<'_, ModelState>,
) -> TranscriptionDiagnostics {
    let status = model::model_status(&app, &state);
    let model_path = model::resolve_active_model(&app).ok();
    let sidecar_exe = model::resolve_packaged_file(
        &app,
        std::path::Path::new("scoreleap-transcriber/scoreleap-transcriber-native.exe"),
        "SCORELEAP_SIDECAR_PATH",
    );
    let onnx_runtime = model::resolve_packaged_file(
        &app,
        std::path::Path::new("scoreleap-transcriber/onnxruntime.dll"),
        "SCORELEAP_ONNX_RUNTIME",
    );
    let app_data = app.path().app_data_dir().ok();
    let jobs_dir = app_data.as_ref().map(|d| d.join("jobs"));
    let jobs_writable = jobs_dir
        .as_ref()
        .map(|d| {
            if std::fs::create_dir_all(d).is_err() {
                return false;
            }
            let probe = d.join(".probe");
            let ok = std::fs::write(&probe, b"").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        })
        .unwrap_or(false);
    let dll_version = onnx_runtime
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| format!("{} bytes", m.len()));

    let diag = TranscriptionDiagnostics {
        model_status: status.status.clone(),
        model_configured: status.configured,
        model_installed_version: status.installed_version.clone(),
        model_path: model_path.as_ref().map(|p| p.display().to_string()),
        model_file_exists: model_path.as_ref().map(|p| p.is_file()).unwrap_or(false),
        sidecar_exe_path: sidecar_exe.as_ref().map(|p| p.display().to_string()),
        sidecar_exe_exists: sidecar_exe.as_ref().map(|p| p.is_file()).unwrap_or(false),
        onnx_runtime_path: onnx_runtime.as_ref().map(|p| p.display().to_string()),
        onnx_runtime_exists: onnx_runtime.as_ref().map(|p| p.is_file()).unwrap_or(false),
        onnx_runtime_version: dll_version,
        jobs_dir: jobs_dir.as_ref().map(|d| d.display().to_string()),
        jobs_dir_writable: jobs_writable,
        app_data_dir: app_data.as_ref().map(|d| d.display().to_string()),
        error: status.error.clone(),
    };
    tracing::info!(
        model_status = %diag.model_status,
        model_configured = diag.model_configured,
        model_path = ?diag.model_path,
        sidecar = ?diag.sidecar_exe_path,
        runtime = ?diag.onnx_runtime_path,
        jobs_writable = diag.jobs_dir_writable,
        "转录环境诊断"
    );
    diag
}
