//! 转录服务集成测试：PowerShell Fake Worker（不依赖 Python/basic-pitch）。
//! Fake Worker 行为由输入文件名关键字控制：slow=延迟、bad=写垃圾 MIDI、fail=退出 9。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use scoreleap_transcription::{
    JobStatus, TranscriptionErrorCode, TranscriptionEvent, TranscriptionService, WorkerSpec,
};

const FAKE_WORKER_PS1: &str = r#"
param($command)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
function Emit($obj) { Write-Output ($obj | ConvertTo-Json -Compress -Depth 4) }
$inputPath = $args[[Array]::IndexOf($args, '--input') + 1]
$midiPath = $args[[Array]::IndexOf($args, '--output-midi') + 1]
Emit @{schema_version=1; type='ready'; request_id='r'; worker_version='0.1.0-test'}
Emit @{schema_version=1; type='stage'; request_id='r'; stage='validating_input'; message='v'}
Emit @{schema_version=1; type='stage'; request_id='r'; stage='loading_model'; message='m'}
if ($inputPath -like '*slow*') { Start-Sleep -Seconds 6 }
Emit @{schema_version=1; type='stage'; request_id='r'; stage='transcribing'; message='t'}
Emit @{schema_version=1; type='result'; request_id='r'; note_count=3; elapsed_ms=100}
if ($inputPath -like '*fail*') { exit 9 }
if ($inputPath -like '*bad*') {
    [System.IO.File]::WriteAllBytes($midiPath, [byte[]](0xDE,0xAD,0xBE,0xEF))
} else {
    [System.IO.File]::WriteAllBytes($midiPath, [byte[]]@(0x4D,0x54,0x68,0x64,0x00,0x00,0x00,0x06,0x00,0x00,0x00,0x01,0x00,0x60,0x4D,0x54,0x72,0x6B,0x00,0x00,0x00,0x0B,0x00,0x90,0x3C,0x64,0x60,0x80,0x3C,0x00,0x00,0xFF,0x2F,0x00))
}
exit 0
"#;

struct Harness {
    service: TranscriptionService,
    events: Arc<Mutex<Vec<TranscriptionEvent>>>,
    imports: Arc<Mutex<Vec<(String, String)>>>,
    data_dir: PathBuf,
}

fn setup(worker_ps1: &str) -> Harness {
    let tmp = std::env::temp_dir().join(format!("scoreleap-tx-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let worker_file = tmp.join("fake_worker.ps1");
    std::fs::write(&worker_file, worker_ps1).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let imports = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    let im = imports.clone();
    let service = TranscriptionService::new(
        tmp.clone(),
        WorkerSpec {
            program: "powershell".into(),
            args: vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                worker_file.to_string_lossy().to_string(),
            ],
        },
        Arc::new(move |e| ev.lock().unwrap().push(e)),
        Arc::new(move |midi, name| {
            im.lock()
                .unwrap()
                .push((midi.to_string(), name.to_string()));
            Ok("doc-transcribed".into())
        }),
    );
    Harness {
        service,
        events,
        imports,
        data_dir: tmp,
    }
}

fn make_input(dir: &PathBuf, name: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, b"fake mp3 bytes").unwrap();
    p.to_string_lossy().to_string()
}

fn wait_terminal(service: &TranscriptionService, secs: u64) -> Option<JobStatus> {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        if let Some(job) = service.status() {
            if job.status.is_terminal() {
                return Some(job.status);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

#[test]
fn success_flow_imports_and_completes() {
    let h = setup(FAKE_WORKER_PS1);
    let input = make_input(&h.data_dir, "in.mp3");
    let job_id = h.service.start(&input).unwrap();
    assert!(!job_id.is_empty());

    let status = wait_terminal(&h.service, 20).expect("应在 20s 内完成");
    assert_eq!(status, JobStatus::Completed);

    // 导入回调收到 MIDI 路径与显示名
    let imports = h.imports.lock().unwrap();
    assert_eq!(imports.len(), 1);
    assert!(imports[0].0.ends_with("generated.mid"));
    assert_eq!(imports[0].1, "in（音频转录）");

    // 事件顺序：state → stage×3 → completed
    let events = h.events.lock().unwrap();
    let types: Vec<&str> = events
        .iter()
        .map(|e| match e {
            TranscriptionEvent::State { .. } => "state",
            TranscriptionEvent::Stage { .. } => "stage",
            TranscriptionEvent::Completed { .. } => "completed",
            TranscriptionEvent::Error { .. } => "error",
        })
        .collect();
    assert_eq!(
        types,
        vec!["state", "stage", "stage", "stage", "stage", "completed"]
    );

    let job = h.service.status().expect("已完成任务应可查询");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.note_count, Some(3));
    assert_eq!(job.result_doc_id.as_deref(), Some("doc-transcribed"));

    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn busy_returns_transcription_busy() {
    let h = setup(FAKE_WORKER_PS1);
    let input = make_input(&h.data_dir, "slow.mp3");
    let _ = h.service.start(&input).unwrap();
    let second = h.service.start(&input);
    assert!(matches!(
        second,
        Err(e) if e.code == TranscriptionErrorCode::TranscriptionBusy
    ));
    wait_terminal(&h.service, 20);
    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn invalid_input_rejected() {
    let h = setup(FAKE_WORKER_PS1);
    // 不存在
    let e = h.service.start(r"C:\no\such\file.mp3").unwrap_err();
    assert_eq!(e.code, TranscriptionErrorCode::InvalidAudioPath);
    // 扩展名
    let wav = make_input(&h.data_dir, "a.wav");
    let e = h.service.start(&wav).unwrap_err();
    assert_eq!(e.code, TranscriptionErrorCode::UnsupportedAudioFormat);
    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn cancel_marks_cancelled_and_cleans_task_dir() {
    let h = setup(FAKE_WORKER_PS1);
    let input = make_input(&h.data_dir, "slow.mp3");
    let job_id = h.service.start(&input).unwrap();
    // 等待进入任务
    std::thread::sleep(Duration::from_millis(800));
    h.service.cancel().unwrap();
    let status = wait_terminal(&h.service, 10).expect("取消应在 10s 内完成");
    assert_eq!(status, JobStatus::Cancelled);
    let job = h.service.status().unwrap();
    assert_eq!(job.error_code.as_deref(), Some("JOB_CANCELLED"));
    // 任务目录已清理
    let task_dir = h.data_dir.join("jobs").join(&job_id);
    assert!(!task_dir.exists(), "任务目录应被清理");
    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn worker_fail_exit_maps_internal_error() {
    let h = setup(FAKE_WORKER_PS1);
    let input = make_input(&h.data_dir, "fail.mp3");
    h.service.start(&input).unwrap();
    let status = wait_terminal(&h.service, 20).expect("应在 20s 内失败");
    assert_eq!(status, JobStatus::Failed);
    let job = h.service.status().unwrap();
    assert_eq!(job.error_code.as_deref(), Some("INTERNAL_ERROR"));
    let events = h.events.lock().unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e, TranscriptionEvent::Error { .. })));
    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn invalid_midi_fails_validation() {
    let h = setup(FAKE_WORKER_PS1);
    let input = make_input(&h.data_dir, "bad.mp3");
    h.service.start(&input).unwrap();
    let status = wait_terminal(&h.service, 20).expect("应在 20s 内失败");
    assert_eq!(status, JobStatus::Failed);
    let job = h.service.status().unwrap();
    assert_eq!(job.error_code.as_deref(), Some("MIDI_VALIDATION_FAILED"));
    std::fs::remove_dir_all(&h.data_dir).ok();
}

#[test]
fn chinese_and_space_paths_work() {
    let h = setup(FAKE_WORKER_PS1);
    let dir = h.data_dir.join("中文 空格 目录");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("测试 音频.mp3");
    std::fs::write(&p, b"bytes").unwrap();
    let input = p.to_string_lossy().to_string();
    let _ = h.service.start(&input).unwrap();
    let status = wait_terminal(&h.service, 20).expect("中文路径应正常完成");
    assert_eq!(status, JobStatus::Completed);
    let imports = h.imports.lock().unwrap();
    assert_eq!(imports[0].1, "测试 音频（音频转录）");
    std::fs::remove_dir_all(&h.data_dir).ok();
}
