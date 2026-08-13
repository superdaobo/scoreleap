import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useLibraryStore } from '../libraryStore'
import * as api from '../../services/api'

// 拦截 Tauri 命令层，避免在 Node 环境加载 @tauri-apps
vi.mock('../../services/api', () => ({
  importMidi: vi.fn(),
  listDocuments: vi.fn(),
  listGroups: vi.fn(),
  getTracks: vi.fn(),
  createGroup: vi.fn(),
  renameGroup: vi.fn(),
  deleteGroup: vi.fn(),
  moveDocumentToGroup: vi.fn(),
  reorderDocuments: vi.fn(),
  deleteDocument: vi.fn(),
  renameDocument: vi.fn(),
  updatePieceInfo: vi.fn(),
}))

const SAMPLE_DOCS: import('../../types').DocumentSummary[] = [
  {
    doc_id: 'doc-1',
    name: '晴天.mid',
    format: 'Parallel',
    track_count: 3,
    note_count: 1109,
    duration_ms: 268235,
    bpm_range: [68, 120],
    source_type: 'midi',
    imported_at: 1700000000000,
    group_id: null,
    content_hash: 'hash-1',
    transcription: null,
    title: null,
    artist: null,
  },
  {
    doc_id: 'doc-2',
    name: 'demo.mid',
    format: 'SingleTrack',
    track_count: 1,
    note_count: 8,
    duration_ms: 4000,
    bpm_range: [120, 120],
    source_type: 'audio_transcription',
    imported_at: 1700000001000,
    group_id: null,
    content_hash: 'hash-2',
    transcription: {
      engine: 'scoreleap-transkun-v2',
      engine_version: '1.0.0',
      model_file: '2.0.pt',
      elapsed_ms: 12345,
      completed_at_ms: 1700000001000,
    },
    title: '致爱丽丝',
    artist: '贝多芬',
  },
]

const SAMPLE_TRACKS = [
  { id: 1, name: '旋律', note_count: 100, enabled: true },
  { id: 2, name: '伴奏', note_count: 50, enabled: true },
  { id: 3, name: '低音', note_count: 30, enabled: true },
]

describe('libraryStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    vi.mocked(api.listDocuments).mockResolvedValue(SAMPLE_DOCS)
    vi.mocked(api.listGroups).mockResolvedValue([])
    vi.mocked(api.getTracks).mockResolvedValue(SAMPLE_TRACKS)
  })

  it('loadDocuments 从后端拉取曲谱库', async () => {
    const store = useLibraryStore()
    expect(store.documents).toHaveLength(0)
    await store.loadDocuments()
    expect(store.documents).toHaveLength(2)
    expect(store.documents[0].doc_id).toBe('doc-1')
  })

  it('loadDocuments 失败时写入 error', async () => {
    vi.mocked(api.listDocuments).mockRejectedValue('后端不可用')
    const store = useLibraryStore()
    await store.loadDocuments()
    expect(store.error).toBe('后端不可用')
    expect(store.documents).toHaveLength(0)
  })

  it('selectDocument 默认启用全部轨道', async () => {
    const store = useLibraryStore()
    await store.selectDocument('doc-1')
    expect(store.currentDocId).toBe('doc-1')
    expect(store.tracks).toHaveLength(3)
    expect(store.enabledTrackIds('doc-1')).toEqual([1, 2, 3])
  })

  it('toggleTrack 切换单条轨道启停', async () => {
    const store = useLibraryStore()
    await store.selectDocument('doc-1')
    store.toggleTrack('doc-1', 2)
    expect(store.enabledTrackIds('doc-1')).toEqual([1, 3])
    store.toggleTrack('doc-1', 2)
    expect(store.enabledTrackIds('doc-1')).toEqual([1, 2, 3])
  })

  it('setAllTracks(false) 后 enabledTrackIds 返回空数组（全禁用）', async () => {
    const store = useLibraryStore()
    await store.selectDocument('doc-1')
    store.setAllTracks('doc-1', false)
    expect(store.enabledTrackIds('doc-1')).toEqual([])
    store.setAllTracks('doc-1', true)
    expect(store.enabledTrackIds('doc-1')).toEqual([1, 2, 3])
  })

  it('未加载轨道时 toggleTrack 基于空列表追加', () => {
    const store = useLibraryStore()
    store.toggleTrack('doc-x', 1)
    expect(store.enabledTrackIds('doc-x')).toEqual([1])
  })

  it('importFile 成功后刷新曲谱库（以后端为准）', async () => {
    vi.mocked(api.importMidi).mockResolvedValue({
      doc_id: 'doc-9',
      name: 'demo.mid',
      format: 'Parallel',
      track_count: 2,
      note_count: 10,
      duration_ms: 1000,
      bpm_range: [60, 120],
      source_type: 'midi',
      duplicated: false,
      duplicate_of: null,
    })
    const store = useLibraryStore()
    await store.importFile('C:/demo.mid')
    expect(api.importMidi).toHaveBeenCalledWith('C:/demo.mid')
    // 导入后触发重新拉取（后端 list_documents 返回全量）
    expect(api.listDocuments).toHaveBeenCalled()
    expect(store.documents).toHaveLength(2)
  })

  it('importFile 检测到重复时写入 notice 且不抛错', async () => {
    vi.mocked(api.importMidi).mockResolvedValue({
      doc_id: '',
      name: 'demo.mid',
      format: '',
      track_count: 0,
      note_count: 0,
      duration_ms: 0,
      bpm_range: [0, 0],
      source_type: 'midi',
      duplicated: true,
      duplicate_of: 'doc-1',
    })
    const store = useLibraryStore()
    const summary = await store.importFile('C:/demo.mid')
    expect(summary.duplicated).toBe(true)
    expect(store.notice).toBe('检测到重复曲谱，已自动跳过导入')
    expect(store.error).toBeNull()
  })

  it('importFile 失败时写入 error 并抛出', async () => {
    vi.mocked(api.importMidi).mockRejectedValue('读取文件失败')
    const store = useLibraryStore()
    await expect(store.importFile('C:/bad.mid')).rejects.toBe('读取文件失败')
    expect(store.error).toBe('读取文件失败')
    expect(store.documents).toHaveLength(0)
  })

  it('deleteDocument 调用后端并移除曲谱与启停状态', async () => {
    vi.mocked(api.deleteDocument).mockResolvedValue(undefined)
    const store = useLibraryStore()
    await store.loadDocuments()
    await store.selectDocument('doc-1')
    await store.deleteDocument('doc-1')
    expect(api.deleteDocument).toHaveBeenCalledWith('doc-1')
    expect(store.documents).toHaveLength(1)
    expect(store.currentDocId).toBeNull()
    expect(store.enabledTracks['doc-1']).toBeUndefined()
  })

  it('renameDocument 同步更新本地列表', async () => {
    vi.mocked(api.renameDocument).mockResolvedValue(undefined)
    const store = useLibraryStore()
    await store.loadDocuments()
    await store.renameDocument('doc-1', '新名字')
    expect(store.documents[0].name).toBe('新名字')
  })

  it('createGroup 追加分组；deleteGroup 清空组内归属', async () => {
    vi.mocked(api.createGroup).mockResolvedValue({
      group_id: 'group-1',
      name: '练习曲',
      created_at_ms: 1700000000000,
    })
    vi.mocked(api.deleteGroup).mockResolvedValue(undefined)
    const store = useLibraryStore()
    await store.loadDocuments()
    await store.createGroup('练习曲')
    expect(store.groups).toHaveLength(1)
    // 把 doc-1 移入分组后删除分组 → 本地归属清空
    vi.mocked(api.moveDocumentToGroup).mockResolvedValue(undefined)
    await store.moveToGroup('doc-1', 'group-1')
    expect(store.documents[0].group_id).toBe('group-1')
    await store.deleteGroup('group-1')
    expect(store.groups).toHaveLength(0)
    expect(store.documents[0].group_id).toBeNull()
  })

  it('updatePieceInfo 同步更新本地曲目信息', async () => {
    vi.mocked(api.updatePieceInfo).mockResolvedValue(undefined)
    const store = useLibraryStore()
    await store.loadDocuments()
    await store.updatePieceInfo('doc-1', '标题', null)
    expect(store.documents[0].title).toBe('标题')
    expect(store.documents[0].artist).toBeNull()
  })
})
