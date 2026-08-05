//! ScoreLeap Tauri 应用入口（命令适配层）。
//!
//! 注意：命令函数**必须为非 pub**——tauri-macros 对 pub 命令生成
//! `#[macro_export]` + `pub use`，与 rustc 的宏命名空间检查冲突（E0255）。

mod diagnostics;
mod model;

use std::sync::{Arc, Mutex};

use scoreleap_core::{AppState, CoreError};
use scoreleap_sequence::PlaybackState;
use scoreleap_transcription::{
    TranscriptionError, TranscriptionErrorCode, TranscriptionEvent, TranscriptionOptions,
    TranscriptionService, WorkerSpec,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// 初始化 tracing 日志（stderr 不可见时写入应用数据目录 logs/，便于诊断）。
fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,scoreleap=info"));

    let log_dir = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("com.superdaobo.scoreleap")
        .join("logs");
    {
        let dir = &log_dir;
        let _ = std::fs::create_dir_all(dir);
        // 清理旧日志：保留最近 10 个
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for old in files.iter().take(files.len().saturating_sub(10)) {
            let _ = std::fs::remove_file(old.path());
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("scoreleap-{ts}.log"));
        if let Ok(file) = std::fs::File::create(&path) {
            tracing::info!("日志文件: {}", path.display());
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false)
                .with_writer(std::sync::Mutex::new(file))
                .try_init();
            return;
        }
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

/// 崩溃标记文件路径（异常退出自检）。
fn crash_flag_path() -> std::path::PathBuf {
    std::env::temp_dir().join("scoreleap-crash-flag")
}

/// 注册 panic hook：记录日志 + 写崩溃标记（尽力释放按键由调度器句柄在窗口销毁时处理）。
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        eprintln!("ScoreLeap panic: {info}");
        let _ = std::fs::write(crash_flag_path(), "crash");
    }));
}

#[tauri::command]
fn import_midi(
    state: State<'_, AppState>,
    path: String,
) -> Result<scoreleap_core::ImportSummary, CoreError> {
    scoreleap_core::import_midi(&state, path)
}

#[tauri::command]
fn list_documents(
    state: State<'_, AppState>,
) -> Result<Vec<scoreleap_core::DocumentSummary>, CoreError> {
    scoreleap_core::list_documents(&state)
}

#[tauri::command]
fn get_sequence_notes(
    state: State<'_, AppState>,
    seq_id: String,
) -> Result<Vec<scoreleap_core::NoteView>, CoreError> {
    scoreleap_core::get_sequence_notes(&state, seq_id)
}

#[tauri::command]
fn get_tracks(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<Vec<scoreleap_core::TrackSummary>, CoreError> {
    scoreleap_core::get_tracks(&state, doc_id)
}

#[tauri::command]
fn compile(
    state: State<'_, AppState>,
    doc_id: String,
    enabled_tracks: Vec<u16>,
    options: scoreleap_arranger::ArrangementOptions,
) -> Result<scoreleap_core::CompileSummary, CoreError> {
    scoreleap_core::compile(&state, doc_id, enabled_tracks, options)
}

#[tauri::command]
fn start_playback(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    seq_id: String,
    backend: String,
) -> Result<scoreleap_core::PlaybackStatus, CoreError> {
    scoreleap_core::start_playback(&app, &state, seq_id, backend)
}

#[tauri::command]
fn pause_playback(state: State<'_, AppState>) -> Result<(), CoreError> {
    scoreleap_core::pause_playback(&state)
}

#[tauri::command]
fn resume_playback(state: State<'_, AppState>) -> Result<(), CoreError> {
    scoreleap_core::resume_playback(&state)
}

#[tauri::command]
fn stop_playback(state: State<'_, AppState>) -> Result<(), CoreError> {
    scoreleap_core::stop_playback(&state)
}

#[tauri::command]
fn emergency_stop(state: State<'_, AppState>) -> Result<(), CoreError> {
    scoreleap_core::emergency_stop(&state)
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<String>, CoreError> {
    scoreleap_core::list_profiles(&state)
}

#[tauri::command]
fn load_profile(
    state: State<'_, AppState>,
    id: String,
) -> Result<scoreleap_music_ir::GameProfile, CoreError> {
    scoreleap_core::load_profile(&state, id)
}

#[tauri::command]
fn current_profile(
    state: State<'_, AppState>,
) -> Result<Option<scoreleap_music_ir::GameProfile>, CoreError> {
    scoreleap_core::current_profile(&state)
}

#[tauri::command]
fn get_crash_flag(state: State<'_, AppState>) -> bool {
    *state.crash_flag.lock().unwrap()
}

#[tauri::command]
fn test_key(scan: u16) -> Result<String, CoreError> {
    scoreleap_core::test_key(scan)
}

#[tauri::command]
fn check_foreground() -> Result<scoreleap_core::ForegroundInfo, CoreError> {
    scoreleap_core::check_foreground()
}

#[tauri::command]
fn list_keymap(state: State<'_, AppState>) -> Result<Vec<scoreleap_core::KeymapEntry>, CoreError> {
    scoreleap_core::list_keymap(&state)
}

#[tauri::command]
fn get_audio_file_info(path: String) -> Result<scoreleap_core::AudioFileInfo, CoreError> {
    scoreleap_core::audio_file_info(&path)
}

/// 转录服务状态（惰性创建；终态任务保留供前端查询）。
struct TxState(Mutex<Option<TranscriptionService>>);

const NATIVE_WORKER_RESOURCE: &str = "scoreleap-transcriber/scoreleap-transcriber-native.exe";

/// 原生 sidecar 路径解析：发布版仅接受资源目录，开发版允许显式环境变量覆盖。
fn resolve_worker_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    model::resolve_packaged_file(
        app,
        std::path::Path::new(NATIVE_WORKER_RESOURCE),
        "SCORELEAP_WORKER_PATH",
    )
}

fn resolve_onnx_runtime_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    model::resolve_packaged_file(
        app,
        std::path::Path::new("scoreleap-transcriber/onnxruntime.dll"),
        "SCORELEAP_ONNXRUNTIME_PATH",
    )
}

fn get_or_init_transcription(
    app: &AppHandle,
    tx: &State<'_, TxState>,
) -> Result<TranscriptionService, TranscriptionError> {
    let mut guard = tx.0.lock().unwrap();
    if let Some(svc) = guard.as_ref() {
        match svc.status() {
            Some(job) if job.status.is_terminal() => {
                // 终态任务后重建服务，以便下一次启动读取新激活的模型。
            }
            _ => return Ok(svc.clone()),
        }
    }
    // 解析 Worker 路径
    let worker_program = resolve_worker_path(app).ok_or_else(|| {
        TranscriptionError::new(
            scoreleap_transcription::TranscriptionErrorCode::WorkerNotFound,
            "未找到原生转录组件，请重新安装完整版本",
        )
    })?;
    let runtime_path = resolve_onnx_runtime_path(app).ok_or_else(|| {
        TranscriptionError::new(
            TranscriptionErrorCode::RuntimeMissing,
            "未找到 ONNX Runtime，请重新安装完整版本",
        )
    })?;
    let model_path = model::resolve_active_model(app).map_err(|error| {
        let code = match &error {
            scoreleap_model_manager::ModelManagerError::CacheMissing(_, _) => {
                TranscriptionErrorCode::ModelDownloadRequired
            }
            _ => TranscriptionErrorCode::ModelLoadFailed,
        };
        TranscriptionError::new(code, error.to_string())
    })?;
    tracing::info!("转录 Worker: {}", worker_program.display());
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("transcriptions");
    let handle = app.clone();
    let on_event = Arc::new(move |event: TranscriptionEvent| {
        let (name, payload) = match event {
            TranscriptionEvent::State { job_id, status } => (
                "transcription://state",
                serde_json::json!({ "job_id": job_id, "status": status }),
            ),
            TranscriptionEvent::Stage {
                job_id,
                stage,
                message,
            } => (
                "transcription://stage",
                serde_json::json!({ "job_id": job_id, "stage": stage, "message": message }),
            ),
            TranscriptionEvent::Completed {
                job_id,
                doc_id,
                midi_path,
                note_count,
                elapsed_ms,
            } => (
                "transcription://completed",
                serde_json::json!({ "job_id": job_id, "doc_id": doc_id, "midi_path": midi_path, "note_count": note_count, "elapsed_ms": elapsed_ms }),
            ),
            TranscriptionEvent::Error {
                job_id,
                code,
                message,
            } => (
                "transcription://error",
                serde_json::json!({ "job_id": job_id, "code": code, "message": message }),
            ),
        };
        let _ = handle.emit(name, payload);
    });
    let handle2 = app.clone();
    let importer = Arc::new(
        move |midi_path: &str, display_name: &str| -> Result<String, String> {
            let core_state = handle2.state::<AppState>();
            scoreleap_core::import_midi_from_path(
                &core_state,
                midi_path,
                display_name,
                "audio_transcription",
            )
            .map(|s| s.doc_id)
            .map_err(|e| e.to_string())
        },
    );
    let svc = TranscriptionService::new(
        data_dir,
        WorkerSpec {
            program: worker_program.to_string_lossy().to_string(),
            args: vec![],
            model_path,
            onnx_runtime_path: Some(runtime_path),
        },
        on_event,
        importer,
    );
    *guard = Some(svc.clone());
    Ok(svc)
}

/// 启动音频转录（返回 job_id；单任务并发）。
#[tauri::command]
fn start_audio_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
    tx: State<'_, TxState>,
    path: String,
    options: TranscriptionOptions,
) -> Result<String, TranscriptionError> {
    let _ = &state;
    let svc = get_or_init_transcription(&app, &tx)?;
    svc.start_with_options(&path, options)
}

#[tauri::command]
fn get_transcription_model_status(
    app: AppHandle,
    state: State<'_, model::ModelState>,
) -> model::ModelStatusView {
    model::model_status(&app, &state)
}

/// 重新读取并验证签名目录；不会在用户确认前下载模型。
#[tauri::command]
fn check_transcription_model_update(
    app: AppHandle,
    state: State<'_, model::ModelState>,
) -> model::ModelStatusView {
    model::model_status(&app, &state)
}

#[tauri::command]
fn download_transcription_model(
    app: AppHandle,
    state: State<'_, model::ModelState>,
) -> Result<(), String> {
    model::start_download(app, &state)
}

#[tauri::command]
fn cancel_transcription_model_download(state: State<'_, model::ModelState>) -> Result<(), String> {
    model::cancel_download(&state)
}

#[tauri::command]
fn rollback_transcription_model(
    app: AppHandle,
    state: State<'_, model::ModelState>,
) -> Result<model::ModelStatusView, String> {
    model::rollback(&app, &state)
}

/// 取消当前转录任务。
#[tauri::command]
fn cancel_audio_transcription(
    app: AppHandle,
    tx: State<'_, TxState>,
) -> Result<(), TranscriptionError> {
    let _ = &app;
    let guard = tx.0.lock().unwrap();
    match guard.as_ref() {
        Some(svc) => svc.cancel(),
        None => Err(TranscriptionError::new(
            scoreleap_transcription::TranscriptionErrorCode::JobCancelled,
            "没有进行中的转录任务",
        )),
    }
}

/// 当前转录任务状态（终态任务保留，供前端展示）。
#[tauri::command]
fn get_audio_transcription_status(
    tx: State<'_, TxState>,
) -> Option<scoreleap_transcription::TranscriptionJob> {
    tx.0.lock().unwrap().as_ref().and_then(|svc| svc.status())
}

pub fn run() {
    init_logging();
    install_panic_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_scoreleap_input::init())
        .manage(AppState::default())
        .manage(TxState(Mutex::new(None)))
        .manage(model::ModelState::default())
        .setup(|app| {
            // Profile 目录查找优先级：资源目录（安装包内置）→ 仓库开发目录 → 用户数据目录
            let mut profiles_dir: Option<std::path::PathBuf> = None;
            let candidates = [
                app.path()
                    .resource_dir()
                    .ok()
                    .map(|d| d.join("game-profiles")),
                Some(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../game-profiles"),
                ),
                app.path()
                    .app_data_dir()
                    .ok()
                    .map(|d| d.join("game-profiles")),
            ];
            for c in candidates.into_iter().flatten() {
                if c.join("identity-v/profile.json").exists() {
                    tracing::info!("使用 Profile 目录: {}", c.display());
                    profiles_dir = Some(c);
                    break;
                }
            }
            let profiles_dir = profiles_dir.unwrap_or_else(|| {
                let data_dir = app
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                let p = data_dir.join("game-profiles");
                let _ = std::fs::create_dir_all(&p);
                p
            });
            *app.state::<AppState>().profile_dir.lock().unwrap() = profiles_dir;

            // 曲谱库目录：<app_data>/library
            let library_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("library");
            let _ = std::fs::create_dir_all(&library_dir);
            *app.state::<AppState>().library_dir.lock().unwrap() = library_dir;

            // 启动自检：检测上次会话异常退出标记
            let crashed = crash_flag_path().exists();
            *app.state::<AppState>().crash_flag.lock().unwrap() = crashed;
            if crashed {
                tracing::warn!("检测到上次会话异常退出；若游戏内按键卡住请重启游戏");
            }

            // 注册紧急停止全局快捷键：Ctrl+Alt+F9（避开系统保留组合）
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::F9);
            let _ = app
                .global_shortcut()
                .on_shortcut(shortcut, |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        tracing::info!("全局快捷键触发：紧急停止");
                        let state = app.state::<AppState>();
                        scoreleap_core::shutdown_scheduler(&state);
                        let _ = app.emit("playback://state", PlaybackState::Stopped);
                    }
                });

            // 启动时记录模型状态（便于诊断“模型无法下载/配置缺失”类问题）
            {
                let model_state = app.state::<model::ModelState>();
                let _status = model::model_status(app.handle(), &model_state);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // 窗口销毁时确保调度器关闭（释放按键）并清除崩溃标记（正常退出）
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                scoreleap_core::shutdown_scheduler(&state);
                let _ = std::fs::remove_file(crash_flag_path());
            }
        })
        .invoke_handler(tauri::generate_handler![
            import_midi,
            list_documents,
            get_tracks,
            get_sequence_notes,
            compile,
            start_playback,
            diagnostics::diagnose_transcription,
            pause_playback,
            resume_playback,
            stop_playback,
            emergency_stop,
            list_profiles,
            load_profile,
            current_profile,
            get_crash_flag,
            test_key,
            check_foreground,
            list_keymap,
            start_audio_transcription,
            cancel_audio_transcription,
            get_audio_transcription_status,
            get_audio_file_info,
            get_transcription_model_status,
            check_transcription_model_update,
            download_transcription_model,
            cancel_transcription_model_download,
            rollback_transcription_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod native_resource_contract_tests {
    use super::NATIVE_WORKER_RESOURCE;
    use std::path::Path;

    #[test]
    fn worker_resource_name_matches_native_packaging_contract() {
        let path = Path::new(NATIVE_WORKER_RESOURCE);
        assert_eq!(
            path.file_name().and_then(|value| value.to_str()),
            Some("scoreleap-transcriber-native.exe")
        );
        assert_eq!(
            path.parent().and_then(|value| value.to_str()),
            Some("scoreleap-transcriber")
        );
    }
}
