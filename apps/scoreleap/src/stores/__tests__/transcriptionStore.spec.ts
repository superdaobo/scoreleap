import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useTranscriptionStore, setEventSubscribeImpl } from '../transcriptionStore'
import * as api from '../../services/api'
import type { TranscriptionJobView } from '../../types'

// 拦截 Tauri 命令层
vi.mock('../../services/api', () => ({
  startAudioTranscription: vi.fn(),
  cancelAudioTranscription: vi.fn(),
  getAudioTranscriptionStatus: vi.fn(),
}))

// 事件订阅注入：记录 handler 供测试手动触发
type Handler = (payload: unknown) => void
const handlers = new Map<string, Handler[]>()
setEventSubscribeImpl(async (event, handler) => {
  if (!handlers.has(event)) handlers.set(event, [])
  handlers.get(event)!.push(handler as Handler)
  return () => {
    /* noop */
  }
})
function emit(event: string, payload: unknown): void {
  for (const h of handlers.get(event) ?? []) h(payload)
}

const SAMPLE_JOB: TranscriptionJobView = {
  job_id: 'job-1',
  request_id: 'req-1',
  source_name: '测试 音频.mp3',
  status: 'Starting',
  stage: 'starting',
  message: '正在启动转录组件',
  started_at_ms: 1000,
  elapsed_ms: 0,
  note_count: null,
  midi_path: 'C:/tx/job-1/generated.mid',
  metadata_path: 'C:/tx/job-1/metadata.json',
  result_doc_id: null,
  error_code: null,
  error_message: null,
}

describe('transcriptionStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    handlers.clear()
    vi.clearAllMocks()
  })

  it('start 发起命令并拉取任务视图', async () => {
    vi.mocked(api.startAudioTranscription).mockResolvedValue('job-1')
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    await store.start('C:/audio.mp3')
    expect(api.startAudioTranscription).toHaveBeenCalledWith('C:/audio.mp3', {
      preset: 'balanced',
      onset_threshold: null,
      frame_threshold: null,
      minimum_note_ms: null,
    })
    expect(store.job?.job_id).toBe('job-1')
    expect(store.running).toBe(true)
  })

  it('start 失败写入 error 并抛出', async () => {
    vi.mocked(api.startAudioTranscription).mockRejectedValue(new Error('TRANSCRIPTION_BUSY'))
    const store = useTranscriptionStore()
    await expect(store.start('C:/a.mp3')).rejects.toThrow()
    expect(store.error).toBeTruthy()
    expect(store.job).toBeNull()
  })

  it('stage 事件更新阶段与状态（不确定进度）', async () => {
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    store.job = { ...SAMPLE_JOB }
    await store.subscribe()
    emit('transcription://stage', {
      job_id: 'job-1',
      stage: 'loading_model',
      message: '正在加载本地模型',
    })
    expect(store.job?.status).toBe('LoadingModel')
    expect(store.job?.stage).toBe('loading_model')
    expect(store.indeterminate).toBe(true)
    expect(store.stageLabel).toContain('加载本地模型')
  })

  it('completed 事件标记完成并记录结果', async () => {
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    store.job = { ...SAMPLE_JOB }
    await store.subscribe()
    emit('transcription://completed', {
      job_id: 'job-1',
      doc_id: 'doc-tx-1',
      midi_path: 'C:/tx/job-1/generated.mid',
      note_count: 174,
      elapsed_ms: 10500,
    })
    expect(store.job?.status).toBe('Completed')
    expect(store.job?.result_doc_id).toBe('doc-tx-1')
    expect(store.job?.note_count).toBe(174)
    expect(store.running).toBe(false)
  })

  it('error 事件标记失败并记录错误码', async () => {
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    store.job = { ...SAMPLE_JOB }
    await store.subscribe()
    emit('transcription://error', {
      job_id: 'job-1',
      code: 'AUDIO_DECODE_FAILED',
      message: '无法解码音频',
    })
    expect(store.job?.status).toBe('Failed')
    expect(store.job?.error_code).toBe('AUDIO_DECODE_FAILED')
    expect(store.running).toBe(false)
  })

  it('cancel 调用命令并标记取消', async () => {
    vi.mocked(api.cancelAudioTranscription).mockResolvedValue(undefined)
    const store = useTranscriptionStore()
    store.job = { ...SAMPLE_JOB }
    await store.cancel()
    expect(api.cancelAudioTranscription).toHaveBeenCalled()
    expect(store.job?.status).toBe('Cancelled')
    expect(store.running).toBe(false)
  })

  it('restore 恢复最近任务状态（含终态）', async () => {
    const done: TranscriptionJobView = { ...SAMPLE_JOB, status: 'Completed' }
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(done)
    const store = useTranscriptionStore()
    await store.restore()
    expect(store.job?.status).toBe('Completed')
  })

  it('askConfirm/dismissConfirm 管理确认界面', async () => {
    const store = useTranscriptionStore()
    store.askConfirm('C:/a.mp3', 'a.mp3', 1048576)
    expect(store.pendingConfirm?.name).toBe('a.mp3')
    expect(store.pendingConfirm?.size_bytes).toBe(1048576)
    store.dismissConfirm()
    expect(store.pendingConfirm).toBeNull()
  })

  it('start 成功后清除确认状态', async () => {
    vi.mocked(api.startAudioTranscription).mockResolvedValue('job-1')
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    store.askConfirm('C:/a.mp3', 'a.mp3', 1)
    await store.start('C:/a.mp3')
    expect(store.pendingConfirm).toBeNull()
  })

  it('errorLabel 区分结构化错误码', async () => {
    const store = useTranscriptionStore()
    expect(store.errorLabel('AUDIO_FILE_TOO_LARGE', 'x')).toContain('200MB')
    expect(store.errorLabel('WORKER_NOT_FOUND', 'x')).toContain('原生转录组件')
    expect(store.errorLabel('MODEL_DOWNLOAD_REQUIRED', 'x')).toContain('下载模型')
    expect(store.errorLabel(null, '回退文案')).toBe('回退文案')
  })

<<<<<<< HEAD
=======
  it('忽略其他任务的迟到 completed/error 事件', async () => {
    const store = useTranscriptionStore()
    store.job = { ...SAMPLE_JOB }
    await store.subscribe()
    emit('transcription://completed', {
      job_id: 'job-old',
      doc_id: 'doc-old',
      midi_path: 'C:/old.mid',
      note_count: 999,
      elapsed_ms: 1,
    })
    emit('transcription://error', {
      job_id: 'job-old',
      code: 'INTERNAL_ERROR',
      message: '旧任务错误',
    })
    expect(store.job?.status).toBe('Starting')
    expect(store.job?.result_doc_id).toBeNull()
    expect(store.job?.error_code).toBeNull()
  })

>>>>>>> feat/onnx-product-integration
  it('高级阈值启用后随预设传给原生 sidecar 命令', async () => {
    vi.mocked(api.startAudioTranscription).mockResolvedValue('job-1')
    vi.mocked(api.getAudioTranscriptionStatus).mockResolvedValue(SAMPLE_JOB)
    const store = useTranscriptionStore()
    store.preset = 'noise_reduced'
    store.advancedEnabled = true
    store.onsetThreshold = 0.7
    store.frameThreshold = 0.55
    store.minimumNoteMs = 90
    await store.start('C:/noise.flac')
    expect(api.startAudioTranscription).toHaveBeenCalledWith('C:/noise.flac', {
      preset: 'noise_reduced',
      onset_threshold: 0.7,
      frame_threshold: 0.55,
      minimum_note_ms: 90,
    })
  })
<<<<<<< HEAD
=======

  it('拒绝超出原生运行时契约的高级参数', async () => {
    const store = useTranscriptionStore()
    store.advancedEnabled = true
    store.minimumNoteMs = 10
    await expect(store.start('C:/invalid.mp3')).rejects.toThrow('20 到 2000')
    expect(api.startAudioTranscription).not.toHaveBeenCalled()
    expect(store.error).toContain('20 到 2000')
  })
>>>>>>> feat/onnx-product-integration
})
