use std::path::{Path, PathBuf};
use std::time::Instant;

use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use serde::{Deserialize, Serialize};

use crate::{
    activations_to_notes, stitch_outputs, AudioWindows, NoteEvent, PianoPreset, ResolvedThresholds,
    StitchedActivations, ThresholdOverrides, TranscribeError, WindowActivations, AUDIO_SAMPLE_RATE,
    AUDIO_WINDOW_SAMPLES, CONTOUR_BINS, MODEL_OUTPUT_FRAMES, NOTE_BINS, PROTOCOL_VERSION,
};

const BASIC_PITCH_INPUT_NAMES: &[&str] = &["serving_default_input_2:0", "input_2", "audio"];
const NOTE_OUTPUT_NAMES: &[&str] = &["StatefulPartitionedCall:1", "note", "notes"];
const ONSET_OUTPUT_NAMES: &[&str] = &["StatefulPartitionedCall:2", "onset", "onsets"];
const CONTOUR_OUTPUT_NAMES: &[&str] = &["StatefulPartitionedCall:0", "contour", "contours"];

pub type ModelActivations = StitchedActivations;

/// 必须在任何 `ort` Session 创建前调用。`ort` 会同时检查 DLL 最低 API 版本。
pub fn initialize_onnx_runtime(runtime_path: impl AsRef<Path>) -> Result<(), TranscribeError> {
    let path = runtime_path.as_ref();
    if !path.is_file() {
        return Err(TranscribeError::RuntimeInitialization(format!(
            "动态库不存在或不是普通文件: {}",
            path.display()
        )));
    }
    let builder = ort::init_from(path)
        .map_err(|error| TranscribeError::RuntimeInitialization(error.to_string()))?;
    let _was_committed = builder.with_name("scoreleap-transcriber-native").commit();
    Ok(())
}

#[derive(Debug, Clone)]
struct OutputNames {
    note: String,
    onset: String,
    contour: String,
}

pub struct BasicPitchModel {
    session: Session,
    input_name: String,
    outputs: OutputNames,
    model_path: PathBuf,
}

impl BasicPitchModel {
    pub fn load(model_path: impl AsRef<Path>) -> Result<Self, TranscribeError> {
        let model_path = model_path.as_ref();
        if !model_path.is_file() {
            return Err(TranscribeError::ModelLoad(format!(
                "模型不存在或不是普通文件: {}",
                model_path.display()
            )));
        }
        let session = Session::builder()
            .map_err(|error| TranscribeError::ModelLoad(error.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|error| TranscribeError::ModelLoad(error.to_string()))?
            .with_intra_threads(default_intra_threads())
            .map_err(|error| TranscribeError::ModelLoad(error.to_string()))?
            .commit_from_file(model_path)
            .map_err(|error| TranscribeError::ModelLoad(error.to_string()))?;

        let input_names: Vec<_> = session.inputs().iter().map(|item| item.name()).collect();
        let output_names: Vec<_> = session.outputs().iter().map(|item| item.name()).collect();
        let input_name = resolve_name(&input_names, BASIC_PITCH_INPUT_NAMES, "输入")?;
        let outputs = OutputNames {
            note: resolve_name(&output_names, NOTE_OUTPUT_NAMES, "note 输出")?,
            onset: resolve_name(&output_names, ONSET_OUTPUT_NAMES, "onset 输出")?,
            contour: resolve_name(&output_names, CONTOUR_OUTPUT_NAMES, "contour 输出")?,
        };
        // 在构造阶段完成接口解析，推理循环只执行张量工作。
        Ok(Self {
            session,
            input_name,
            outputs,
            model_path: model_path.to_path_buf(),
        })
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    pub fn infer_window(&mut self, window: &[f32]) -> Result<WindowActivations, TranscribeError> {
        if window.len() != AUDIO_WINDOW_SAMPLES {
            return Err(TranscribeError::Inference(format!(
                "模型窗口必须是 {AUDIO_WINDOW_SAMPLES} 个样本，实际 {}",
                window.len()
            )));
        }
        if window.iter().any(|sample| !sample.is_finite()) {
            return Err(TranscribeError::Inference(
                "模型输入包含 NaN 或无穷值".into(),
            ));
        }
        let input = Tensor::from_array(([1, AUDIO_WINDOW_SAMPLES, 1], window.to_vec()))
            .map_err(|error| TranscribeError::Inference(error.to_string()))?;
        let outputs = self
            .session
            .run(ort::inputs![self.input_name.as_str() => input])
            .map_err(|error| TranscribeError::Inference(error.to_string()))?;

        Ok(WindowActivations {
            frames: extract_output(&outputs, &self.outputs.note, NOTE_BINS)?,
            onsets: extract_output(&outputs, &self.outputs.onset, NOTE_BINS)?,
            contours: extract_output(&outputs, &self.outputs.contour, CONTOUR_BINS)?,
        })
    }
}

fn default_intra_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 4))
        .unwrap_or(1)
}

fn resolve_name(
    actual: &[&str],
    candidates: &[&str],
    description: &str,
) -> Result<String, TranscribeError> {
    for candidate in candidates {
        if actual.iter().any(|actual| actual == candidate) {
            return Ok((*candidate).to_string());
        }
    }
    Err(TranscribeError::ModelInterface(format!(
        "找不到 {description}；模型提供: {}",
        actual.join(", ")
    )))
}

fn extract_output(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    frequency_bins: usize,
) -> Result<Vec<f32>, TranscribeError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| TranscribeError::InvalidOutput(format!("推理结果缺少输出 `{name}`")))?;
    let (shape, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|error| TranscribeError::InvalidOutput(error.to_string()))?;
    let expected = [1_i64, MODEL_OUTPUT_FRAMES as i64, frequency_bins as i64];
    if **shape != expected {
        return Err(TranscribeError::InvalidOutput(format!(
            "输出 `{name}` 形状错误: 期望 {expected:?}，实际 {:?}",
            &**shape
        )));
    }
    if data.iter().any(|value| !value.is_finite()) {
        return Err(TranscribeError::InvalidOutput(format!(
            "输出 `{name}` 包含 NaN 或无穷值"
        )));
    }
    Ok(data.to_vec())
}

pub struct Transcriber {
    model: BasicPitchModel,
    preset: PianoPreset,
    thresholds: ResolvedThresholds,
}

impl Transcriber {
    pub fn new(
        model_path: impl AsRef<Path>,
        preset: PianoPreset,
        overrides: ThresholdOverrides,
    ) -> Result<Self, TranscribeError> {
        let thresholds = overrides.resolve(preset)?;
        Ok(Self {
            model: BasicPitchModel::load(model_path)?,
            preset,
            thresholds,
        })
    }

    pub fn transcribe_file<F>(
        &mut self,
        audio_path: impl AsRef<Path>,
        mut progress: F,
    ) -> Result<TranscriptionResult, TranscribeError>
    where
        F: FnMut(usize, usize),
    {
        let started = Instant::now();
        let config = scoreleap_audio::AudioConfig {
            target_sample_rate: AUDIO_SAMPLE_RATE,
            ..scoreleap_audio::AudioConfig::default()
        };
        let audio = scoreleap_audio::decode_file(audio_path.as_ref(), &config)?;
        let original_samples = audio.samples.len();
        let duration_seconds = audio.duration_seconds();
        let windows = AudioWindows::new(&audio.samples);
        let total_windows = windows.len();
        let mut outputs = Vec::with_capacity(total_windows);
        for (index, window) in windows.enumerate() {
            outputs.push(self.model.infer_window(&window)?);
            progress(index + 1, total_windows);
        }
        let activations = stitch_outputs(&outputs, original_samples)?;
        let notes = activations_to_notes(&activations, self.thresholds)?;
        let metadata = TranscriptionMetadata {
            schema_version: PROTOCOL_VERSION,
            engine: "scoreleap-native-basic-pitch".into(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            model_file: self
                .model
                .model_path()
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "model.onnx".into()),
            preset: self.preset,
            thresholds: self.thresholds,
            sample_rate: audio.sample_rate,
            original_samples,
            duration_seconds,
            model_frame_count: activations.frame_count,
            note_count: notes.len(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            notes: notes.clone(),
        };
        Ok(TranscriptionResult { notes, metadata })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionMetadata {
    pub schema_version: u32,
    pub engine: String,
    pub engine_version: String,
    pub model_file: String,
    pub preset: PianoPreset,
    pub thresholds: ResolvedThresholds,
    pub sample_rate: u32,
    pub original_samples: usize,
    pub duration_seconds: f64,
    pub model_frame_count: usize,
    pub note_count: usize,
    pub elapsed_ms: u64,
    pub notes: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub notes: Vec<NoteEvent>,
    pub metadata: TranscriptionMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_name_adapter_accepts_official_and_aliases() {
        assert_eq!(
            resolve_name(
                &["StatefulPartitionedCall:1", "StatefulPartitionedCall:2"],
                NOTE_OUTPUT_NAMES,
                "note"
            )
            .unwrap(),
            "StatefulPartitionedCall:1"
        );
        assert_eq!(
            resolve_name(&["note"], NOTE_OUTPUT_NAMES, "note").unwrap(),
            "note"
        );
        assert!(resolve_name(&["unknown"], NOTE_OUTPUT_NAMES, "note").is_err());
    }

    #[test]
    #[ignore = "需要显式设置 SCORELEAP_REAL_MODEL 与 SCORELEAP_ONNX_RUNTIME"]
    fn real_basic_pitch_model_runs_one_window() {
        let model = std::env::var_os("SCORELEAP_REAL_MODEL").expect("SCORELEAP_REAL_MODEL");
        let runtime = std::env::var_os("SCORELEAP_ONNX_RUNTIME").expect("SCORELEAP_ONNX_RUNTIME");
        initialize_onnx_runtime(runtime).unwrap();
        let mut model = BasicPitchModel::load(model).unwrap();
        let output = model
            .infer_window(&vec![0.0; AUDIO_WINDOW_SAMPLES])
            .unwrap();
        assert_eq!(output.frames.len(), MODEL_OUTPUT_FRAMES * NOTE_BINS);
        assert_eq!(output.onsets.len(), MODEL_OUTPUT_FRAMES * NOTE_BINS);
        assert_eq!(output.contours.len(), MODEL_OUTPUT_FRAMES * CONTOUR_BINS);
    }
}
