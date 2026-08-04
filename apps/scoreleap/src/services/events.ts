/**
 * Tauri 事件监听封装。
 *
 * listen 实现可注入（setEventSubscribeImpl），以便 Vitest 在无 Tauri
 * 环境下测试 playbackStore 的状态映射逻辑。
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  PlaybackProgress,
  PlaybackState,
  TranscriptionCompletedPayload,
  TranscriptionErrorPayload,
  TranscriptionStagePayload,
} from '../types'

export type { UnlistenFn } from '@tauri-apps/api/event'

/** 事件订阅实现签名 */
export type EventSubscribe = (
  event: string,
  handler: (payload: unknown) => void,
) => Promise<UnlistenFn>

let subscribeImpl: EventSubscribe = (event, handler) =>
  listen(event, (e) => handler(e.payload))

/** 注入自定义事件订阅实现（测试用） */
export function setEventSubscribeImpl(impl: EventSubscribe): void {
  subscribeImpl = impl
}

/** 订阅播放状态事件（playback://state） */
export function subscribePlaybackState(
  handler: (payload: PlaybackState) => void,
): Promise<UnlistenFn> {
  return subscribeImpl(
    'playback://state',
    handler as (payload: unknown) => void,
  )
}

/** 订阅播放进度事件（playback://progress） */
export function subscribePlaybackProgress(
  handler: (payload: PlaybackProgress) => void,
): Promise<UnlistenFn> {
  return subscribeImpl(
    'playback://progress',
    handler as (payload: unknown) => void,
  )
}

/** 订阅播放错误事件（playback://error，payload 为错误消息字符串） */
export function subscribePlaybackError(
  handler: (payload: string) => void,
): Promise<UnlistenFn> {
  return subscribeImpl(
    'playback://error',
    handler as (payload: unknown) => void,
  )
}

/** 订阅转录状态事件（transcription://state） */
export function subscribeTranscriptionState(
  handler: (payload: { job_id: string; status: string }) => void,
): Promise<UnlistenFn> {
  return subscribeImpl('transcription://state', (payload) =>
    handler(payload as { job_id: string; status: string }),
  )
}

/** 订阅转录阶段事件（transcription://stage） */
export function subscribeTranscriptionStage(
  handler: (payload: TranscriptionStagePayload) => void,
): Promise<UnlistenFn> {
  return subscribeImpl('transcription://stage', (payload) =>
    handler(payload as TranscriptionStagePayload),
  )
}

/** 订阅转录完成事件（transcription://completed） */
export function subscribeTranscriptionCompleted(
  handler: (payload: TranscriptionCompletedPayload) => void,
): Promise<UnlistenFn> {
  return subscribeImpl('transcription://completed', (payload) =>
    handler(payload as TranscriptionCompletedPayload),
  )
}

/** 订阅转录错误事件（transcription://error） */
export function subscribeTranscriptionError(
  handler: (payload: TranscriptionErrorPayload) => void,
): Promise<UnlistenFn> {
  return subscribeImpl('transcription://error', (payload) =>
    handler(payload as TranscriptionErrorPayload),
  )
}
