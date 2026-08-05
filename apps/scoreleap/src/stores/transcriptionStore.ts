import { defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import type { TranscriptionJobView, TranscriptionPreset } from '../types'
import * as api from '../services/api'
import { errorText } from '../utils/format'
import {
  setEventSubscribeImpl,
  subscribeTranscriptionCompleted,
  subscribeTranscriptionError,
  subscribeTranscriptionStage,
  subscribeTranscriptionState,
  type UnlistenFn,
} from '../services/events'

export { setEventSubscribeImpl }

/** 阶段 → 用户可读文案（进度卡片展示） */
export const STAGE_LABELS: Record<string, string> = {
  starting: '正在启动转录组件',
  validating_input: '正在验证音频',
  loading_model: '正在加载本地模型（首次较慢）',
  transcribing: '正在识别音符',
  writing_midi: '正在生成 MIDI',
  importing_midi: '正在导入曲谱库',
}

/** 阶段是否处于不确定进度区间（模型加载/推理不可分段） */
export const STAGE_INDETERMINATE: Record<string, boolean> = {
  loading_model: true,
  transcribing: true,
}

/** 错误码 → 用户可读文案（#47 验收：错误区分） */
export const ERROR_LABELS: Record<string, string> = {
  WORKER_NOT_FOUND: '未找到原生转录组件，请重新安装完整版本',
  WORKER_START_FAILED: '转录组件启动失败',
  TRANSCRIPTION_BUSY: '已有转录任务在进行中，请稍后再试',
  INVALID_AUDIO_PATH: '音频文件不存在或无法读取',
  UNSUPPORTED_AUDIO_FORMAT: '仅支持 MP3、WAV 或 FLAC 格式',
  AUDIO_FILE_TOO_LARGE: '音频文件超过 200MB 上限',
  AUDIO_TOO_LONG: '音频超过 10 分钟上限',
  AUDIO_DECODE_FAILED: '音频解码失败（文件可能已损坏）',
  MODEL_MISSING: '转录模型文件缺失',
  MODEL_DOWNLOAD_REQUIRED: '首次转录前需要在设置中下载模型',
  MODEL_LOAD_FAILED: '本地模型加载失败',
  RUNTIME_MISSING: 'ONNX Runtime 缺失，请重新安装完整版本',
  RUNTIME_LOAD_FAILED: 'ONNX Runtime 加载失败',
  INFERENCE_FAILED: '音符识别失败',
  MIDI_WRITE_FAILED: 'MIDI 生成失败',
  MIDI_VALIDATION_FAILED: '生成的 MIDI 无法通过校验',
  JOB_CANCELLED: '任务已取消',
  WORKER_PROTOCOL_ERROR: '转录组件通信异常',
  WORKER_EXITED_UNEXPECTEDLY: '转录组件异常退出',
  INTERNAL_ERROR: '内部错误，请查看日志',
}

/** 待确认的转录任务（确认界面数据） */
export interface PendingConfirm {
  path: string
  name: string
  size_bytes: number
}

export const useTranscriptionStore = defineStore('transcription', () => {
  const preset = ref<TranscriptionPreset>(readPreset())
  const advancedEnabled = ref(false)
  const onsetThreshold = ref(0.5)
  const frameThreshold = ref(0.3)
  const minimumNoteMs = ref(58)
  watch(preset, (value) => {
    try {
      localStorage.setItem('scoreleap-transcription-preset', value)
    } catch {
      // WebView 存储不可用时保持本次会话设置。
    }
  })
  /** 当前/最近一次转录任务（终态保留展示） */
  const job = ref<TranscriptionJobView | null>(null)
  /** 错误信息（红色提示条） */
  const error = ref<string | null>(null)
  /** 是否正在发起转录（避免重复点击） */
  const starting = ref(false)
  /** 待确认的转录任务（确认界面显示；null = 未在选择状态） */
  const pendingConfirm = ref<PendingConfirm | null>(null)
  /** 事件订阅句柄（测试注入用） */
  const unlisteners = ref<UnlistenFn[]>([])

  const running = computed(
    () =>
      job.value !== null &&
      !['Completed', 'Failed', 'Cancelled'].includes(job.value.status),
  )

  const stageLabel = computed(() => {
    if (!job.value) return ''
    return STAGE_LABELS[job.value.stage] ?? job.value.message
  })

  const indeterminate = computed(
    () => job.value !== null && (STAGE_INDETERMINATE[job.value.stage] ?? false),
  )

  /** 订阅转录事件（App 挂载时调用一次；测试可注入 setEventSubscribeImpl） */
  async function subscribe(): Promise<void> {
    if (unlisteners.value.length > 0) return
    unlisteners.value.push(
      await subscribeTranscriptionState((p) => {
        if (!job.value || job.value.job_id === p.job_id) {
          if (job.value) job.value.status = p.status as TranscriptionJobView['status']
        }
      }),
      await subscribeTranscriptionStage((p) => {
        if (!job.value || job.value.job_id === p.job_id) {
          if (job.value) {
            job.value.stage = p.stage
            job.value.message = p.message
            job.value.status = stageFromName(p.stage)
          }
        }
      }),
      await subscribeTranscriptionCompleted((p) => {
        if (job.value) {
          job.value.status = 'Completed'
          job.value.result_doc_id = p.doc_id
          job.value.note_count = p.note_count
          job.value.elapsed_ms = p.elapsed_ms
          job.value.message = '转录完成，已导入曲谱库'
        }
      }),
      await subscribeTranscriptionError((p) => {
        if (job.value) {
          job.value.status = 'Failed'
          job.value.error_code = p.code
          job.value.error_message = p.message
          job.value.message = p.message
        }
      }),
    )
  }

  /** 取消事件订阅（测试用/App 卸载） */
  function unsubscribe(): void {
    for (const u of unlisteners.value) u()
    unlisteners.value = []
  }

  /** 启动转录：发起命令并拉取任务视图（事件流随后增量更新） */
  async function start(path: string): Promise<void> {
    error.value = null
    starting.value = true
    try {
      const jobId = await api.startAudioTranscription(path, {
        preset: preset.value,
        onset_threshold: advancedEnabled.value ? onsetThreshold.value : null,
        frame_threshold: advancedEnabled.value ? frameThreshold.value : null,
        minimum_note_ms: advancedEnabled.value ? minimumNoteMs.value : null,
      })
      const view = await api.getAudioTranscriptionStatus()
      if (view && view.job_id === jobId) {
        job.value = view
      }
      pendingConfirm.value = null
    } catch (e) {
      error.value = errorText(e)
      throw e
    } finally {
      starting.value = false
    }
  }

  /** 设置待确认任务（选择受支持音频后调用） */
  function askConfirm(path: string, name: string, sizeBytes: number): void {
    pendingConfirm.value = { path, name, size_bytes: sizeBytes }
  }

  /** 关闭确认界面 */
  function dismissConfirm(): void {
    pendingConfirm.value = null
  }

  /** 错误文案（区分结构化错误码；未知码回退原文） */
  function errorLabel(code: string | null, fallback: string): string {
    if (!code) return fallback
    return ERROR_LABELS[code] ?? fallback
  }

  /** 取消当前转录任务 */
  async function cancel(): Promise<void> {
    error.value = null
    try {
      await api.cancelAudioTranscription()
      if (job.value) {
        job.value.status = 'Cancelled'
        job.value.message = '任务已取消'
      }
    } catch (e) {
      error.value = errorText(e)
    }
  }

  /** 恢复最近任务状态（页面进入时调用） */
  async function restore(): Promise<void> {
    try {
      job.value = await api.getAudioTranscriptionStatus()
    } catch (e) {
      error.value = errorText(e)
    }
  }

  function clearError(): void {
    error.value = null
  }

  return {
    job,
    error,
    starting,
    running,
    stageLabel,
    indeterminate,
    subscribe,
    unsubscribe,
    start,
    cancel,
    restore,
    clearError,
    pendingConfirm,
    askConfirm,
    dismissConfirm,
    errorLabel,
    preset,
    advancedEnabled,
    onsetThreshold,
    frameThreshold,
    minimumNoteMs,
  }
})

function readPreset(): TranscriptionPreset {
  try {
    const value = localStorage.getItem('scoreleap-transcription-preset')
    if (value === 'balanced' || value === 'detail' || value === 'noise_reduced') return value
  } catch {
    // 使用默认值。
  }
  return 'balanced'
}

/** stage 名 → 任务状态（与后端 JobStatus 对齐） */
function stageFromName(stage: string): TranscriptionJobView['status'] {
  switch (stage) {
    case 'validating_input':
      return 'ValidatingInput'
    case 'loading_model':
      return 'LoadingModel'
    case 'transcribing':
      return 'Transcribing'
    case 'writing_midi':
      return 'WritingMidi'
    case 'importing_midi':
      return 'ImportingMidi'
    default:
      return 'Starting'
  }
}
