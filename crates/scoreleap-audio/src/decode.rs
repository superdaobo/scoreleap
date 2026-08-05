use crate::{AudioConfig, AudioError, AudioInfo, DecodedAudio};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecParameters, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const RESAMPLE_CHUNK_SIZE: usize = 1_024;
const MAX_OUTPUT_FRAMES: usize = 22_050 * 600;

struct ValidatedFile {
    path: PathBuf,
    extension: String,
    file_size_bytes: u64,
}

struct OpenedMedia {
    format: Box<dyn FormatReader>,
    track_id: u32,
    codec_params: CodecParameters,
    sample_rate: u32,
    channels: usize,
}

/// 探测 MP3、WAV 或 FLAC 的源音频信息，不保留解码样本。
pub fn probe_file(path: impl AsRef<Path>, config: &AudioConfig) -> Result<AudioInfo, AudioError> {
    validate_config(config)?;
    let file = validate_file(path.as_ref(), config)?;
    let mut media = open_media(&file)?;
    let duration_seconds = match media.codec_params.n_frames {
        Some(frames) => frames as f64 / f64::from(media.sample_rate),
        None => scan_packet_duration(&mut *media.format, media.track_id, &media.codec_params)?,
    };
    validate_duration(duration_seconds, config)?;
    Ok(audio_info(&file, &media, duration_seconds))
}

/// 解码所有声道为 `f32`，安全下混为单声道，并重采样到配置采样率。
pub fn decode_file(
    path: impl AsRef<Path>,
    config: &AudioConfig,
) -> Result<DecodedAudio, AudioError> {
    validate_config(config)?;
    let file = validate_file(path.as_ref(), config)?;
    let mut media = open_media(&file)?;
    if let Some(frames) = media.codec_params.n_frames {
        validate_duration(frames as f64 / f64::from(media.sample_rate), config)?;
    }

    let mut decoder = symphonia::default::get_codecs()
        .make(&media.codec_params, &DecoderOptions::default())
        .map_err(|error| AudioError::DecoderCreation {
            message: error.to_string(),
        })?;
    let mut mono = Vec::new();
    let max_source_frames = max_source_frames(media.sample_rate, config.max_duration_seconds)?;

    loop {
        let packet = match media.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => {
                return Err(AudioError::Decode {
                    message: error.to_string(),
                })
            }
        };
        if packet.track_id() != media.track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // 质量优先：跳过坏包会静默压缩时间轴，造成后续音符整体提前。
            Err(SymphoniaError::DecodeError(message)) => {
                return Err(AudioError::Decode {
                    message: message.to_owned(),
                })
            }
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => {
                return Err(AudioError::Decode {
                    message: error.to_string(),
                })
            }
        };
        let spec = *decoded.spec();
        if spec.rate != media.sample_rate || spec.channels.count() != media.channels {
            return Err(AudioError::StreamParametersChanged);
        }

        let mut interleaved = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(decoded);
        let incoming_frames = interleaved.samples().len() / media.channels;
        let next_source_frames = mono
            .len()
            .checked_add(incoming_frames)
            .ok_or(AudioError::FrameCountOverflow)?;
        if next_source_frames > max_source_frames {
            return Err(AudioError::DurationExceeded {
                actual_seconds: next_source_frames as f64 / f64::from(media.sample_rate),
                max_seconds: config.max_duration_seconds,
            });
        }
        append_downmixed(&mut mono, interleaved.samples(), media.channels)?;
    }
    if mono.is_empty() {
        return Err(AudioError::EmptyAudio);
    }

    let source_frames = mono.len();
    let samples = if media.sample_rate == config.target_sample_rate {
        mono
    } else {
        resample_mono(&mono, media.sample_rate, config.target_sample_rate)?
    };
    let source = audio_info(
        &file,
        &media,
        source_frames as f64 / f64::from(media.sample_rate),
    );
    Ok(DecodedAudio {
        samples,
        sample_rate: config.target_sample_rate,
        source,
    })
}

fn validate_config(config: &AudioConfig) -> Result<(), AudioError> {
    if config.target_sample_rate != crate::types::DEFAULT_TARGET_SAMPLE_RATE {
        return Err(AudioError::InvalidConfig {
            field: "target_sample_rate",
            reason: "当前模型仅接受 22050Hz",
        });
    }
    if config.max_file_size_bytes == 0 {
        return Err(AudioError::InvalidConfig {
            field: "max_file_size_bytes",
            reason: "必须大于 0",
        });
    }
    if !config.max_duration_seconds.is_finite() || config.max_duration_seconds <= 0.0 {
        return Err(AudioError::InvalidConfig {
            field: "max_duration_seconds",
            reason: "必须是大于 0 的有限数值",
        });
    }
    if config.max_duration_seconds > crate::types::DEFAULT_MAX_DURATION_SECONDS {
        return Err(AudioError::InvalidConfig {
            field: "max_duration_seconds",
            reason: "不得超过 600 秒安全上限",
        });
    }
    Ok(())
}

fn max_source_frames(sample_rate: u32, duration_seconds: f64) -> Result<usize, AudioError> {
    let frames = (f64::from(sample_rate) * duration_seconds).ceil();
    if !frames.is_finite() || frames > usize::MAX as f64 {
        return Err(AudioError::FrameCountOverflow);
    }
    Ok(frames as usize)
}

fn validate_file(path: &Path, config: &AudioConfig) -> Result<ValidatedFile, AudioError> {
    let metadata = fs::metadata(path).map_err(|source| AudioError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(AudioError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !matches!(extension.as_str(), "mp3" | "wav" | "flac") {
        return Err(AudioError::UnsupportedExtension { extension });
    }
    let file_size_bytes = metadata.len();
    if file_size_bytes == 0 {
        return Err(AudioError::EmptyFile {
            path: path.to_path_buf(),
        });
    }
    if file_size_bytes > config.max_file_size_bytes {
        return Err(AudioError::FileTooLarge {
            actual_bytes: file_size_bytes,
            max_bytes: config.max_file_size_bytes,
        });
    }
    Ok(ValidatedFile {
        path: path.to_path_buf(),
        extension,
        file_size_bytes,
    })
}

fn open_media(file: &ValidatedFile) -> Result<OpenedMedia, AudioError> {
    let source = File::open(&file.path).map_err(|source| AudioError::Open {
        path: file.path.clone(),
        source,
    })?;
    let stream = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    hint.with_extension(&file.extension);
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..FormatOptions::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Probe {
            message: error.to_string(),
        })?;
    let format = probed.format;
    let track = format
        .default_track()
        .ok_or(AudioError::MissingDefaultTrack)?;
    let codec_params = track.codec_params.clone();
    let track_id = track.id;
    let sample_rate = codec_params
        .sample_rate
        .filter(|rate| *rate > 0)
        .ok_or(AudioError::MissingSampleRate)?;
    let channels = codec_params
        .channels
        .ok_or(AudioError::MissingChannels)?
        .count();
    if channels == 0 {
        return Err(AudioError::MissingChannels);
    }
    Ok(OpenedMedia {
        format,
        track_id,
        codec_params,
        sample_rate,
        channels,
    })
}

fn scan_packet_duration(
    format: &mut dyn FormatReader,
    track_id: u32,
    params: &CodecParameters,
) -> Result<f64, AudioError> {
    let time_base = params.time_base.ok_or_else(|| AudioError::Probe {
        message: "默认音轨同时缺少总帧数和时间基准".to_owned(),
    })?;
    let mut max_end = 0u64;
    loop {
        match format.next_packet() {
            Ok(packet) if packet.track_id() == track_id => {
                max_end = max_end.max(packet.ts().saturating_add(packet.dur()));
            }
            Ok(_) => {}
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => {
                return Err(AudioError::Probe {
                    message: error.to_string(),
                })
            }
        }
    }
    if max_end == 0 {
        return Err(AudioError::EmptyAudio);
    }
    let duration = time_base.calc_time(max_end);
    Ok(duration.seconds as f64 + duration.frac)
}

fn validate_duration(seconds: f64, config: &AudioConfig) -> Result<(), AudioError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(AudioError::Probe {
            message: "音频时长无效".to_owned(),
        });
    }
    if seconds > config.max_duration_seconds {
        return Err(AudioError::DurationExceeded {
            actual_seconds: seconds,
            max_seconds: config.max_duration_seconds,
        });
    }
    Ok(())
}

fn audio_info(file: &ValidatedFile, media: &OpenedMedia, duration_seconds: f64) -> AudioInfo {
    AudioInfo {
        path: file.path.clone(),
        format: file.extension.clone(),
        file_size_bytes: file.file_size_bytes,
        sample_rate: media.sample_rate,
        channels: media.channels,
        duration_seconds,
    }
}

fn append_downmixed(
    output: &mut Vec<f32>,
    samples: &[f32],
    channels: usize,
) -> Result<(), AudioError> {
    if channels == 0 || !samples.len().is_multiple_of(channels) {
        return Err(AudioError::Decode {
            message: "解码器返回了不完整的音频帧".to_owned(),
        });
    }
    for frame in samples.chunks_exact(channels) {
        let mut sum = 0.0f64;
        for (channel, sample) in frame.iter().enumerate() {
            if !sample.is_finite() {
                let sample_index = output
                    .len()
                    .checked_mul(channels)
                    .and_then(|offset| offset.checked_add(channel))
                    .ok_or(AudioError::FrameCountOverflow)?;
                return Err(AudioError::NonFiniteSample { sample_index });
            }
            sum += f64::from(*sample);
        }
        // 对所有声道取平均可保持满幅同相信号幅度，避免下混削波。
        output.push((sum / channels as f64) as f32);
    }
    Ok(())
}

fn resample_mono(
    input: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> Result<Vec<f32>, AudioError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let ratio = f64::from(target_rate) / f64::from(source_rate);
    let mut resampler = SincFixedIn::<f32>::new(ratio, 1.0, params, RESAMPLE_CHUNK_SIZE, 1)
        .map_err(|error| AudioError::ResamplerCreation {
            message: error.to_string(),
        })?;
    let expected = expected_output_frames(input.len(), source_rate, target_rate)?;
    let delay = resampler.output_delay();
    let required = expected
        .checked_add(delay)
        .ok_or(AudioError::FrameCountOverflow)?;
    let capacity = required
        .checked_add(RESAMPLE_CHUNK_SIZE)
        .ok_or(AudioError::FrameCountOverflow)?;
    let mut output = Vec::with_capacity(capacity);
    let mut offset = 0usize;
    while input.len() - offset >= resampler.input_frames_next() {
        let length = resampler.input_frames_next();
        let chunk = resampler
            .process(&[&input[offset..offset + length]], None)
            .map_err(|error| AudioError::Resample {
                message: error.to_string(),
            })?;
        output.extend_from_slice(&chunk[0]);
        offset += length;
    }
    if offset < input.len() {
        let chunk = resampler
            .process_partial(Some(&[&input[offset..]]), None)
            .map_err(|error| AudioError::Resample {
                message: error.to_string(),
            })?;
        output.extend_from_slice(&chunk[0]);
    }
    // Sinc 输出以 `output_delay` 个延迟帧开头；先完整刷新，再去头保尾。
    while output.len() < required {
        let no_input: Option<&[&[f32]]> = None;
        let chunk =
            resampler
                .process_partial(no_input, None)
                .map_err(|error| AudioError::Resample {
                    message: error.to_string(),
                })?;
        if chunk[0].is_empty() {
            return Err(AudioError::Resample {
                message: "无法刷新到预期长度".to_owned(),
            });
        }
        output.extend_from_slice(&chunk[0]);
    }
    output.copy_within(delay..required, 0);
    output.truncate(expected);
    if let Some(sample_index) = output.iter().position(|sample| !sample.is_finite()) {
        return Err(AudioError::NonFiniteSample { sample_index });
    }
    Ok(output)
}

fn expected_output_frames(
    input_frames: usize,
    source_rate: u32,
    target_rate: u32,
) -> Result<usize, AudioError> {
    let numerator = (input_frames as u128)
        .checked_mul(u128::from(target_rate))
        .and_then(|value| value.checked_add(u128::from(source_rate / 2)))
        .ok_or(AudioError::FrameCountOverflow)?;
    let expected_u128 = numerator / u128::from(source_rate);
    if expected_u128 > MAX_OUTPUT_FRAMES as u128 {
        return Err(AudioError::OutputFrameLimitExceeded {
            actual_frames: expected_u128,
            max_frames: MAX_OUTPUT_FRAMES,
        });
    }
    usize::try_from(expected_u128).map_err(|_| AudioError::FrameCountOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;
    use tempfile::tempdir;

    fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[f32]) {
        assert_eq!(samples.len() % usize::from(channels), 0);
        let size = u32::try_from(samples.len() * 2).unwrap();
        let mut bytes = Vec::with_capacity(44 + size as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn probes_wav() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Piano.WAV");
        write_wav(&path, 8_000, 1, &vec![0.0; 4_000]);
        let info = probe_file(&path, &AudioConfig::default()).unwrap();
        assert_eq!(
            (info.format.as_str(), info.sample_rate, info.channels),
            ("wav", 8_000, 1)
        );
        assert!((info.duration_seconds - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rejects_non_regular_and_unsupported_inputs() {
        let dir = tempdir().unwrap();
        assert!(matches!(
            probe_file(dir.path(), &AudioConfig::default()),
            Err(AudioError::NotRegularFile { .. })
        ));
        let path = dir.path().join("audio.ogg");
        fs::write(&path, b"data").unwrap();
        assert!(matches!(
            probe_file(&path, &AudioConfig::default()),
            Err(AudioError::UnsupportedExtension { .. })
        ));
    }

    #[test]
    fn rejects_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        fs::write(&path, []).unwrap();
        assert!(matches!(
            decode_file(&path, &AudioConfig::default()),
            Err(AudioError::EmptyFile { .. })
        ));
    }

    #[test]
    fn enforces_size_and_duration_limits() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("long.wav");
        write_wav(&path, 8_000, 1, &vec![0.0; 16_000]);
        let size_config = AudioConfig {
            max_file_size_bytes: 16,
            ..AudioConfig::default()
        };
        assert!(matches!(
            probe_file(&path, &size_config),
            Err(AudioError::FileTooLarge { .. })
        ));
        let duration_config = AudioConfig {
            max_duration_seconds: 1.0,
            ..AudioConfig::default()
        };
        assert!(matches!(
            probe_file(&path, &duration_config),
            Err(AudioError::DurationExceeded { .. })
        ));
    }

    #[test]
    fn downmixes_stereo() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        let samples: Vec<f32> = (0..128).flat_map(|_| [0.5, -0.25]).collect();
        write_wav(&path, 22_050, 2, &samples);
        let decoded = decode_file(&path, &AudioConfig::default()).unwrap();
        assert_eq!(decoded.samples.len(), 128);
        assert!(decoded
            .samples
            .iter()
            .all(|sample| (*sample - 0.125).abs() < 1e-3));
    }

    #[test]
    fn resamples_to_default_rate() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let rate = 48_000u32;
        let samples: Vec<f32> = (0..rate)
            .map(|i| (TAU * 440.0 * i as f32 / rate as f32).sin() * 0.5)
            .collect();
        write_wav(&path, rate, 1, &samples);
        let decoded = decode_file(&path, &AudioConfig::default()).unwrap();
        assert_eq!(
            (decoded.sample_rate, decoded.samples.len()),
            (22_050, 22_050)
        );
        assert!(decoded.samples.iter().all(|sample| sample.is_finite()));
        assert!(decoded.samples.iter().any(|sample| sample.abs() > 0.1));
    }

    #[test]
    fn rejects_unsupported_target_rate_and_unsafe_duration_limit() {
        let invalid_rate = AudioConfig {
            target_sample_rate: 44_100,
            ..AudioConfig::default()
        };
        assert!(matches!(
            validate_config(&invalid_rate),
            Err(AudioError::InvalidConfig {
                field: "target_sample_rate",
                ..
            })
        ));

        let unsafe_duration = AudioConfig {
            max_duration_seconds: 601.0,
            ..AudioConfig::default()
        };
        assert!(matches!(
            validate_config(&unsafe_duration),
            Err(AudioError::InvalidConfig {
                field: "max_duration_seconds",
                ..
            })
        ));

        assert!(matches!(
            expected_output_frames(48_000 * 601, 48_000, 22_050),
            Err(AudioError::OutputFrameLimitExceeded { .. })
        ));
    }

    #[test]
    fn compensates_resampler_delay_at_both_ends() {
        let mut input = vec![0.0; 48_000];
        // 在滤波器支撑范围内放置首尾脉冲，避免测试信号本身被零填充边界截断。
        input[256] = 1.0;
        input[47_999 - 256] = 1.0;

        let output = resample_mono(&input, 48_000, 22_050).unwrap();

        assert_eq!(output.len(), 22_050);
        let front = output[..512]
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
            .unwrap();
        let tail = output[output.len() - 512..]
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
            .unwrap();
        assert!(front.1.abs() > 0.2, "front peak: {front:?}");
        assert!(tail.1.abs() > 0.2, "tail peak: {tail:?}");
    }
}
