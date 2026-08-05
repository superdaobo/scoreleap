//! 转录服务：管理原生 ONNX sidecar 生命周期、解析 JSON Lines、导入结果 MIDI。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{TranscriptionError, TranscriptionErrorCode};
use crate::job::{JobStatus, TranscriptionJob};
use crate::protocol::WorkerMsg;

/// Worker 启动规格（参数数组；禁止 shell 字符串拼接）。
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    pub program: String,
    pub args: Vec<String>,
    pub model_path: PathBuf,
    pub onnx_runtime_path: Option<PathBuf>,
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

type EventFn = Arc<dyn Fn(TranscriptionEvent) + Send + Sync>;
type ImporterFn = Arc<dyn Fn(&str, &str) -> Result<String, String> + Send + Sync>;

struct ActiveJob {
    job: TranscriptionJob,
    task_dir: PathBuf,
    child: Arc<Mutex<Option<Child>>>,
    cancelled: Arc<AtomicBool>,
}

/// 转录服务（单任务并发；第二个任务返回 TRANSCRIPTION_BUSY）。
/// Clone 共享同一任务状态（内部为 Arc）。
#[derive(Clone)]
pub struct TranscriptionService {
    inner: Arc<Mutex<Option<ActiveJob>>>,
    /// 串行化“检查空闲 → 启动进程 → 发布任务”，防止两个命令同时穿透单任务限制。
    start_lock: Arc<Mutex<()>>,
    worker: WorkerSpec,
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
    pub fn new(
        data_dir: PathBuf,
        worker: WorkerSpec,
        on_event: EventFn,
        importer: ImporterFn,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            start_lock: Arc::new(Mutex::new(())),
            worker,
            data_dir,
            on_event,
            importer,
            last_error_code: Arc::new(Mutex::new(None)),
            last_error_message: Arc::new(Mutex::new(None)),
        }
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
        if !self.worker.model_path.is_file() {
            return Err(TranscriptionError::new(
                TranscriptionErrorCode::ModelDownloadRequired,
                "尚未安装可用的转录模型，请先在设置中下载",
            ));
        }
        if self
            .worker
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

        // 启动 Worker（参数数组；无 shell；路径由后端决定）
        let mut cmd = Command::new(&self.worker.program);
        cmd.args(&self.worker.args)
            .arg("transcribe")
            .arg("--request-id")
            .arg(&request_id)
            .arg("--input")
            .arg(input_path)
            .arg("--output-midi")
            .arg(&midi_path)
            .arg("--output-metadata")
            .arg(&metadata_path)
            .arg("--model")
            .arg(&self.worker.model_path)
            .arg("--preset")
            .arg(options.preset.as_str())
            .stdin(Stdio::null());
        if let Some(runtime) = &self.worker.onnx_runtime_path {
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
                        Ok(msg) => handle_worker_msg(
                            &WorkerMessageContext {
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
                    // 导入曲谱库（共享入口）
                    let base = Path::new(&job.source_name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "转录曲谱".into());
                    let display_name = format!("{base}（音频转录）");
                    match importer(&midi_path, &display_name) {
                        Ok(doc_id) => {
                            job.status = JobStatus::Completed;
                            job.result_doc_id = Some(doc_id.clone());
                            job.message = "转录完成".into();
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
                    let (code, message) = map_exit_code(code, &protocol_code, &protocol_message);
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

/// 退出码 → 结构化错误码（Worker 契约）。
fn map_exit_code(
    code: i32,
    last_code: &Option<String>,
    last_msg: &Option<String>,
) -> (String, String) {
    // Worker 已通过 JSONL 给出结构化错误时，以协议内容为准，避免退出码降级信息。
    if let Some(worker_code) = last_code {
        return (
            worker_code.clone(),
            last_msg
                .clone()
                .unwrap_or_else(|| "转录组件返回错误".into()),
        );
    }
    let mapped = match code {
        2 => Some((
            TranscriptionErrorCode::WorkerProtocolError,
            "Worker 参数错误".into(),
        )),
        3 => Some((
            TranscriptionErrorCode::InvalidAudioPath,
            "Worker 输入错误".into(),
        )),
        4 => Some((
            TranscriptionErrorCode::AudioDecodeFailed,
            "音频解码失败".into(),
        )),
        5 => Some((
            TranscriptionErrorCode::ModelLoadFailed,
            "模型加载失败".into(),
        )),
        6 => Some((
            TranscriptionErrorCode::InferenceFailed,
            "音符识别失败".into(),
        )),
        7 => Some((
            TranscriptionErrorCode::MidiWriteFailed,
            "MIDI 写入失败".into(),
        )),
        8 => Some((TranscriptionErrorCode::JobCancelled, "任务已取消".into())),
        9 => Some((
            TranscriptionErrorCode::InternalError,
            "Worker 内部错误".into(),
        )),
        _ => None,
    };
    mapped
        .map(|(code, message)| (code.as_str().into(), message))
        .unwrap_or_else(|| {
            (
                TranscriptionErrorCode::WorkerExitedUnexpectedly
                    .as_str()
                    .into(),
                format!("Worker 异常退出（退出码 {code}）"),
            )
        })
}

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

struct WorkerMessageContext<'a> {
    inner: &'a Mutex<Option<ActiveJob>>,
    on_event: &'a EventFn,
    last_code: &'a Mutex<Option<String>>,
    last_msg: &'a Mutex<Option<String>>,
    saw_result: &'a AtomicBool,
    job_id: &'a str,
    expected_request_id: &'a str,
}

fn handle_worker_msg(context: &WorkerMessageContext<'_>, msg: WorkerMsg) {
    if msg.schema_version != Some(1)
        || msg.request_id.as_deref() != Some(context.expected_request_id)
    {
        *context.last_code.lock().unwrap() =
            Some(TranscriptionErrorCode::WorkerProtocolError.as_str().into());
        *context.last_msg.lock().unwrap() = Some("Worker schema_version 或 request_id 无效".into());
        return;
    }
    match msg.msg_type.as_str() {
        "ready" => {
            if let Some(v) = msg.worker_version {
                let mut guard = context.inner.lock().unwrap();
                if let Some(a) = guard.as_mut() {
                    if a.job.job_id == context.job_id {
                        a.job.message = format!("Worker {v} 就绪");
                    }
                }
            }
        }
        "stage" => {
            let stage = msg.stage.unwrap_or_default();
            let message = msg.message.unwrap_or_default();
            let status = match stage.as_str() {
                "validating_input" => JobStatus::ValidatingInput,
                "loading_model" => JobStatus::LoadingModel,
                "transcribing" => JobStatus::Transcribing,
                "writing_midi" => JobStatus::WritingMidi,
                _ => JobStatus::Starting,
            };
            {
                let mut guard = context.inner.lock().unwrap();
                if let Some(a) = guard.as_mut() {
                    if a.job.job_id == context.job_id {
                        a.job.status = status;
                        a.job.stage = stage.clone();
                        a.job.message = message.clone();
                    }
                }
            }
            (context.on_event)(TranscriptionEvent::Stage {
                job_id: context.job_id.into(),
                stage,
                message,
            });
        }
        "result" => {
            let mut guard = context.inner.lock().unwrap();
            if let Some(a) = guard.as_mut() {
                if a.job.job_id == context.job_id {
                    let paths_match = msg.midi_path.as_deref() == a.job.midi_path.as_deref()
                        && msg.metadata_path.as_deref() == a.job.metadata_path.as_deref();
                    if !paths_match || msg.elapsed_ms.is_none() || msg.note_count.is_none() {
                        *context.last_code.lock().unwrap() =
                            Some(TranscriptionErrorCode::WorkerProtocolError.as_str().into());
                        *context.last_msg.lock().unwrap() =
                            Some("Worker result 字段缺失或输出路径不匹配".into());
                        return;
                    }
                    a.job.note_count = msg.note_count;
                    a.job.elapsed_ms = msg.elapsed_ms.unwrap_or(0);
                    context.saw_result.store(true, Ordering::Release);
                }
            }
        }
        "error" => {
            *context.last_code.lock().unwrap() = Some(
                msg.code
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| TranscriptionErrorCode::WorkerProtocolError.as_str().into()),
            );
            *context.last_msg.lock().unwrap() = msg
                .message
                .clone()
                .or(msg.detail.clone())
                .or_else(|| Some("Worker 返回了未说明的错误".into()));
            tracing::warn!(
                job_id = context.job_id,
                "worker-error: {:?} {:?}",
                msg.code,
                msg.message
            );
        }
        other => {
            tracing::debug!(job_id = context.job_id, "忽略未知 Worker 消息类型: {other}");
        }
    }
}
