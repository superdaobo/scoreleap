import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { usePlaybackStore } from '../playbackStore'
import { setEventSubscribeImpl } from '../../services/events'
import * as api from '../../services/api'

// 拦截 Tauri 事件与命令层，测试不依赖真实 Tauri 环境
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock('../../services/api', () => ({
  startPlayback: vi.fn(),
  pausePlayback: vi.fn(),
  resumePlayback: vi.fn(),
  stopPlayback: vi.fn(),
  emergencyStop: vi.fn(),
}))

interface CapturedEvent {
  event: string
  handler: (payload: unknown) => void
}

let captured: CapturedEvent[] = []
let unlistenCalls = 0

function capturedHandler(eventName: string): (payload: unknown) => void {
  const entry = captured.find((c) => c.event === eventName)
  if (!entry) throw new Error(`未订阅事件: ${eventName}`)
  return entry.handler
}

describe('playbackStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.useFakeTimers()
    captured = []
    unlistenCalls = 0
    setEventSubscribeImpl((event, handler) => {
      captured.push({ event, handler })
      return Promise.resolve(() => {
        unlistenCalls += 1
      })
    })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('订阅三个播放事件', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    expect(captured.map((c) => c.event)).toEqual([
      'playback://state',
      'playback://progress',
      'playback://error',
    ])
  })

  it('状态事件正确映射到 state（含 Stopped 清零按键）', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    const stateCb = capturedHandler('playback://state')

    stateCb('Playing')
    expect(store.state).toBe('Playing')

    stateCb('Paused')
    expect(store.state).toBe('Paused')

    store.pressedKeys = 5
    stateCb('Stopped')
    expect(store.state).toBe('Stopped')
    expect(store.pressedKeys).toBe(0)

    stateCb('Finished')
    expect(store.state).toBe('Finished')

    stateCb('Idle')
    expect(store.state).toBe('Idle')
  })

  it('进度事件更新位置、按键与当前音符', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    const progressCb = capturedHandler('playback://progress')

    progressCb({
      position_us: 1_500_000,
      current_note: null,
      pressed_keys: 3,
    })
    expect(store.positionUs).toBe(1_500_000)
    expect(store.pressedKeys).toBe(3)
    expect(store.currentNote).toBeNull()

    progressCb({
      position_us: 2_000_000,
      current_note: {
        track_id: 0,
        note: 60,
        velocity: 100,
        start_us: 1_800_000,
        duration_us: 400_000,
      },
      pressed_keys: 2,
    })
    expect(store.currentNote?.note).toBe(60)
  })

  it('Countdown 状态启动 3-2-1 倒计时，Playing 时清除', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    const stateCb = capturedHandler('playback://state')

    stateCb('Countdown')
    expect(store.countdownSeconds).toBe(3)
    vi.advanceTimersByTime(1000)
    expect(store.countdownSeconds).toBe(2)
    vi.advanceTimersByTime(1000)
    expect(store.countdownSeconds).toBe(1)
    vi.advanceTimersByTime(1000)
    expect(store.countdownSeconds).toBe(0)

    stateCb('Countdown')
    expect(store.countdownSeconds).toBe(3)
    stateCb('Playing')
    expect(store.countdownSeconds).toBe(0)
  })

  it('错误事件写入 error', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    const errorCb = capturedHandler('playback://error')
    errorCb('模拟错误：注入失败')
    expect(store.error).toBe('模拟错误：注入失败')
  })

  it('start 调用后端并映射返回状态', async () => {
    vi.mocked(api.startPlayback).mockResolvedValue({
      state: 'Countdown',
      position_ms: 0,
      pressed_keys: 0,
    })
    const store = usePlaybackStore()
    await store.setupEventListeners()
    await store.start('seq-1', 'mock')
    expect(api.startPlayback).toHaveBeenCalledWith('seq-1', 'mock')
    expect(store.seqId).toBe('seq-1')
    expect(store.backend).toBe('mock')
    expect(store.state).toBe('Countdown')
    expect(store.countdownSeconds).toBe(3)
  })

  it('teardownEventListeners 退订全部事件', async () => {
    const store = usePlaybackStore()
    await store.setupEventListeners()
    await store.teardownEventListeners()
    expect(unlistenCalls).toBe(3)
  })
})
