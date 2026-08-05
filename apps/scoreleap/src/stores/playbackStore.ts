import { defineStore } from 'pinia'
import { errorText } from '../utils/format'
import { ref } from 'vue'
import type { PlaybackBackend, PlaybackProgress, PlaybackState } from '../types'
import * as api from '../services/api'
import {
  subscribePlaybackError,
  subscribePlaybackProgress,
  subscribePlaybackState,
  type UnlistenFn,
} from '../services/events'

const COUNTDOWN_TOTAL_SECONDS = 3

export const usePlaybackStore = defineStore('playback', () => {
  /** 播放状态机 */
  const state = ref<PlaybackState>('Idle')
  /** 播放位置（微秒） */
  const positionUs = ref(0)
  /** 当前按住的按键数 */
  const pressedKeys = ref(0)
  /** 当前演奏中的音符（无则为 null） */
  const currentNote = ref<PlaybackProgress['current_note']>(null)
  /** 倒计时剩余秒数（Countdown 状态显示 3-2-1） */
  const countdownSeconds = ref(0)
  /** 错误信息 */
  const error = ref<string | null>(null)
  /** 最近一次启动的序列 id */
  const seqId = ref<string | null>(null)
  /** 最近一次使用的播放后端 */
  const backend = ref<PlaybackBackend>('sendinput')

  let countdownTimer: ReturnType<typeof setInterval> | null = null
  let unlisteners: UnlistenFn[] = []

  function clearCountdownTimer(): void {
    if (countdownTimer !== null) {
      clearInterval(countdownTimer)
      countdownTimer = null
    }
    countdownSeconds.value = 0
  }

  function startCountdown(): void {
    clearCountdownTimer()
    countdownSeconds.value = COUNTDOWN_TOTAL_SECONDS
    countdownTimer = setInterval(() => {
      countdownSeconds.value -= 1
      if (countdownSeconds.value <= 0) clearCountdownTimer()
    }, 1000)
  }

  /** 状态事件 → store 状态映射 */
  function applyState(next: PlaybackState): void {
    state.value = next
    if (next === 'Countdown') {
      startCountdown()
    } else if (next === 'Playing') {
      clearCountdownTimer()
    } else if (next === 'Stopped' || next === 'Finished' || next === 'Idle') {
      clearCountdownTimer()
      pressedKeys.value = 0
      currentNote.value = null
    }
  }

  /** 进度事件 → store 状态映射 */
  function applyProgress(progress: PlaybackProgress): void {
    positionUs.value = progress.position_us
    pressedKeys.value = progress.pressed_keys
    currentNote.value = progress.current_note
  }

  /** 订阅 Tauri 播放事件（main.ts 调用一次） */
  async function setupEventListeners(): Promise<void> {
    await teardownEventListeners()
    const [un1, un2, un3] = await Promise.all([
      subscribePlaybackState(applyState),
      subscribePlaybackProgress(applyProgress),
      subscribePlaybackError((message) => {
        error.value = message
      }),
    ])
    unlisteners = [un1, un2, un3]
  }

  /** 取消全部事件订阅 */
  async function teardownEventListeners(): Promise<void> {
    for (const un of unlisteners) {
      try {
        un()
      } catch {
        // 忽略单个退订失败
      }
    }
    unlisteners = []
  }

  /** 开始播放（3 秒倒计时后执行） */
  async function start(seq: string, backendName: PlaybackBackend): Promise<void> {
    error.value = null
    seqId.value = seq
    backend.value = backendName
    try {
      const status = await api.startPlayback(seq, backendName)
      positionUs.value = status.position_ms * 1000
      pressedKeys.value = status.pressed_keys
      applyState(status.state)
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  async function pause(): Promise<void> {
    try {
      await api.pausePlayback()
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 跳转到指定位置（微秒）；后端 Playing/Paused 状态均支持 */
  async function seek(positionUs: number): Promise<void> {
    try {
      await api.seekPlayback(positionUs)
    } catch (e) {
      error.value = errorText(e)
    }
  }

  async function resume(): Promise<void> {
    try {
      await api.resumePlayback()
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  async function stop(): Promise<void> {
    try {
      await api.stopPlayback()
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  async function emergency(): Promise<void> {
    try {
      await api.emergencyStop()
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  return {
    state,
    positionUs,
    pressedKeys,
    currentNote,
    countdownSeconds,
    error,
    seqId,
    backend,
    setupEventListeners,
    teardownEventListeners,
    start,
    pause,
    resume,
    stop,
    emergency,
    seek,
  }
})
