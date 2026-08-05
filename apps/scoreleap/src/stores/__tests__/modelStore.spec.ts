import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useModelStore } from '../modelStore'
import * as api from '../../services/api'

vi.mock('../../services/api', () => ({
  getTranscriptionModelStatus: vi.fn(),
  checkTranscriptionModelUpdate: vi.fn(),
  downloadTranscriptionModel: vi.fn(),
  cancelTranscriptionModelDownload: vi.fn(),
  rollbackTranscriptionModel: vi.fn(),
}))

const status = {
  status: 'not_installed' as const,
  configured: true,
  model_id: 'basic-pitch',
  installed_version: null,
  latest_version: '1.0.0',
  size_bytes: 100,
  source: 'CDN',
  received_bytes: 0,
  total_bytes: 100,
  error: null,
  can_rollback: false,
}

describe('modelStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('加载按需模型状态且未安装时不可转录', async () => {
    vi.mocked(api.getTranscriptionModelStatus).mockResolvedValue(status)
    const store = useModelStore()
    await store.load()
    expect(store.model.latest_version).toBe('1.0.0')
    expect(store.ready).toBe(false)
  })

  it('用户触发下载后进入 downloading 状态', async () => {
    vi.mocked(api.downloadTranscriptionModel).mockResolvedValue(undefined)
    const store = useModelStore()
    store.model = status
    await store.download()
    expect(api.downloadTranscriptionModel).toHaveBeenCalledTimes(1)
    expect(store.model.status).toBe('downloading')
  })

  it('已安装但有更新时仍可使用当前已验证版本', () => {
    const store = useModelStore()
    store.model = {
      ...status,
      status: 'update_available',
      installed_version: '0.9.0',
    }
    expect(store.ready).toBe(true)
  })
})
