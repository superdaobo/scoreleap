import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useLibraryStore } from '../libraryStore'
import * as api from '../../services/api'

// 拦截 Tauri 命令层，避免在 Node 环境加载 @tauri-apps
vi.mock('../../services/api', () => ({
  importMidi: vi.fn(),
  getTracks: vi.fn(),
}))

/** 安装内存版 localStorage（Node 环境没有） */
function installLocalStorageStub(): void {
  const data = new Map<string, string>()
  const stub: Storage = {
    get length() {
      return data.size
    },
    clear: () => {
      data.clear()
    },
    getItem: (key: string) => data.get(key) ?? null,
    key: (index: number) => [...data.keys()][index] ?? null,
    removeItem: (key: string) => {
      data.delete(key)
    },
    setItem: (key: string, value: string) => {
      data.set(key, String(value))
    },
  }
  Object.defineProperty(globalThis, 'localStorage', {
    value: stub,
    configurable: true,
    writable: true,
  })
}

const SAMPLE_TRACKS = [
  { id: 1, name: '旋律', note_count: 100, enabled: true },
  { id: 2, name: '伴奏', note_count: 50, enabled: true },
  { id: 3, name: '低音', note_count: 30, enabled: true },
]

describe('libraryStore', () => {
  beforeEach(() => {
    installLocalStorageStub()
    setActivePinia(createPinia())
    vi.clearAllMocks()
    vi.mocked(api.getTracks).mockResolvedValue(SAMPLE_TRACKS)
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

  it('importFile 成功后写入 documents 并持久化 localStorage', async () => {
    vi.mocked(api.importMidi).mockResolvedValue({
      doc_id: 'doc-9',
      name: 'demo.mid',
      format: 'Parallel',
      track_count: 2,
      note_count: 10,
      duration_ms: 1000,
      bpm_range: [60, 120],
    })
    const store = useLibraryStore()
    await store.importFile('C:/demo.mid')
    expect(store.documents).toHaveLength(1)
    expect(store.documents[0].doc_id).toBe('doc-9')
    const stored = JSON.parse(
      globalThis.localStorage.getItem('scoreleap-documents') ?? '[]',
    ) as Array<{ doc_id: string }>
    expect(stored).toHaveLength(1)
    expect(stored[0].doc_id).toBe('doc-9')
  })

  it('importFile 失败时写入 error 并抛出', async () => {
    vi.mocked(api.importMidi).mockRejectedValue('读取文件失败')
    const store = useLibraryStore()
    await expect(store.importFile('C:/bad.mid')).rejects.toBe('读取文件失败')
    expect(store.error).toBe('读取文件失败')
    expect(store.documents).toHaveLength(0)
  })

  it('removeDocument 移除曲谱与启停状态', async () => {
    const store = useLibraryStore()
    await store.selectDocument('doc-1')
    store.removeDocument('doc-1')
    expect(store.documents).toHaveLength(0)
    expect(store.currentDocId).toBeNull()
    expect(store.enabledTracks['doc-1']).toBeUndefined()
  })
})
