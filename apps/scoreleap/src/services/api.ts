/**
 * Tauri 命令封装。函数名与返回类型必须与 Rust 端命令严格一致。
 */
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  ArrangementOptions,
  CompileSummary,
  GameProfile,
  ImportSummary,
  PlaybackBackend,
  PlaybackStatus,
  TrackSummary,
} from '../types'

/** 导入 MIDI 文件（命令 import_midi） */
export function importMidi(path: string): Promise<ImportSummary> {
  return invoke<ImportSummary>('import_midi', { path })
}

/** 轨道列表（命令 get_tracks） */
export function getTracks(docId: string): Promise<TrackSummary[]> {
  return invoke<TrackSummary[]>('get_tracks', { docId })
}

/** 执行编排并缓存序列（命令 compile） */
export function compile(
  docId: string,
  enabledTracks: number[],
  options: ArrangementOptions,
): Promise<CompileSummary> {
  return invoke<CompileSummary>('compile', { docId, enabledTracks, options })
}

/** 开始播放（命令 start_playback；3 秒倒计时后执行） */
export function startPlayback(
  seqId: string,
  backend: PlaybackBackend,
): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>('start_playback', { seqId, backend })
}

/** 暂停（命令 pause_playback） */
export function pausePlayback(): Promise<void> {
  return invoke<void>('pause_playback')
}

/** 继续（命令 resume_playback） */
export function resumePlayback(): Promise<void> {
  return invoke<void>('resume_playback')
}

/** 停止（命令 stop_playback） */
export function stopPlayback(): Promise<void> {
  return invoke<void>('stop_playback')
}

/** 紧急停止，立即释放全部按键（命令 emergency_stop） */
export function emergencyStop(): Promise<void> {
  return invoke<void>('emergency_stop')
}

/** 可用 Profile 列表（命令 list_profiles） */
export function listProfiles(): Promise<string[]> {
  return invoke<string[]>('list_profiles')
}

/** 加载 Profile（命令 load_profile） */
export function loadProfile(id: string): Promise<GameProfile> {
  return invoke<GameProfile>('load_profile', { id })
}

/** 当前 Profile（命令 current_profile；未选择时为 null） */
export function currentProfile(): Promise<GameProfile | null> {
  return invoke<GameProfile | null>('current_profile')
}

/** 上次会话是否异常退出（命令 get_crash_flag；启动自检） */
export function getCrashFlag(): Promise<boolean> {
  return invoke<boolean>('get_crash_flag')
}

/**
 * 选择 MIDI 文件（@tauri-apps/plugin-dialog）。
 * 用户取消时返回 null。
 */
export async function pickMidiFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'MIDI', extensions: ['mid', 'midi'] }],
  })
  return typeof selected === 'string' ? selected : null
}
