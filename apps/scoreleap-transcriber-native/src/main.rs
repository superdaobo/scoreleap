use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use scoreleap_transcribe::{
    initialize_onnx_runtime, write_midi, PianoPreset, ThresholdOverrides, TranscribeError,
    Transcriber, PROTOCOL_VERSION,
};
use serde::Serialize;

#[derive(Debug, Clone)]
struct CliArgs {
    request_id: String,
    input: PathBuf,
    output_midi: PathBuf,
    output_metadata: PathBuf,
    model: PathBuf,
    onnx_runtime: PathBuf,
    preset: PianoPreset,
    overrides: ThresholdOverrides,
}

#[derive(Debug, Serialize)]
struct WorkerMessage<'a> {
    schema_version: u32,
    #[serde(rename = "type")]
    message_type: &'a str,
    request_id: &'a str,
    timestamp_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    worker_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    midi_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl<'a> WorkerMessage<'a> {
    fn base(message_type: &'a str, request_id: &'a str) -> Self {
        Self {
            schema_version: PROTOCOL_VERSION,
            message_type,
            request_id,
            timestamp_ms: now_ms(),
            worker_version: None,
            stage: None,
            message: None,
            progress: None,
            midi_path: None,
            metadata_path: None,
            elapsed_ms: None,
            note_count: None,
            code: None,
            detail: None,
        }
    }
}

fn main() {
    let args = match CliArgs::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            emit_error("unknown", &error);
            eprintln!("参数解析失败: {error}");
            std::process::exit(2);
        }
    };
    emit(WorkerMessage {
        worker_version: Some(env!("CARGO_PKG_VERSION")),
        ..WorkerMessage::base("ready", &args.request_id)
    });
    if let Err(error) = run(&args) {
        emit_error(&args.request_id, &error);
        eprintln!("转录失败 [{}]: {error}", error.code().as_str());
        std::process::exit(1);
    }
}

fn run(args: &CliArgs) -> Result<(), TranscribeError> {
    emit_stage(
        &args.request_id,
        "validating_input",
        "正在验证音频与模型文件",
        0.0,
    );
    if !args.input.is_file() {
        return Err(TranscribeError::InvalidAudioPath(format!(
            "文件不存在或不是普通文件: {}",
            args.input.display()
        )));
    }
    if !args.model.is_file() {
        return Err(TranscribeError::ModelLoad(format!(
            "模型不存在或不是普通文件: {}",
            args.model.display()
        )));
    }
    if !args.onnx_runtime.is_file() {
        return Err(TranscribeError::RuntimeInitialization(format!(
            "动态库不存在或不是普通文件: {}",
            args.onnx_runtime.display()
        )));
    }

    emit_stage(
        &args.request_id,
        "loading_model",
        "正在加载本地 ONNX 模型",
        0.05,
    );
    initialize_onnx_runtime(&args.onnx_runtime)?;
    let mut transcriber = Transcriber::new(args.model.clone(), args.preset, args.overrides)?;

    emit_stage(
        &args.request_id,
        "decoding_audio",
        "正在解码并重采样音频",
        0.10,
    );
    let result = transcriber.transcribe_file(&args.input, |completed, total| {
        let fraction = if total == 0 {
            1.0
        } else {
            completed as f32 / total as f32
        };
        emit_stage(
            &args.request_id,
            "transcribing",
            "正在识别钢琴音符",
            0.10 + fraction * 0.80,
        );
    })?;

    emit_stage(
        &args.request_id,
        "writing_midi",
        "正在写入并验证 MIDI",
        0.92,
    );
    write_midi(&args.output_midi, &result.notes)?;
    write_metadata(&args.output_metadata, &result.metadata)?;

    emit(WorkerMessage {
        midi_path: Some(args.output_midi.to_string_lossy().into_owned()),
        metadata_path: Some(args.output_metadata.to_string_lossy().into_owned()),
        elapsed_ms: Some(result.metadata.elapsed_ms),
        note_count: Some(result.notes.len() as u64),
        progress: Some(1.0),
        ..WorkerMessage::base("result", &args.request_id)
    });
    Ok(())
}

impl CliArgs {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, TranscribeError> {
        let mut arguments = arguments.into_iter();
        let command = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| TranscribeError::InvalidOptions("缺少 `transcribe` 子命令".into()))?;
        if command != "transcribe" {
            return Err(TranscribeError::InvalidOptions(format!(
                "不支持的子命令 `{command}`"
            )));
        }

        let mut request_id = None;
        let mut input = None;
        let mut output_midi = None;
        let mut output_metadata = None;
        let mut model = None;
        let mut runtime = None;
        let mut preset = PianoPreset::default();
        let mut overrides = ThresholdOverrides::default();
        while let Some(flag) = arguments.next() {
            let flag = flag
                .into_string()
                .map_err(|_| TranscribeError::InvalidOptions("参数名必须是有效 UTF-8".into()))?;
            let value = arguments
                .next()
                .ok_or_else(|| TranscribeError::InvalidOptions(format!("参数 `{flag}` 缺少值")))?;
            match flag.as_str() {
                "--request-id" => request_id = Some(os_to_string(value, &flag)?),
                "--input" => input = Some(PathBuf::from(value)),
                "--output-midi" => output_midi = Some(PathBuf::from(value)),
                "--output-metadata" => output_metadata = Some(PathBuf::from(value)),
                "--model" => model = Some(PathBuf::from(value)),
                "--onnx-runtime" => runtime = Some(PathBuf::from(value)),
                "--preset" => preset = os_to_string(value, &flag)?.parse()?,
                "--onset-threshold" => overrides.onset = Some(parse_number(value, &flag)?),
                "--frame-threshold" => overrides.frame = Some(parse_number(value, &flag)?),
                "--minimum-note-length-ms" => {
                    overrides.minimum_note_length_ms = Some(parse_number(value, &flag)?)
                }
                "--energy-tolerance" => {
                    overrides.energy_tolerance_frames = Some(parse_number(value, &flag)?)
                }
                "--duplicate-gap-ms" => {
                    overrides.duplicate_gap_ms = Some(parse_number(value, &flag)?)
                }
                _ => {
                    return Err(TranscribeError::InvalidOptions(format!(
                        "未知参数 `{flag}`"
                    )))
                }
            }
        }
        overrides.resolve(preset)?;

        Ok(Self {
            request_id: required(request_id, "--request-id")?,
            input: required(input, "--input")?,
            output_midi: required(output_midi, "--output-midi")?,
            output_metadata: required(output_metadata, "--output-metadata")?,
            model: model
                .or_else(|| std::env::var_os("SCORELEAP_MODEL_PATH").map(PathBuf::from))
                .ok_or_else(|| {
                    TranscribeError::InvalidOptions("缺少 --model 或 SCORELEAP_MODEL_PATH".into())
                })?,
            onnx_runtime: runtime
                .or_else(|| std::env::var_os("SCORELEAP_ONNX_RUNTIME_PATH").map(PathBuf::from))
                .or_else(|| std::env::var_os("ORT_DYLIB_PATH").map(PathBuf::from))
                .ok_or_else(|| {
                    TranscribeError::InvalidOptions(
                        "缺少 --onnx-runtime、SCORELEAP_ONNX_RUNTIME_PATH 或 ORT_DYLIB_PATH".into(),
                    )
                })?,
            preset,
            overrides,
        })
    }
}

fn required<T>(value: Option<T>, name: &str) -> Result<T, TranscribeError> {
    value.ok_or_else(|| TranscribeError::InvalidOptions(format!("缺少必需参数 {name}")))
}

fn os_to_string(value: OsString, name: &str) -> Result<String, TranscribeError> {
    value
        .into_string()
        .map_err(|_| TranscribeError::InvalidOptions(format!("参数 {name} 必须是有效 UTF-8")))
}

fn parse_number<T: std::str::FromStr>(value: OsString, name: &str) -> Result<T, TranscribeError> {
    os_to_string(value, name)?
        .parse()
        .map_err(|_| TranscribeError::InvalidOptions(format!("参数 {name} 不是有效数字")))
}

fn write_metadata(
    path: &Path,
    metadata: &scoreleap_transcribe::TranscriptionMetadata,
) -> Result<(), TranscribeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            TranscribeError::MetadataWrite(format!("创建输出目录失败: {error}"))
        })?;
    }
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|error| TranscribeError::MetadataWrite(error.to_string()))?;
    let mut file = std::fs::File::create(path)
        .map_err(|error| TranscribeError::MetadataWrite(error.to_string()))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| TranscribeError::MetadataWrite(error.to_string()))
}

fn emit_stage(request_id: &str, stage: &str, message: &str, progress: f32) {
    emit(WorkerMessage {
        stage: Some(stage),
        message: Some(message),
        progress: Some(progress.clamp(0.0, 1.0)),
        ..WorkerMessage::base("stage", request_id)
    });
}

fn emit_error(request_id: &str, error: &TranscribeError) {
    let message = error.to_string();
    emit(WorkerMessage {
        code: Some(error.code().as_str()),
        message: Some(&message),
        detail: Some(message.clone()),
        ..WorkerMessage::base("error", request_id)
    });
}

fn emit(message: WorkerMessage<'_>) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    if serde_json::to_writer(&mut lock, &message).is_ok() {
        let _ = lock.write_all(b"\n");
        let _ = lock.flush();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_args() -> Vec<OsString> {
        [
            "transcribe",
            "--request-id",
            "request-1",
            "--input",
            "input.mp3",
            "--output-midi",
            "output.mid",
            "--output-metadata",
            "metadata.json",
            "--model",
            "model.onnx",
            "--onnx-runtime",
            "onnxruntime.dll",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn parses_existing_service_arguments() {
        let args = CliArgs::parse(complete_args()).unwrap();
        assert_eq!(args.request_id, "request-1");
        assert_eq!(args.preset, PianoPreset::PianoBalanced);
        assert_eq!(args.input, PathBuf::from("input.mp3"));
    }

    #[test]
    fn parses_preset_and_threshold_overrides() {
        let mut args = complete_args();
        args.extend([
            OsString::from("--preset"),
            OsString::from("piano_noise_reduced"),
            OsString::from("--onset-threshold"),
            OsString::from("0.63"),
        ]);
        let args = CliArgs::parse(args).unwrap();
        assert_eq!(args.preset, PianoPreset::PianoNoiseReduced);
        assert_eq!(args.overrides.onset, Some(0.63));
    }

    #[test]
    fn rejects_unknown_or_invalid_arguments() {
        let mut unknown = complete_args();
        unknown.extend([OsString::from("--mystery"), OsString::from("x")]);
        assert!(CliArgs::parse(unknown).is_err());

        let mut invalid = complete_args();
        invalid.extend([OsString::from("--frame-threshold"), OsString::from("2.0")]);
        assert!(CliArgs::parse(invalid).is_err());
    }

    #[test]
    fn protocol_message_is_schema_v1_json_line_safe() {
        let message = WorkerMessage {
            stage: Some("transcribing"),
            progress: Some(0.5),
            ..WorkerMessage::base("stage", "r1")
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(!json.contains('\n'));
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"type\":\"stage\""));
    }
}
