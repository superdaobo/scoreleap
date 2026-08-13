//! 转录服务：管理原生 ONNX sidecar 生命周期、解析 JSON Lines、导入结果 MIDI。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;

use crate::error::{TranscriptionError, TranscriptionErrorCode};
use crate::job::{JobStatus, TranscriptionJob};
use crate::protocol::WorkerMsg;
use crate::raw_stats::RawTranscriptionStats;

/// 读取音频文件元数据标签（标题/艺术家；ID3/FLAC/Vorbis）。
/// 读取失败或无标签返回 (None, None)，不阻断转录流程。
fn read_audio_tags(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(probe) = lofty::probe::Probe::open(path) else {
        return (None, None);
    };
    let Ok(file) = probe.read() else {
        return (None, None);
    };
    let mut title = None;
    let mut artist = None;
    for tag in file.tags() {
        if title.is_none() {
            title = tag.title().map(|s| s.to_string());
        }
        if artist.is_none() {
            artist = tag.artist().map(|s| s.to_string());
        }
        if title.is_some() && artist.is_some() {
            break;
        }
    }
    (title, artist)
}

/// 转录引擎。快速模式使用现有 Basic Pitch ONNX；高质量模式使用安装包内置 Transkun。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionEngine {
    #[default]
    Fast,
    HighQuality,
}

impl TranscriptionEngine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::HighQuality => "high_quality",
        }
    }
}

/// Worker 启动规格（参数数组；禁止 shell 字符串拼接）。
/// 自包含 Worker（如 Transkun PyInstaller sidecar）不需要外部模型和运行时路径。
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    pub program: String,
    pub args: Vec<String>,
    pub model_path: Option<PathBuf>,
    pub onnx_runtime_path: Option<PathBuf>,
}

impl WorkerSpec {
    pub fn basic_pitch(
        program: impl Into<String>,
        args: Vec<String>,
        model_path: PathBuf,
        onnx_runtime_path: Option<PathBuf>,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            model_path: Some(model_path),
            onnx_runtime_path,
        }
    }

    pub fn self_contained(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            model_path: None,
            onnx_runtime_path: None,
        }
    }
}

/// 可用 Worker 注册表。高质量资源可以缺席，快速模式仍可独立工作。
#[derive(Debug, Clone, Default)]
pub struct TranscriptionWorkers {
    pub fast: Option<WorkerSpec>,
    pub high_quality: Option<WorkerSpec>,
}

impl TranscriptionWorkers {
    fn resolve(&self, engine: TranscriptionEngine) -> Result<&WorkerSpec, TranscriptionError> {
        let worker = match engine {
            TranscriptionEngine::Fast => self.fast.as_ref(),
            TranscriptionEngine::HighQuality => self.high_quality.as_ref(),
        };
        worker.ok_or_else(|| {
            TranscriptionError::new(
                TranscriptionErrorCode::EngineUnavailable,
                match engine {
                    TranscriptionEngine::Fast => "快速转录组件当前不可用",
                    TranscriptionEngine::HighQuality => {
                        "高质量钢琴转录组件未包含在当前安装包中，请重新安装完整版"
                    }
                },
            )
        })
    }

    pub fn supports(&self, engine: TranscriptionEngine) -> bool {
        match engine {
            TranscriptionEngine::Fast => self.fast.is_some(),
            TranscriptionEngine::HighQuality => self.high_quality.is_some(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionPreset {
    #[default]
    Balanced,
    Detail,
    NoiseReduced,
}

impl TranscriptionPreset {
    fn as_str(self) -> &'static str {
        match self {
            // UI 使用简短稳定值，启动 sidecar 时转换为原生运行时的钢琴预设契约。
            Self::Balanced => "piano_balanced",
            Self::Detail => "piano_detail",
            Self::NoiseReduced => "piano_noise_reduced",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptionOptions {
    #[serde(default)]
    pub engine: TranscriptionEngine,
    #[serde(default)]
    pub preset: TranscriptionPreset,
    pub onset_threshold: Option<f32>,
    pub frame_threshold: Option<f32>,
    pub minimum_note_ms: Option<u32>,
}

impl TranscriptionOptions {
    fn validate(&self) -> Result<(), TranscriptionError> {
        for (name, value) in [
            ("onset_threshold", self.onset_threshold),
            ("frame_threshold", self.frame_threshold),
        ] {
            if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
                return Err(TranscriptionError::new(
                    TranscriptionErrorCode::WorkerProtocolError,
                    format!("{name} 必须在 0..=1 范围内"),
                ));
            }
        }
        if self
            .minimum_note_ms
            .is_some_and(|value| !(20..=2000).contains(&value))
        {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::WorkerProtocolError,
                "minimum_note_ms 必须在 20..=2000 范围内",
            ));
        }
        Ok(())
    }
}

/// 事件（src-tauri 层转 app.emit）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum TranscriptionEvent {
    #[serde(rename = "state")]
    State { job_id: String, status: String },
    #[serde(rename = "stage")]
    Stage {
        job_id: String,
        stage: String,
        message: String,
    },
    #[serde(rename = "completed")]
    Completed {
        job_id: String,
        doc_id: String,
        midi_path: String,
        note_count: u64,
        elapsed_ms: i64,
    },
    #[serde(rename = "error")]
    Error {
        job_id: String,
        code: String,
        message: String,
    },
}

pub(crate) type EventFn = Arc<dyn Fn(TranscriptionEvent) + Send + Sync>;

/// 转录结果导入曲谱库的载荷（转录元数据与源音频标签随曲谱持久化）。
#[derive(Debug, Clone)]
pub struct ImportPayload {
    pub midi_path: String,
    pub display_name: String,
    /// worker 写入的 metadata.json 原文（读取失败为 None；解析由曲谱库侧负责）。
    pub metadata_json: Option<String>,
    /// 源音频文件元数据标签（ID3/FLAC/Vorbis）。
    pub title: Option<String>,
    pub artist: Option<String>,
}

type ImporterFn = Arc<dyn Fn(&ImportPayload) -> Result<String, String> + Send + Sync>;

pub(crate) struct ActiveJob {
    pub(crate) job: TranscriptionJob,
    pub(crate) task_dir: PathBuf,
    pub(crate) child: Arc<Mutex<Option<Child>>>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

/// 转录服务（单任务并发；第二个任务返回 TRANSCRIPTION_BUSY）。
/// Clone 共享同一任务状态（内部为 Arc）。
#[derive(Clone)]
pub struct TranscriptionService {
    inner: Arc<Mutex<Option<ActiveJob>>>,
    /// 串行化“检查空闲 → 启动进程 → 发布任务”，防止两个命令同时穿透单任务限制。
    start_lock: Arc<Mutex<()>>,
    workers: TranscriptionWorkers,
    data_dir: PathBuf,
    on_event: EventFn,
    importer: ImporterFn,
    last_error_code: Arc<Mutex<Option<String>>>,
    last_error_message: Arc<Mutex<Option<String>>>,
}

/// 输入限制（与 Worker 端一致）。
pub const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;
pub const ALLOWED_EXTENSIONS: &[&str] = &["mp3", "wav", "flac"];

impl TranscriptionService {
    /// 向后兼容的快速模式构造器。
    pub fn new(
        data_dir: PathBuf,
        worker: WorkerSpec,
        on_event: EventFn,
        importer: ImporterFn,
    ) -> Self {
        Self::new_with_workers(
            data_dir,
            TranscriptionWorkers {
                fast: Some(worker),
                high_quality: None,
            },
            on_event,
            importer,
        )
    }

    pub fn new_with_workers(
        data_dir: PathBuf,
        workers: TranscriptionWorkers,
        on_event: EventFn,
        importer: ImporterFn,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            start_lock: Arc::new(Mutex::new(())),
            workers,
            data_dir,
            on_event,
            importer,
            last_error_code: Arc::new(Mutex::new(None)),
            last_error_message: Arc::new(Mutex::new(None)),
        }
    }

    pub fn supports_engine(&self, engine: TranscriptionEngine) -> bool {
        self.workers.supports(engine)
    }

    fn emit(&self, event: TranscriptionEvent) {
        (self.on_event)(event);
    }

    /// 校验输入路径（存在/普通文件/扩展名/大小；时长由 Worker 校验）。
    fn validate_input(&self, path: &str) -> Result<(), TranscriptionError> {
        let p = Path::new(path);
        if !p.exists() {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::InvalidAudioPath,
                "输入文件不存在",
            ));
        }
        if !p.is_file() {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::InvalidAudioPath,
                "输入不是普通文件",
            ));
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::UnsupportedAudioFormat,
                format!("仅支持 MP3/WAV/FLAC，收到 .{ext}"),
            ));
        }
        let size = std::fs::metadata(p).map(|m| m.len()).map_err(|e| {
            TranscriptionError::new(TranscriptionErrorCode::InvalidAudioPath, e.to_string())
        })?;
        if size == 0 {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::InvalidAudioPath,
                "输入文件为空",
            ));
        }
        if size > MAX_FILE_BYTES {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::AudioFileTooLarge,
                format!("文件超过 {}MB 上限", MAX_FILE_BYTES / 1024 / 1024),
            ));
        }
        Ok(())
    }

    /// 启动转录。返回 job_id。
    pub fn start(&self, input_path: &str) -> Result<String, TranscriptionError> {
        self.start_with_options(input_path, TranscriptionOptions::default())
    }

    /// 使用预设和可选高级阈值启动转录。
    pub fn start_with_options(
        &self,
        input_path: &str,
        options: TranscriptionOptions,
    ) -> Result<String, TranscriptionError> {
        let _start_guard = self
            .start_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let guard = self.inner.lock().unwrap();
            if let Some(a) = guard.as_ref() {
                if !a.job.status.is_terminal() {
                    return Err(TranscriptionError::new(
                        TranscriptionErrorCode::TranscriptionBusy,
                        "已有转录任务在运行",
                    ));
                }
            }
        }
        self.validate_input(input_path)?;
        options.validate()?;
        let worker = self.workers.resolve(options.engine)?.clone();
        if options.engine == TranscriptionEngine::Fast && worker.model_path.is_none() {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::ModelDownloadRequired,
                "尚未安装可用的转录模型，请先在设置中下载",
            ));
        }
        if worker
            .model_path
            .as_ref()
            .is_some_and(|path| !path.is_file())
        {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::ModelDownloadRequired,
                "尚未安装可用的转录模型，请先在设置中下载",
            ));
        }
        if worker
            .onnx_runtime_path
            .as_ref()
            .is_some_and(|path| !path.is_file())
        {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::RuntimeMissing,
                "未找到 ONNX Runtime，请重新安装完整版本",
            ));
        }
        *self.last_error_code.lock().unwrap() = None;
        *self.last_error_message.lock().unwrap() = None;

        let job_id = format!("job-{}", uuid::Uuid::new_v4());
        let request_id = format!("req-{}", uuid::Uuid::new_v4());
        let source_name = Path::new(input_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio.mp3".into());
        // 源音频元数据标签（标题/艺术家），随转录结果持久化到曲谱库
        let (tag_title, tag_artist) = read_audio_tags(Path::new(input_path));

        let jobs_root = self.data_dir.join("jobs");
        std::fs::create_dir_all(&jobs_root).map_err(|e| {
            TranscriptionError::new(
                TranscriptionErrorCode::InternalError,
                format!("创建任务目录失败: {e}"),
            )
        })?;
        let task_dir = jobs_root.join(&job_id);
        std::fs::create_dir_all(&task_dir).map_err(|e| {
            TranscriptionError::new(
                TranscriptionErrorCode::InternalError,
                format!("创建任务目录失败: {e}"),
            )
        })?;
        let midi_path = task_dir.join("generated.mid");
        let metadata_path = task_dir.join("metadata.json");

        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // 启动 Worker（参数数组；无 shell；路径由后端决定）。
        // Basic Pitch 需要模型/ORT 参数；Transkun sidecar 已自包含这些资源。
        let mut cmd = Command::new(&worker.program);
        cmd.args(&worker.args)
            .arg("transcribe")
            .arg("--request-id")
            .arg(&request_id)
            .arg("--input")
            .arg(input_path)
            .arg("--output-midi")
            .arg(&midi_path)
            .arg("--output-metadata")
            .arg(&metadata_path)
            .arg("--preset")
            .arg(options.preset.as_str())
            .stdin(Stdio::null());
        if let Some(model) = &worker.model_path {
            cmd.arg("--model").arg(model);
        }
        if let Some(runtime) = &worker.onnx_runtime_path {
            cmd.arg("--onnx-runtime").arg(runtime);
        }
        if let Some(value) = options.onset_threshold {
            cmd.arg("--onset-threshold").arg(value.to_string());
        }
        if let Some(value) = options.frame_threshold {
            cmd.arg("--frame-threshold").arg(value.to_string());
        }
        if let Some(value) = options.minimum_note_ms {
            cmd.arg("--minimum-note-length-ms").arg(value.to_string());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // 隐藏子进程控制台：GUI 父进程启动控制台子系统 worker 时，
            // 不设置该标志会在桌面弹出黑色终端窗口。
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&task_dir);
                return Err(TranscriptionError::new(
                    TranscriptionErrorCode::WorkerStartFailed,
                    format!("Worker 启动失败: {e}"),
                ));
            }
        };

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let child_shared = Arc::new(Mutex::new(Some(child)));
        let cancelled = Arc::new(AtomicBool::new(false));
        let saw_result = Arc::new(AtomicBool::new(false));

        {
            let mut guard = self.inner.lock().unwrap();
            *guard = Some(ActiveJob {
                job: TranscriptionJob {
                    job_id: job_id.clone(),
                    request_id: request_id.clone(),
                    source_name: source_name.clone(),
                    status: JobStatus::Starting,
                    stage: "starting".into(),
                    message: "正在启动转录组件".into(),
                    started_at_ms: started_at,
                    elapsed_ms: 0,
                    note_count: None,
                    midi_path: Some(midi_path.to_string_lossy().to_string()),
                    metadata_path: Some(metadata_path.to_string_lossy().to_string()),
                    result_doc_id: None,
                    error_code: None,
                    error_message: None,
                    title: tag_title,
                    artist: tag_artist,
                    raw_stats: None,
                },
                task_dir,
                child: child_shared.clone(),
                cancelled: cancelled.clone(),
            });
        }
        self.emit(TranscriptionEvent::State {
            job_id: job_id.clone(),
            status: JobStatus::Starting.as_str().into(),
        });

        // stdout 解析线程
        let stdout_thread = {
            let inner = self.inner.clone();
            let on_event = self.on_event.clone();
            let last_code = self.last_error_code.clone();
            let last_msg = self.last_error_message.clone();
            let jid = job_id.clone();
            let expected_request_id = request_id.clone();
            let saw_result = saw_result.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    match WorkerMsg::parse_line(line) {
                        Ok(msg) => crate::protocol::handle_worker_msg(
                            &crate::protocol::WorkerMessageContext {
                                inner: &inner,
                                on_event: &on_event,
                                last_code: &last_code,
                                last_msg: &last_msg,
                                saw_result: &saw_result,
                                job_id: &jid,
                                expected_request_id: &expected_request_id,
                            },
                            msg,
                        ),
                        Err(_) => {
                            tracing::warn!(job_id = %jid, "Worker 输出非 JSON 行: {line}");
                            *last_code.lock().unwrap() =
                                Some(TranscriptionErrorCode::WorkerProtocolError.as_str().into());
                            *last_msg.lock().unwrap() = Some("Worker 输出了无效 JSONL".into());
                        }
                    }
                }
            })
        };

        // stderr 日志线程
        let stderr_thread = {
            let jid = job_id.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    tracing::warn!(job_id = %jid, "worker-stderr: {line}");
                }
            })
        };

        // 等待线程（轮询退出 → 验证 → 导入 → 完成/失败/取消）
        {
            let inner = self.inner.clone();
            let on_event = self.on_event.clone();
            let importer = self.importer.clone();
            let last_code = self.last_error_code.clone();
            let last_msg = self.last_error_message.clone();
            let jid = job_id.clone();
            let child_shared2 = child_shared.clone();
            let cancelled2 = cancelled.clone();
            let saw_result2 = saw_result.clone();
            std::thread::spawn(move || {
                // 等待退出（100ms 轮询；取消时 kill）
                let exit_code: Option<i32> = loop {
                    if cancelled2.load(Ordering::SeqCst) {
                        if let Some(mut c) = child_shared2.lock().unwrap().take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                    }
                    let mut guard = child_shared2.lock().unwrap();
                    let done = match guard.as_mut() {
                        Some(c) => match c.try_wait() {
                            Ok(Some(status)) => Some(status.code()),
                            Ok(None) => None,
                            Err(_) => Some(None),
                        },
                        None => Some(None),
                    };
                    drop(guard);
                    if let Some(code) = done {
                        break code;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                };

                // 子进程退出后先等待管道读取完成，确保最后一条 result/error 不会与退出判断竞态。
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();

                // 取出 active job（仅当仍是本 job）
                let mut active: Option<(TranscriptionJob, PathBuf)> = None;
                {
                    let guard = inner.lock().unwrap();
                    if let Some(a) = guard.as_ref() {
                        if a.job.job_id == jid {
                            active = Some((a.job.clone(), a.task_dir.clone()));
                        }
                    }
                }
                let Some((mut job, task_dir)) = active else {
                    return;
                };
                job.elapsed_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64 - job.started_at_ms as i64)
                    .unwrap_or(0);

                if cancelled2.load(Ordering::SeqCst) {
                    job.status = JobStatus::Cancelled;
                    job.error_code = Some(TranscriptionErrorCode::JobCancelled.as_str().into());
                    job.error_message = Some("任务已取消".into());
                    let _ = std::fs::remove_dir_all(&task_dir);
                    {
                        let mut guard = inner.lock().unwrap();
                        if guard.as_ref().map(|a| a.job.job_id.clone()) == Some(jid.clone()) {
                            guard.as_mut().unwrap().job = job;
                        }
                    }
                    on_event(TranscriptionEvent::State {
                        job_id: jid,
                        status: JobStatus::Cancelled.as_str().into(),
                    });
                    return;
                }

                // 退出码映射
                let code = exit_code.unwrap_or(9);
                if code == 0
                    && last_code.lock().unwrap().is_none()
                    && saw_result2.load(Ordering::Acquire)
                {
                    // 验证 MIDI
                    job.status = JobStatus::ImportingMidi;
                    job.stage = "importing_midi".into();
                    job.message = "正在导入曲谱库".into();
                    on_event(TranscriptionEvent::Stage {
                        job_id: jid.clone(),
                        stage: "importing_midi".into(),
                        message: "正在导入曲谱库".into(),
                    });
                    let midi_path = job.midi_path.clone().unwrap_or_default();
                    let parse_ok = std::fs::read(&midi_path)
                        .map(|b| scoreleap_midi::parse_midi(&b).is_ok())
                        .unwrap_or(false);
                    if !parse_ok {
                        job.status = JobStatus::Failed;
                        job.error_code =
                            Some(TranscriptionErrorCode::MidiValidationFailed.as_str().into());
                        job.error_message = Some("生成 MIDI 无法被解析".into());
                        let _ = std::fs::remove_dir_all(&task_dir);
                        finish_job(&inner, &on_event, &jid, job);
                        return;
                    }
                    // 导入曲谱库（共享入口）；转录元数据与源音频标签随曲谱持久化
                    let base = Path::new(&job.source_name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "转录曲谱".into());
                    let display_name = format!("{base}（音频转录）");
                    let metadata_json = job
                        .metadata_path
                        .as_deref()
                        .and_then(|p| std::fs::read_to_string(p).ok());
                    let payload = ImportPayload {
                        midi_path: midi_path.clone(),
                        display_name,
                        metadata_json,
                        title: job.title.clone(),
                        artist: job.artist.clone(),
                    };
                    match importer(&payload) {
                        Ok(doc_id) => {
                            job.status = JobStatus::Completed;
                            job.result_doc_id = Some(doc_id.clone());
                            job.message = "转录完成".into();
                            // Issue #57 可观测性：解析 worker metadata 的 notes，
                            // 计算原始高音统计（区分「模型漏高音」与「编排删除高音」）。
                            if let Some(meta_path) = &job.metadata_path {
                                if let Ok(bytes) = std::fs::read(meta_path) {
                                    if let Some(stats) =
                                        RawTranscriptionStats::from_metadata_json(&bytes)
                                    {
                                        tracing::debug!(
                                            job_id = %job.job_id,
                                            raw_note_count = stats.raw_note_count,
                                            raw_min_pitch = stats.raw_min_pitch,
                                            raw_max_pitch = stats.raw_max_pitch,
                                            low_outside_game = stats.low_outside_game,
                                            directly_playable = stats.directly_playable,
                                            high_outside_game = stats.high_outside_game,
                                            "Transkun raw register stats (Issue #57)"
                                        );
                                        job.raw_stats = Some(stats);
                                    }
                                }
                            }
                            {
                                let mut guard = inner.lock().unwrap();
                                if let Some(a) = guard.as_mut() {
                                    a.job = job.clone();
                                }
                            }
                            on_event(TranscriptionEvent::Completed {
                                job_id: jid,
                                doc_id,
                                midi_path,
                                note_count: job.note_count.unwrap_or(0),
                                elapsed_ms: job.elapsed_ms,
                            });
                        }
                        Err(e) => {
                            job.status = JobStatus::Failed;
                            job.error_code =
                                Some(TranscriptionErrorCode::InternalError.as_str().into());
                            job.error_message = Some(format!("曲谱导入失败: {e}"));
                            let _ = std::fs::remove_dir_all(&task_dir);
                            finish_job(&inner, &on_event, &jid, job);
                        }
                    }
                } else {
                    // 非零退出
                    let protocol_code = last_code.lock().unwrap().clone().or_else(|| {
                        (code == 0).then(|| {
                            TranscriptionErrorCode::WorkerProtocolError
                                .as_str()
                                .to_string()
                        })
                    });
                    let protocol_message = last_msg.lock().unwrap().clone().or_else(|| {
                        (code == 0).then(|| "Worker 未返回有效 result 消息".to_string())
                    });
                    let (code, message) =
                        crate::protocol::map_exit_code(code, &protocol_code, &protocol_message);
                    job.status = JobStatus::Failed;
                    job.error_code = Some(code.to_string());
                    job.error_message = Some(message.clone());
                    let _ = std::fs::remove_dir_all(&task_dir);
                    finish_job(&inner, &on_event, &jid, job);
                }
            });
        }

        Ok(job_id)
    }

    /// 取消当前任务（终止 Worker → 等待 → 清理 → Cancelled）。
    pub fn cancel(&self) -> Result<(), TranscriptionError> {
        let (child, cancelled) = {
            let guard = self.inner.lock().unwrap();
            match guard.as_ref() {
                Some(a) if !a.job.status.is_terminal() => (a.child.clone(), a.cancelled.clone()),
                None => {
                    return Err(TranscriptionError::new(
                        TranscriptionErrorCode::JobCancelled,
                        "没有进行中的转录任务",
                    ));
                }
                Some(_) => {
                    return Err(TranscriptionError::new(
                        TranscriptionErrorCode::JobCancelled,
                        "没有进行中的转录任务",
                    ));
                }
            }
        };
        cancelled.store(true, Ordering::SeqCst);
        if let Some(mut c) = child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // 等待线程会完成清理与状态更新；这里等待最多 3 秒
        for _ in 0..30 {
            let done = {
                let guard = self.inner.lock().unwrap();
                guard
                    .as_ref()
                    .map(|a| a.job.status == JobStatus::Cancelled)
                    .unwrap_or(false)
            };
            if done {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// 当前任务状态。
    pub fn status(&self) -> Option<TranscriptionJob> {
        self.inner.lock().unwrap().as_ref().map(|a| a.job.clone())
    }

    /// 停止活动任务（程序退出时调用；不阻塞过久）。
    pub fn shutdown(&self) {
        let _ = self.cancel();
    }
}

/// 将失败任务发布为终态并发出 Error 事件。
fn finish_job(
    inner: &Mutex<Option<ActiveJob>>,
    on_event: &EventFn,
    job_id: &str,
    job: TranscriptionJob,
) {
    {
        let mut guard = inner.lock().unwrap();
        if guard.as_ref().map(|a| a.job.job_id.as_str()) == Some(job_id) {
            guard.as_mut().unwrap().job = job.clone();
        }
    }
    on_event(TranscriptionEvent::Error {
        job_id: job_id.into(),
        code: job
            .error_code
            .clone()
            .unwrap_or_else(|| "INTERNAL_ERROR".into()),
        message: job
            .error_message
            .clone()
            .unwrap_or_else(|| "转录失败".into()),
    });
}
