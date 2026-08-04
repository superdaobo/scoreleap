//! scoreleap-core：应用状态与业务逻辑层（无 Tauri command 宏）。
//!
//! 持有文档/序列/Profile/播放会话状态；命令适配层位于 apps/scoreleap/src-tauri。

use scoreleap_arranger::{arrange_pipeline, compile_notes, ArrangeStats, ArrangementOptions};
use scoreleap_music_ir::{GameProfile, MusicDocument};
use scoreleap_scheduler::{
    Clock, InputBackend, MockInputBackend, Scheduler, SchedulerEvent, SchedulerHandle, SystemClock,
};
use scoreleap_sequence::{CompiledSequence, PlaybackCommand, PlaybackState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
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
    /// 曲谱库目录（`<app_data>/library`）。
    pub library_dir: Mutex<std::path::PathBuf>,
    /// 编排后音符缓存（卷帘预览）。
    pub notes_cache: Mutex<HashMap<String, Vec<NoteView>>>,
    pub scheduler: Mutex<Option<SchedulerHandle>>,
    /// 上次会话异常退出标记（启动自检）。
    pub crash_flag: Mutex<bool>,
}

/// 卷帘预览音符（编排后、编译前）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NoteView {
    pub note: u8,
    pub start_us: i64,
    pub duration_us: i64,
}

/// 曲谱库条目摘要。
#[derive(Debug, Clone, Serialize)]
pub struct DocumentSummary {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
    /// 来源类型：midi / audio_transcription（serde default 向后兼容）。
    #[serde(default = "default_source_type")]
    pub source_type: String,
}

/// manifest 持久化条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
    pub imported_at: u64,
    /// 来源类型：midi / audio_transcription（旧 manifest 缺省 = midi）。
    #[serde(default = "default_source_type")]
    pub source_type: String,
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
    /// 来源类型：midi / audio_transcription。
    #[serde(default = "default_source_type")]
    pub source_type: String,
}

fn default_source_type() -> String {
    "midi".into()
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

/// 导入 MIDI 文件（解析在后台线程执行）。来源类型为 midi。
pub fn import_midi(state: &AppState, path: String) -> Result<ImportSummary, CoreError> {
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".into());
    import_midi_from_path(state, &path, &name, "midi")
}

/// 共享 MIDI 导入入口：直接导入的 MIDI 与转录生成的 MIDI 均经此进入曲谱库。
/// `display_name` 用于曲谱库显示名；`source_type` 为 midi 或 audio_transcription。
pub fn import_midi_from_path(
    state: &AppState,
    path: &str,
    display_name: &str,
    source_type: &str,
) -> Result<ImportSummary, CoreError> {
    let bytes =
        std::fs::read(path).map_err(|e| CoreError::Invalid(format!("读取文件失败: {e}")))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scoreleap_midi::parse_midi(&bytes));
    });
    let doc = rx
        .recv()
        .map_err(|_| CoreError::Invalid("解析线程失败".into()))?
        .map_err(CoreError::from)?;
    let doc_id = format!("doc-{}", uuid::Uuid::new_v4());
    let summary = ImportSummary {
        doc_id: doc_id.clone(),
        note_count: doc.note_count(),
        duration_ms: doc.duration_us / 1000,
        format: format!("{:?}", doc.format),
        track_count: doc.tracks.len(),
        bpm_range: doc.bpm_range(),
        name: display_name.to_string(),
        source_type: source_type.to_string(),
    };
    state.documents.lock().unwrap().insert(doc_id.clone(), doc);

    // 持久化：复制源文件到曲谱库并更新 manifest（失败不阻断导入，仅记录日志）
    if let Err(e) = persist_import(state, &doc_id, path, &summary) {
        tracing::warn!("曲谱库持久化失败（本次会话仍可用）: {e}");
    }

    Ok(summary)
}

/// 将导入的 MIDI 复制到曲谱库并更新 manifest（原子写）。
fn persist_import(
    state: &AppState,
    doc_id: &str,
    src_path: &str,
    summary: &ImportSummary,
) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    std::fs::create_dir_all(&dir)
        .map_err(|e| CoreError::Invalid(format!("创建曲谱库目录失败: {e}")))?;
    let dest = dir.join(format!("{doc_id}.mid"));
    std::fs::copy(src_path, &dest)
        .map_err(|e| CoreError::Invalid(format!("复制 MIDI 失败: {e}")))?;

    let mut entries = read_manifest(&dir);
    entries.insert(
        0,
        ManifestEntry {
            doc_id: doc_id.to_string(),
            name: summary.name.clone(),
            format: summary.format.clone(),
            track_count: summary.track_count,
            note_count: summary.note_count,
            source_type: summary.source_type.clone(),
            duration_ms: summary.duration_ms,
            bpm_range: summary.bpm_range,
            imported_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        },
    );
    entries.truncate(50);
    write_manifest(&dir, &entries)
}

/// 读取 manifest；不存在返回空；损坏时重置为空数组并记录日志。
fn read_manifest(dir: &Path) -> Vec<ManifestEntry> {
    let path = dir.join("manifest.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            tracing::warn!("manifest 损坏，重置为空: {e}");
            let _ = std::fs::write(&path, "[]");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// 原子写 manifest。
fn write_manifest(dir: &Path, entries: &[ManifestEntry]) -> Result<(), CoreError> {
    let path = dir.join("manifest.json");
    let tmp = dir.join("manifest.json.tmp");
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| CoreError::Invalid(format!("manifest 序列化失败: {e}")))?;
    std::fs::write(&tmp, json)
        .map_err(|e| CoreError::Invalid(format!("manifest 写入失败: {e}")))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| CoreError::Invalid(format!("manifest 替换失败: {e}")))?;
    Ok(())
}

/// 曲谱库列表（过滤文件缺失条目）。
pub fn list_documents(state: &AppState) -> Result<Vec<DocumentSummary>, CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    let mut out = Vec::new();
    let mut kept = Vec::new();
    for e in read_manifest(&dir) {
        if dir.join(format!("{}.mid", e.doc_id)).exists() {
            kept.push(e.clone());
            out.push(DocumentSummary {
                doc_id: e.doc_id,
                name: e.name,
                format: e.format,
                track_count: e.track_count,
                note_count: e.note_count,
                duration_ms: e.duration_ms,
                bpm_range: e.bpm_range,
                source_type: e.source_type,
            });
        }
    }
    // 顺手清理已缺失条目
    if kept.len() != out.len() {
        let _ = write_manifest(&dir, &kept);
    }
    Ok(out)
}

/// 取文档；内存没有时从曲谱库自动加载。
pub fn load_document(state: &AppState, doc_id: &str) -> Result<MusicDocument, CoreError> {
    if let Some(doc) = state.documents.lock().unwrap().get(doc_id) {
        return Ok(doc.clone());
    }
    let dir = state.library_dir.lock().unwrap().clone();
    let path = dir.join(format!("{doc_id}.mid"));
    let bytes =
        std::fs::read(&path).map_err(|_| CoreError::DocumentNotFound(doc_id.to_string()))?;
    let doc = scoreleap_midi::parse_midi(&bytes).map_err(CoreError::from)?;
    state
        .documents
        .lock()
        .unwrap()
        .insert(doc_id.to_string(), doc.clone());
    Ok(doc)
}

/// 轨道列表。
pub fn get_tracks(state: &AppState, doc_id: String) -> Result<Vec<TrackSummary>, CoreError> {
    let doc = load_document(state, &doc_id)?;
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

/// 执行编排并缓存序列。Profile 未加载时自动加载默认（首个可用）Profile。
pub fn compile(
    state: &AppState,
    doc_id: String,
    enabled_tracks: Vec<u16>,
    options: ArrangementOptions,
) -> Result<CompileSummary, CoreError> {
    let doc = load_document(state, &doc_id)?;
    let profile = ensure_profile(state)?;
    let (notes, stats) = arrange_pipeline(&doc, &options, &profile, &enabled_tracks)?;
    let seq = compile_notes(
        &notes,
        &profile,
        &doc,
        &enabled_tracks,
        stats.applied_transpose,
    );
    let seq_id = format!("seq-{}", uuid::Uuid::new_v4());
    let summary = CompileSummary {
        seq_id: seq_id.clone(),
        action_count: seq.actions.len(),
        duration_ms: seq.duration_us / 1000,
        stats,
    };
    state.sequences.lock().unwrap().insert(seq_id.clone(), seq);
    state.notes_cache.lock().unwrap().insert(
        seq_id.clone(),
        notes
            .iter()
            .map(|n| NoteView {
                note: n.note,
                start_us: n.start_us,
                duration_us: n.duration_us,
            })
            .collect(),
    );
    Ok(summary)
}

/// 卷帘预览音符（编译时缓存）。
pub fn get_sequence_notes(state: &AppState, seq_id: String) -> Result<Vec<NoteView>, CoreError> {
    state
        .notes_cache
        .lock()
        .unwrap()
        .get(&seq_id)
        .cloned()
        .ok_or_else(|| CoreError::SequenceNotFound(seq_id.clone()))
}

/// 取当前 Profile；未加载时自动加载默认（identity-v 或首个可用）Profile。
pub fn ensure_profile(state: &AppState) -> Result<GameProfile, CoreError> {
    if let Some(p) = state.profile.lock().unwrap().clone() {
        return Ok(p);
    }
    let dir = state.profile_dir.lock().unwrap().clone();
    let mut store = scoreleap_game_profile::ProfileStore::new(dir);
    let ids = store.list_ids().map_err(CoreError::from)?;
    let id = ids
        .iter()
        .find(|id| id.as_str() == "identity-v")
        .or_else(|| ids.first())
        .ok_or(CoreError::NoProfile)?;
    let p = store.load(id).map_err(CoreError::from)?;
    *state.profile.lock().unwrap() = Some(p.clone());
    Ok(p)
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

/// 测试按键注入：向当前前台窗口发送一次按下+抬起（用于排查 SendInput 被 UIPI 阻止）。
/// scan：Windows 扫描码（十进制）。返回注入结果信息。
/// 音频文件信息（转录确认界面使用：存在性 + 名称 + 大小）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioFileInfo {
    pub name: String,
    pub size_bytes: u64,
}

pub fn audio_file_info(path: &str) -> Result<AudioFileInfo, CoreError> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(CoreError::Invalid("输入文件不存在".into()));
    }
    if !p.is_file() {
        return Err(CoreError::Invalid("输入不是普通文件".into()));
    }
    let size = std::fs::metadata(p)
        .map(|m| m.len())
        .map_err(|e| CoreError::Invalid(format!("读取文件信息失败: {e}")))?;
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio.mp3".into());
    Ok(AudioFileInfo { name, size_bytes: size })
}

pub fn test_key(scan: u16) -> Result<String, CoreError> {
    #[cfg(windows)]
    {
        tauri_plugin_scoreleap_input::test_inject_key(scan)
            .map_err(|e| CoreError::Invalid(e.to_string()))
    }
    #[cfg(not(windows))]
    {
        let _ = scan;
        Err(CoreError::Invalid("仅 Windows 支持按键测试".into()))
    }
}

/// 前台窗口信息（测试页诊断：UIPI 提权检测）。
#[derive(Debug, Clone, Serialize)]
pub struct ForegroundInfo {
    /// 前台窗口标题。
    pub title: String,
    /// 前台进程 PID。
    pub pid: u32,
    /// 前台进程是否以管理员（高完整性）运行。
    pub elevated: bool,
    /// 本程序是否以管理员运行。
    pub our_elevated: bool,
}

/// 前台窗口与提权状态检测。
pub fn check_foreground() -> Result<ForegroundInfo, CoreError> {
    #[cfg(windows)]
    {
        let info = tauri_plugin_scoreleap_input::check_foreground_info()
            .map_err(|e| CoreError::Invalid(e.to_string()))?;
        Ok(ForegroundInfo {
            title: info.title,
            pid: info.pid,
            elevated: info.elevated,
            our_elevated: info.our_elevated,
        })
    }
    #[cfg(not(windows))]
    {
        Err(CoreError::Invalid("仅 Windows 支持前台检测".into()))
    }
}

/// 键位条目（测试页逐键测试）。
#[derive(Debug, Clone, Serialize)]
pub struct KeymapEntry {
    pub note: u8,
    /// 扫描码（十进制）。
    pub scan: u16,
    /// 是否扩展扫描码。
    pub extended: bool,
    /// 键名（如 "A"、"1"）。
    pub label: String,
}

/// 当前 Profile 的 Windows 键位列表（note → 键）。
pub fn list_keymap(state: &AppState) -> Result<Vec<KeymapEntry>, CoreError> {
    let profile = ensure_profile(state)?;
    let mut entries: Vec<KeymapEntry> = profile
        .keymap
        .iter()
        .map(|(note, code)| {
            let (scan, extended) = match code {
                scoreleap_music_ir::KeyCode::Scan(s) => (*s, false),
                scoreleap_music_ir::KeyCode::ExtendedScan(s) => (*s, true),
            };
            KeymapEntry {
                note: *note,
                scan,
                extended,
                label: scan_code_name(scan),
            }
        })
        .collect();
    entries.sort_by_key(|e| e.note);
    Ok(entries)
}

/// 扫描码 → 键名（常用键；未知名返回十六进制）。
pub fn scan_code_name(scan: u16) -> String {
    const NAMES: &[(u16, &str)] = &[
        (0x01, "Esc"),
        (0x02, "1"),
        (0x03, "2"),
        (0x04, "3"),
        (0x05, "4"),
        (0x06, "5"),
        (0x07, "6"),
        (0x08, "7"),
        (0x09, "8"),
        (0x0A, "9"),
        (0x0B, "0"),
        (0x0C, "-"),
        (0x0D, "="),
        (0x0E, "Backspace"),
        (0x0F, "Tab"),
        (0x10, "Q"),
        (0x11, "W"),
        (0x12, "E"),
        (0x13, "R"),
        (0x14, "T"),
        (0x15, "Y"),
        (0x16, "U"),
        (0x17, "I"),
        (0x18, "O"),
        (0x19, "P"),
        (0x1A, "["),
        (0x1B, "]"),
        (0x1C, "Enter"),
        (0x1D, "Ctrl"),
        (0x1E, "A"),
        (0x1F, "S"),
        (0x20, "D"),
        (0x21, "F"),
        (0x22, "G"),
        (0x23, "H"),
        (0x24, "J"),
        (0x25, "K"),
        (0x26, "L"),
        (0x27, ";"),
        (0x28, "'"),
        (0x29, "`"),
        (0x2B, "\\"),
        (0x2C, "Z"),
        (0x2D, "X"),
        (0x2E, "C"),
        (0x2F, "V"),
        (0x30, "B"),
        (0x31, "N"),
        (0x32, "M"),
        (0x33, ","),
        (0x34, "."),
        (0x35, "/"),
        (0x39, "Space"),
        (0x47, "Home"),
        (0x48, "Up"),
        (0x49, "PgUp"),
        (0x4B, "Left"),
        (0x4D, "Right"),
        (0x4F, "End"),
    ];
    NAMES
        .iter()
        .find(|(s, _)| *s == scan)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("0x{scan:02X}"))
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
