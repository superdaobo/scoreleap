/**
 * ScoreLeap 前后端共享类型定义。
 * 字段名与 Rust 端 serde 序列化结果严格对应（snake_case）。
 */

/** 导入结果摘要（import_midi 返回值） */
export interface ImportSummary {
  doc_id: string
  name: string
  format: string
  track_count: number
  note_count: number
  duration_ms: number
  bpm_range: [number, number]
}

/** 曲谱库条目摘要（list_documents 返回值；重启后从后端持久化曲谱库读取） */
export interface DocumentSummary {
  doc_id: string
  name: string
  format: string
  track_count: number
  note_count: number
  duration_ms: number
  bpm_range: [number, number]
  /** 来源类型：midi（直接导入）/ audio_transcription（音频转录） */
  source_type: string
}

/** 转录任务视图（命令 get_audio_transcription_status；camelCase 由 Tauri 转换） */
export type TranscriptionStatus =
  | 'Queued'
  | 'Starting'
  | 'ValidatingInput'
  | 'LoadingModel'
  | 'Transcribing'
  | 'WritingMidi'
  | 'ImportingMidi'
  | 'Completed'
  | 'Failed'
  | 'Cancelled'

export interface TranscriptionJobView {
  job_id: string
  request_id: string
  source_name: string
  status: TranscriptionStatus
  stage: string
  message: string
  started_at_ms: number
  elapsed_ms: number
  note_count: number | null
  midi_path: string | null
  metadata_path: string | null
  result_doc_id: string | null
  error_code: string | null
  error_message: string | null
}

/** transcription://completed 事件载荷 */
export interface TranscriptionCompletedPayload {
  job_id: string
  doc_id: string
  midi_path: string
  note_count: number
  elapsed_ms: number
}

/** transcription://stage 事件载荷 */
export interface TranscriptionStagePayload {
  job_id: string
  stage: string
  message: string
}

/** transcription://error 事件载荷 */
export interface TranscriptionErrorPayload {
  job_id: string
  code: string
  message: string
}

export type TranscriptionPreset = 'balanced' | 'detail' | 'noise_reduced'

export interface TranscriptionOptions {
  preset: TranscriptionPreset
  onset_threshold: number | null
  frame_threshold: number | null
  minimum_note_ms: number | null
}

export type ModelStatus =
  | 'unknown'
  | 'configuration_missing'
  | 'not_installed'
  | 'ready'
  | 'update_available'
  | 'downloading'
  | 'failed'

export interface ModelStatusView {
  status: ModelStatus
  configured: boolean
  model_id: string
  installed_version: string | null
  latest_version: string | null
  size_bytes: number | null
  source: string | null
  received_bytes: number
  total_bytes: number | null
  error: string | null
  can_rollback: boolean
}

export interface ModelDownloadProgress {
  phase: 'connecting' | 'receiving' | 'completed'
  received_bytes: number
  total_bytes: number | null
  source: string
}

/** 卷帘预览音符（get_sequence_notes 返回值；编排后、按键编译前） */
export interface NoteView {
  note: number
  start_us: number
  duration_us: number
}

/** 前台窗口信息（check_foreground 返回值；UIPI 提权检测） */
export interface ForegroundInfo {
  title: string
  pid: number
  elevated: boolean
  our_elevated: boolean
}

/** 键位条目（list_keymap 返回值；测试页逐键测试） */
export interface KeymapEntry {
  note: number
  scan: number
  extended: boolean
  label: string
}

/** 轨道摘要（get_tracks 返回值） */
export interface TrackSummary {
  id: number
  name: string
  note_count: number
  enabled: boolean
}

/** 音域越界策略 */
export type RangeStrategy = 'OctaveDown' | 'Drop' | 'Mute'

/** 量化网格 */
export type QuantizeGrid = 'Eighth' | 'Sixteenth'

/** 编排参数（compile 入参） */
export interface ArrangementOptions {
  transpose_semitones: number
  auto_fit_range: boolean
  range_strategy: RangeStrategy
  max_polyphony: number
  quantize_grid: QuantizeGrid | null
  simplify_chords: boolean
}

/** 编排统计 */
export interface ArrangeStats {
  input_notes: number
  output_notes: number
  dropped_out_of_range: number
  muted: number
  folded: number
  dropped_polyphony: number
  applied_transpose: number
}

/** 编译结果摘要（compile 返回值） */
export interface CompileSummary {
  seq_id: string
  action_count: number
  duration_ms: number
  stats: ArrangeStats
}

/** 播放状态机 */
export type PlaybackState =
  | 'Idle'
  | 'Countdown'
  | 'Playing'
  | 'Paused'
  | 'Stopped'
  | 'Finished'

/** 播放状态摘要（start_playback 返回值） */
export interface PlaybackStatus {
  state: PlaybackState
  position_ms: number
  pressed_keys: number
}

/** 当前演奏中的音符 */
export interface CurrentNote {
  track_id: number
  note: number
  velocity: number
  start_us: number
  duration_us: number
}

/** 播放进度事件（playback://progress） */
export interface PlaybackProgress {
  position_us: number
  current_note: CurrentNote | null
  pressed_keys: number
}

/** 键盘槽位 */
export interface KeySlot {
  note: number
  x: number
  y: number
}

/** 游戏乐器 Profile（load_profile / current_profile 返回值） */
export interface GameProfile {
  id: string
  display_name: string
  version: number
  keys: number
  midi_low: number
  midi_high: number
  max_polyphony: number
  keymap: Record<string, string>
  layout: { keys: KeySlot[] }
  warning: string
}

/** 播放后端：sendinput 注入真实输入；mock 仅记录按键（测试） */
export type PlaybackBackend = 'sendinput' | 'mock'
