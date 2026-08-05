import { defineStore } from 'pinia'
import { errorText } from '../utils/format'
import { ref } from 'vue'
import type { DocumentSummary, ImportSummary, TrackSummary } from '../types'
import * as api from '../services/api'

export const useLibraryStore = defineStore('library', () => {
  /** 曲谱库摘要列表（数据源：后端持久化曲谱库 list_documents） */
  const documents = ref<DocumentSummary[]>([])
  /** 当前选中的曲谱 */
  const currentDocId = ref<string | null>(null)
  /** 当前曲谱的轨道列表 */
  const tracks = ref<TrackSummary[]>([])
  /** docId → 启用的轨道 id 列表 */
  const enabledTracks = ref<Record<string, number[]>>({})
  /** 错误信息（红色提示条） */
  const error = ref<string | null>(null)
  /** 是否正在从后端加载曲谱库 */
  const loading = ref(false)

  /** 从后端拉取曲谱库列表 */
  async function loadDocuments(): Promise<void> {
    loading.value = true
    try {
      documents.value = await api.listDocuments()
    } catch (e) {
      error.value = errorText(e)
    } finally {
      loading.value = false
    }
  }

  /** 导入 MIDI 文件并刷新曲谱库列表 */
  async function importFile(path: string): Promise<ImportSummary> {
    error.value = null
    try {
      const summary = await api.importMidi(path)
      await loadDocuments()
      return summary
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /**
   * 从列表移除曲谱（仅当前会话显示层移除；后端删除接口将在后续版本提供，
   * 重启后曲谱库仍会列出该曲谱）。
   */
  function removeDocument(docId: string): void {
    documents.value = documents.value.filter((d) => d.doc_id !== docId)
    delete enabledTracks.value[docId]
    if (currentDocId.value === docId) currentDocId.value = null
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
      error.value = errorText(e)
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
    loading,
    loadDocuments,
    importFile,
    removeDocument,
    selectDocument,
    toggleTrack,
    setAllTracks,
    enabledTrackIds,
  }
})
