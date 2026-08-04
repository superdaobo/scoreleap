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
}

/** 卷帘预览音符（get_sequence_notes 返回值；编排后、按键编译前） */
export interface NoteView {
  note: number
  start_us: number
  duration_us: number
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
