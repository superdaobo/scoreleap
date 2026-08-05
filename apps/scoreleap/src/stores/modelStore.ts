import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { ModelStatusView } from '../types'
import * as api from '../services/api'
<<<<<<< HEAD
import { errorText } from '../utils/format'
=======
>>>>>>> feat/onnx-product-integration
import {
  subscribeModelProgress,
  subscribeModelState,
  type UnlistenFn,
} from '../services/events'

const EMPTY_STATUS: ModelStatusView = {
  status: 'unknown',
  configured: false,
  model_id: 'basic-pitch',
  installed_version: null,
  latest_version: null,
  size_bytes: null,
  source: null,
  received_bytes: 0,
  total_bytes: null,
  error: null,
  can_rollback: false,
}

export const useModelStore = defineStore('transcription-model', () => {
  const model = ref<ModelStatusView>({ ...EMPTY_STATUS })
  const busy = ref(false)
  const unlisteners = ref<UnlistenFn[]>([])
<<<<<<< HEAD
=======
  let subscribePromise: Promise<void> | null = null
>>>>>>> feat/onnx-product-integration

  const ready = computed(
    () => model.value.status === 'ready' || model.value.status === 'update_available',
  )
  const progressPercent = computed(() => {
    const total = model.value.total_bytes
    if (!total) return 0
    return Math.min(100, Math.round((model.value.received_bytes / total) * 100))
  })

  async function load(): Promise<void> {
    try {
      model.value = await api.getTranscriptionModelStatus()
    } catch (error) {
<<<<<<< HEAD
      model.value = { ...EMPTY_STATUS, status: 'failed', error: errorText(error) }
=======
      model.value = { ...EMPTY_STATUS, status: 'failed', error: String(error) }
>>>>>>> feat/onnx-product-integration
    }
  }

  async function checkUpdate(): Promise<void> {
    busy.value = true
    try {
      model.value = await api.checkTranscriptionModelUpdate()
    } catch (error) {
<<<<<<< HEAD
      model.value.error = errorText(error)
=======
      model.value.error = String(error)
>>>>>>> feat/onnx-product-integration
    } finally {
      busy.value = false
    }
  }

  async function download(): Promise<void> {
    busy.value = true
    try {
      await api.downloadTranscriptionModel()
      model.value.status = 'downloading'
      model.value.error = null
    } catch (error) {
      model.value.status = 'failed'
<<<<<<< HEAD
      model.value.error = errorText(error)
=======
      model.value.error = String(error)
>>>>>>> feat/onnx-product-integration
    } finally {
      busy.value = false
    }
  }

  async function cancel(): Promise<void> {
    try {
      await api.cancelTranscriptionModelDownload()
    } catch (error) {
<<<<<<< HEAD
      model.value.error = errorText(error)
=======
      model.value.error = String(error)
>>>>>>> feat/onnx-product-integration
    }
  }

  async function rollback(): Promise<void> {
    busy.value = true
    try {
      model.value = await api.rollbackTranscriptionModel()
    } catch (error) {
<<<<<<< HEAD
      model.value.error = errorText(error)
=======
      model.value.error = String(error)
>>>>>>> feat/onnx-product-integration
    } finally {
      busy.value = false
    }
  }

  async function subscribe(): Promise<void> {
    if (unlisteners.value.length > 0) return
<<<<<<< HEAD
    unlisteners.value.push(
      await subscribeModelProgress((progress) => {
        model.value.status = 'downloading'
        model.value.received_bytes = progress.received_bytes
        model.value.total_bytes = progress.total_bytes ?? model.value.total_bytes
        model.value.source = progress.source
      }),
      await subscribeModelState((status) => {
        model.value = status
      }),
    )
=======
    if (subscribePromise) return subscribePromise
    subscribePromise = (async () => {
      const acquired: UnlistenFn[] = []
      try {
        acquired.push(
          await subscribeModelProgress((progress) => {
            model.value.status = 'downloading'
            model.value.received_bytes = progress.received_bytes
            model.value.total_bytes = progress.total_bytes ?? model.value.total_bytes
            model.value.source = progress.source
          }),
        )
        acquired.push(
          await subscribeModelState((status) => {
            model.value = status
          }),
        )
        unlisteners.value.push(...acquired)
      } catch (error) {
        for (const unlisten of acquired) unlisten()
        throw error
      }
    })()
    try {
      await subscribePromise
    } finally {
      subscribePromise = null
    }
>>>>>>> feat/onnx-product-integration
  }

  function unsubscribe(): void {
    for (const unlisten of unlisteners.value) unlisten()
    unlisteners.value = []
  }

  return {
    model,
    busy,
    ready,
    progressPercent,
    load,
    checkUpdate,
    download,
    cancel,
    rollback,
    subscribe,
    unsubscribe,
  }
})
