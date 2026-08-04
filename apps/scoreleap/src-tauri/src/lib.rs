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

pub fn run() {
    init_logging();
    install_panic_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_scoreleap_input::init())
        .manage(AppState::default())
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
            get_crash_flag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
