import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ImportSummary, TrackSummary } from '../types'
import * as api from '../services/api'

const STORAGE_KEY = 'scoreleap-documents'

/** 已导入曲谱（含本地元数据） */
export interface StoredDocument extends ImportSummary {
  imported_at: number
}

function loadStoredDocuments(): StoredDocument[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed) ? (parsed as StoredDocument[]) : []
  } catch {
    return []
  }
}

export const useLibraryStore = defineStore('library', () => {
  /** 最近导入的曲谱摘要列表（localStorage 持久化） */
  const documents = ref<StoredDocument[]>(loadStoredDocuments())
  /** 当前选中的曲谱 */
  const currentDocId = ref<string | null>(null)
  /** 当前曲谱的轨道列表 */
  const tracks = ref<TrackSummary[]>([])
  /** docId → 启用的轨道 id 列表 */
  const enabledTracks = ref<Record<string, number[]>>({})
  /** 错误信息（红色提示条） */
  const error = ref<string | null>(null)

  function persistDocuments(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(documents.value))
    } catch {
      // 存储不可用（如隐私模式）时静默失败，不影响使用
    }
  }

  /** 导入 MIDI 文件并加入列表 */
  async function importFile(path: string): Promise<ImportSummary> {
    error.value = null
    try {
      const summary = await api.importMidi(path)
      documents.value.unshift({ ...summary, imported_at: Date.now() })
      persistDocuments()
      return summary
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  /** 从列表移除曲谱 */
  function removeDocument(docId: string): void {
    documents.value = documents.value.filter((d) => d.doc_id !== docId)
    delete enabledTracks.value[docId]
    if (currentDocId.value === docId) currentDocId.value = null
    persistDocuments()
  }

  /** 选择曲谱并加载轨道（默认全部启用） */
  async function selectDocument(docId: string): Promise<void> {
    currentDocId.value = docId
    error.value = null
    try {
      tracks.value = await api.getTracks(docId)
      enabledTracks.value[docId] = tracks.value
        .filter((t) => t.enabled)
        .map((t) => t.id)
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  /** 切换某条轨道的启用状态 */
  function toggleTrack(docId: string, trackId: number): void {
    const current =
      enabledTracks.value[docId] ??
      tracks.value.filter((t) => t.enabled).map((t) => t.id)
    const next = new Set(current)
    if (next.has(trackId)) next.delete(trackId)
    else next.add(trackId)
    enabledTracks.value[docId] = [...next].sort((a, b) => a - b)
  }

  /** 一键启用/禁用全部轨道 */
  function setAllTracks(docId: string, enabled: boolean): void {
    enabledTracks.value[docId] = enabled ? tracks.value.map((t) => t.id) : []
  }

  /** 某曲谱的启用轨道；未初始化时默认全部启用 */
  function enabledTrackIds(docId: string): number[] {
    const ids = enabledTracks.value[docId]
    if (ids !== undefined) return ids
    return tracks.value.filter((t) => t.enabled).map((t) => t.id)
  }

  return {
    documents,
    currentDocId,
    tracks,
    enabledTracks,
    error,
    importFile,
    removeDocument,
    selectDocument,
    toggleTrack,
    setAllTracks,
    enabledTrackIds,
  }
})
