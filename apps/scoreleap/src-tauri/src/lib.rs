//! ScoreLeap Tauri 应用入口（命令适配层）。
//!
//! 注意：命令函数**必须为非 pub**——tauri-macros 对 pub 命令生成
//! `#[macro_export]` + `pub use`，与 rustc 的宏命名空间检查冲突（E0255）。

use scoreleap_core::{AppState, CoreError};
use scoreleap_sequence::PlaybackState;
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// 初始化 tracing 日志。
fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,scoreleap=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[tauri::command]
fn import_midi(
    state: State<'_, AppState>,
    path: String,
) -> Result<scoreleap_core::ImportSummary, CoreError> {
    scoreleap_core::import_midi(&state, path)
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

pub fn run() {
    init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_scoreleap_input::init())
        .manage(AppState::default())
        .setup(|app| {
            // Profile 目录：优先仓库内置 game-profiles，否则用数据目录
            let builtin =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../game-profiles");
            let profiles_dir = if builtin.join("identity-v/profile.json").exists() {
                builtin
            } else {
                let data_dir = app
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                let p = data_dir.join("game-profiles");
                let _ = std::fs::create_dir_all(&p);
                p
            };
            *app.state::<AppState>().profile_dir.lock().unwrap() = profiles_dir;

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
            Ok(())
        })
        .on_window_event(|window, event| {
            // 窗口销毁时确保调度器关闭（释放按键）
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                scoreleap_core::shutdown_scheduler(&state);
            }
        })
        .invoke_handler(tauri::generate_handler![
            import_midi,
            get_tracks,
            compile,
            start_playback,
            pause_playback,
            resume_playback,
            stop_playback,
            emergency_stop,
            list_profiles,
            load_profile,
            current_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
