import { defineStore } from 'pinia'
import { errorText } from '../utils/format'
import { ref } from 'vue'
import type {
  DocumentSummary,
  GroupInfo,
  ImportSummary,
  TrackSummary,
} from '../types'
import * as api from '../services/api'

export const useLibraryStore = defineStore('library', () => {
  /** 曲谱库摘要列表（数据源：后端持久化曲谱库 list_documents；数组顺序 = 全局顺序） */
  const documents = ref<DocumentSummary[]>([])
  /** 分组列表（list_groups） */
  const groups = ref<GroupInfo[]>([])
  /** 当前选中的曲谱 */
  const currentDocId = ref<string | null>(null)
  /** 当前曲谱的轨道列表 */
  const tracks = ref<TrackSummary[]>([])
  /** docId → 启用的轨道 id 列表 */
  const enabledTracks = ref<Record<string, number[]>>({})
  /** 错误信息（红色提示条） */
  const error = ref<string | null>(null)
  /** 成功提示信息（绿色提示条；导入/去重/管理操作反馈） */
  const notice = ref<string | null>(null)
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

  /** 从后端拉取分组列表 */
  async function loadGroups(): Promise<void> {
    try {
      groups.value = await api.listGroups()
    } catch (e) {
      error.value = errorText(e)
    }
  }

  /** 加载曲谱库 + 分组（页面进入时调用） */
  async function loadAll(): Promise<void> {
    await Promise.all([loadDocuments(), loadGroups()])
  }

  /** 导入 MIDI 文件并刷新曲谱库列表（自动去重由后端判定） */
  async function importFile(path: string): Promise<ImportSummary> {
    error.value = null
    notice.value = null
    try {
      const summary = await api.importMidi(path)
      await loadDocuments()
      if (summary.duplicated) {
        notice.value = '检测到重复曲谱，已自动跳过导入'
      }
      return summary
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 删除曲谱（后端同步删除磁盘文件与索引） */
  async function deleteDocument(docId: string): Promise<void> {
    error.value = null
    try {
      await api.deleteDocument(docId)
      documents.value = documents.value.filter((d) => d.doc_id !== docId)
      delete enabledTracks.value[docId]
      if (currentDocId.value === docId) currentDocId.value = null
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 重命名曲谱 */
  async function renameDocument(docId: string, name: string): Promise<void> {
    error.value = null
    try {
      await api.renameDocument(docId, name)
      const doc = documents.value.find((d) => d.doc_id === docId)
      if (doc) doc.name = name
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 创建分组 */
  async function createGroup(name: string): Promise<GroupInfo> {
    error.value = null
    try {
      const group = await api.createGroup(name)
      groups.value.push(group)
      return group
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 重命名分组 */
  async function renameGroup(groupId: string, name: string): Promise<void> {
    error.value = null
    try {
      const group = await api.renameGroup(groupId, name)
      const target = groups.value.find((g) => g.group_id === groupId)
      if (target) target.name = group.name
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 删除分组；组内曲谱回到未分类 */
  async function deleteGroup(groupId: string): Promise<void> {
    error.value = null
    try {
      await api.deleteGroup(groupId)
      groups.value = groups.value.filter((g) => g.group_id !== groupId)
      for (const doc of documents.value) {
        if (doc.group_id === groupId) doc.group_id = null
      }
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /** 将曲谱移入分组；groupId 传 null 移回未分类 */
  async function moveToGroup(docId: string, groupId: string | null): Promise<void> {
    error.value = null
    try {
      await api.moveDocumentToGroup(docId, groupId)
      const doc = documents.value.find((d) => d.doc_id === docId)
      if (doc) doc.group_id = groupId
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
  }

  /**
   * 按视图内新顺序重排（拖拽/上移下移后提交）。
   * 视图 = 当前分组（或未分类）过滤出的子集；视图外曲谱保持相对位置不变，
   * 视图内曲谱按新序占据它们在全局中原来的槽位。
   */
  async function reorderInView(viewDocIds: string[]): Promise<void> {
    const all = documents.value.map((d) => d.doc_id)
    const viewSet = new Set(viewDocIds)
    // 视图元素在全局中占用的槽位（按全局顺序）
    const slots = all
      .map((id, i) => (viewSet.has(id) ? i : -1))
      .filter((i) => i >= 0)
    if (slots.length === 0 || slots.length !== viewDocIds.length) return
    const next = [...all]
    slots.forEach((pos, idx) => {
      next[pos] = viewDocIds[idx]
    })
    // 乐观更新本地顺序
    const byId = new Map(documents.value.map((d) => [d.doc_id, d]))
    documents.value = next
      .map((id) => byId.get(id))
      .filter((d): d is DocumentSummary => d !== undefined)
    try {
      await api.reorderDocuments(next)
    } catch (e) {
      error.value = errorText(e)
      // 回滚：重新拉取后端顺序
      await loadDocuments()
    }
  }

  /** 更新曲目信息（标题/艺术家） */
  async function updatePieceInfo(
    docId: string,
    title: string | null,
    artist: string | null,
  ): Promise<void> {
    error.value = null
    try {
      await api.updatePieceInfo(docId, title, artist)
      const doc = documents.value.find((d) => d.doc_id === docId)
      if (doc) {
        doc.title = title
        doc.artist = artist
      }
    } catch (e) {
      error.value = errorText(e)
      throw e
    }
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

  function clearError(): void {
    error.value = null
  }

  function clearNotice(): void {
    notice.value = null
  }

  return {
    documents, groups, currentDocId, tracks, enabledTracks, error, notice, loading,
    loadDocuments, loadGroups, loadAll, importFile, deleteDocument, renameDocument,
    createGroup, renameGroup, deleteGroup, moveToGroup, reorderInView, updatePieceInfo,
    selectDocument, toggleTrack, setAllTracks, enabledTrackIds, clearError, clearNotice,
  }
})
