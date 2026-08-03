//! scoreleap-core：应用状态与业务逻辑层（无 Tauri command 宏）。
//!
//! 持有文档/序列/Profile/播放会话状态；命令适配层位于 apps/scoreleap/src-tauri。

use scoreleap_arranger::{arrange, ArrangeStats, ArrangementOptions};
use scoreleap_music_ir::{GameProfile, MusicDocument};
use scoreleap_scheduler::{
    Clock, InputBackend, MockInputBackend, Scheduler, SchedulerEvent, SchedulerHandle, SystemClock,
};
use scoreleap_sequence::{CompiledSequence, PlaybackCommand, PlaybackState};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// 错误类型。
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    Midi(#[from] scoreleap_midi::MidiError),
    #[error("{0}")]
    Arrange(#[from] scoreleap_arranger::ArrangeError),
    #[error("{0}")]
    Profile(#[from] scoreleap_game_profile::ProfileError),
    #[error("文档不存在: {0}")]
    DocumentNotFound(String),
    #[error("序列不存在: {0}")]
    SequenceNotFound(String),
    #[error("未加载 Profile")]
    NoProfile,
    #[error("调度器未运行")]
    SchedulerNotRunning,
    #[error("调度器错误: {0}")]
    Scheduler(String),
    #[error("无效参数: {0}")]
    Invalid(String),
}

impl Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// 应用状态（内部可变）。
#[derive(Default)]
pub struct AppState {
    pub documents: Mutex<HashMap<String, MusicDocument>>,
    pub sequences: Mutex<HashMap<String, CompiledSequence>>,
    pub profile: Mutex<Option<GameProfile>>,
    pub profile_dir: Mutex<std::path::PathBuf>,
    pub scheduler: Mutex<Option<SchedulerHandle>>,
}

/// 导入结果摘要。
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
}

/// 轨道摘要。
#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub id: u16,
    pub name: String,
    pub note_count: usize,
    pub enabled: bool,
}

/// 编译结果摘要。
#[derive(Debug, Clone, Serialize)]
pub struct CompileSummary {
    pub seq_id: String,
    pub action_count: usize,
    pub duration_ms: i64,
    pub stats: ArrangeStats,
}

/// 播放状态摘要。
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub position_ms: i64,
    pub pressed_keys: u32,
}

// ---------------------------------------------------------------------------
// 业务逻辑（命令适配层调用）
// ---------------------------------------------------------------------------

/// 导入 MIDI 文件（解析在后台线程执行）。
pub fn import_midi(state: &AppState, path: String) -> Result<ImportSummary, CoreError> {
    let bytes =
        std::fs::read(&path).map_err(|e| CoreError::Invalid(format!("读取文件失败: {e}")))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scoreleap_midi::parse_midi(&bytes));
    });
    let doc = rx
        .recv()
        .map_err(|_| CoreError::Invalid("解析线程失败".into()))?
        .map_err(CoreError::from)?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".into());
    let doc_id = format!("doc-{}", uuid::Uuid::new_v4());
    let summary = ImportSummary {
        doc_id: doc_id.clone(),
        note_count: doc.note_count(),
        duration_ms: doc.duration_us / 1000,
        format: format!("{:?}", doc.format),
        track_count: doc.tracks.len(),
        bpm_range: doc.bpm_range(),
        name,
    };
    state.documents.lock().unwrap().insert(doc_id, doc);
    Ok(summary)
}

/// 轨道列表。
pub fn get_tracks(state: &AppState, doc_id: String) -> Result<Vec<TrackSummary>, CoreError> {
    let docs = state.documents.lock().unwrap();
    let doc = docs
        .get(&doc_id)
        .ok_or_else(|| CoreError::DocumentNotFound(doc_id.clone()))?;
    Ok(doc
        .tracks
        .iter()
        .map(|t| TrackSummary {
            id: t.id,
            name: t.name.clone(),
            note_count: t.notes.len(),
            enabled: true,
        })
        .collect())
}

/// 执行编排并缓存序列。
pub fn compile(
    state: &AppState,
    doc_id: String,
    enabled_tracks: Vec<u16>,
    options: ArrangementOptions,
) -> Result<CompileSummary, CoreError> {
    let doc = state
        .documents
        .lock()
        .unwrap()
        .get(&doc_id)
        .ok_or_else(|| CoreError::DocumentNotFound(doc_id.clone()))?
        .clone();
    let profile = state
        .profile
        .lock()
        .unwrap()
        .clone()
        .ok_or(CoreError::NoProfile)?;
    let (seq, stats) = arrange(&doc, &options, &profile, &enabled_tracks)?;
    let seq_id = format!("seq-{}", uuid::Uuid::new_v4());
    let summary = CompileSummary {
        seq_id: seq_id.clone(),
        action_count: seq.actions.len(),
        duration_ms: seq.duration_us / 1000,
        stats,
    };
    state.sequences.lock().unwrap().insert(seq_id, seq);
    Ok(summary)
}

/// 开始播放（3 秒倒计时后执行；backend="mock" 用于测试）。
pub fn start_playback(
    app: &AppHandle,
    state: &AppState,
    seq_id: String,
    backend: String,
) -> Result<PlaybackStatus, CoreError> {
    stop_existing(state);
    let seq = state
        .sequences
        .lock()
        .unwrap()
        .get(&seq_id)
        .ok_or_else(|| CoreError::SequenceNotFound(seq_id.clone()))?
        .clone();

    let backend: Box<dyn InputBackend> = if backend == "mock" {
        Box::new(MockInputBackend::new())
    } else {
        #[cfg(windows)]
        {
            Box::new(tauri_plugin_scoreleap_input::SendInputBackend::new())
        }
        #[cfg(not(windows))]
        {
            let _ = backend;
            Box::new(MockInputBackend::new())
        }
    };

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let handle = Scheduler::spawn(seq, clock, backend);
    // 事件转发线程：Scheduler → Tauri 前端
    let app2 = app.clone();
    let h2 = handle
        .try_clone()
        .map_err(|e| CoreError::Invalid(format!("调度器句柄克隆失败: {e}")))?;
    std::thread::spawn(move || loop {
        match h2.recv_event() {
            Ok(SchedulerEvent::State(s)) => {
                let _ = app2.emit("playback://state", s);
            }
            Ok(SchedulerEvent::Progress(p)) => {
                let _ = app2.emit("playback://progress", p);
            }
            Ok(SchedulerEvent::Error(e)) => {
                let _ = app2.emit("playback://error", e);
            }
            Err(_) => break,
        }
    });
    handle
        .command(PlaybackCommand::Start)
        .map_err(CoreError::Scheduler)?;
    *state.scheduler.lock().unwrap() = Some(handle);
    Ok(PlaybackStatus {
        state: PlaybackState::Countdown,
        position_ms: 0,
        pressed_keys: 0,
    })
}

/// 暂停。
pub fn pause_playback(state: &AppState) -> Result<(), CoreError> {
    with_scheduler(state, |h| h.command(PlaybackCommand::Pause))
}

/// 继续。
pub fn resume_playback(state: &AppState) -> Result<(), CoreError> {
    with_scheduler(state, |h| h.command(PlaybackCommand::Resume))
}

/// 停止。
pub fn stop_playback(state: &AppState) -> Result<(), CoreError> {
    with_scheduler(state, |h| h.command(PlaybackCommand::Stop))
}

/// 紧急停止（立即释放全部按键）。
pub fn emergency_stop(state: &AppState) -> Result<(), CoreError> {
    with_scheduler(state, |h| h.command(PlaybackCommand::EmergencyStop))
}

/// Profile 列表。
pub fn list_profiles(state: &AppState) -> Result<Vec<String>, CoreError> {
    let dir = state.profile_dir.lock().unwrap().clone();
    let store = scoreleap_game_profile::ProfileStore::new(dir);
    store.list_ids().map_err(CoreError::from)
}

/// 加载 Profile。
pub fn load_profile(state: &AppState, id: String) -> Result<GameProfile, CoreError> {
    let dir = state.profile_dir.lock().unwrap().clone();
    let mut store = scoreleap_game_profile::ProfileStore::new(dir);
    let p = store.load(&id).map_err(CoreError::from)?;
    *state.profile.lock().unwrap() = Some(p.clone());
    Ok(p)
}

/// 当前 Profile。
pub fn current_profile(state: &AppState) -> Result<Option<GameProfile>, CoreError> {
    Ok(state.profile.lock().unwrap().clone())
}

/// 会话结束时停止调度器（释放按键）。
pub fn shutdown_scheduler(state: &AppState) {
    stop_existing(state);
}

fn with_scheduler<F>(state: &AppState, f: F) -> Result<(), CoreError>
where
    F: FnOnce(&SchedulerHandle) -> Result<(), String>,
{
    match state.scheduler.lock().unwrap().as_ref() {
        Some(h) => f(h).map_err(CoreError::Scheduler),
        None => Err(CoreError::SchedulerNotRunning),
    }
}

fn stop_existing(state: &AppState) {
    if let Some(h) = state.scheduler.lock().unwrap().take() {
        h.shutdown();
    }
}
