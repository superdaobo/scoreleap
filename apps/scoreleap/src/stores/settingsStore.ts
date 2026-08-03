import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { GameProfile } from '../types'
import * as api from '../services/api'

const RISK_KEY = 'scoreleap-risk-accepted'
const PROFILE_KEY = 'scoreleap-profile-id'
const LOG_KEY = 'scoreleap-log-level'

/** 日志级别（占位：当前仅影响本地记录） */
export type LogLevel = 'error' | 'warn' | 'info' | 'debug' | 'trace'

function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function writeStorage(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch {
    // 存储不可用时静默失败
  }
}

export const useSettingsStore = defineStore('settings', () => {
  /** 当前选中的 Profile id */
  const profileId = ref<string | null>(readStorage(PROFILE_KEY))
  /** 可用 Profile 列表 */
  const profiles = ref<string[]>([])
  /** 当前 Profile 详情 */
  const current = ref<GameProfile | null>(null)
  /** 是否已接受风险确认（localStorage 持久化） */
  const riskAccepted = ref(readStorage(RISK_KEY) === 'true')
  /** 日志级别（占位） */
  const logLevel = ref<LogLevel>((readStorage(LOG_KEY) as LogLevel | null) ?? 'info')
  /** 错误信息 */
  const error = ref<string | null>(null)

  /** 加载 Profile 列表与当前 Profile */
  async function loadProfiles(): Promise<void> {
    error.value = null
    try {
      profiles.value = await api.listProfiles()
      current.value = await api.currentProfile()
      if (profileId.value) {
        try {
          current.value = await api.loadProfile(profileId.value)
        } catch {
          // 保存的 id 已失效，回退到列表第一个
          profileId.value = null
        }
      }
      if (!profileId.value && profiles.value.length > 0) {
        await selectProfile(profiles.value[0])
      }
    } catch (e) {
      error.value = String(e)
    }
  }

  /** 选择 Profile */
  async function selectProfile(id: string): Promise<void> {
    profileId.value = id
    writeStorage(PROFILE_KEY, id)
    try {
      current.value = await api.loadProfile(id)
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  /** 接受风险确认 */
  function acceptRisk(): void {
    riskAccepted.value = true
    writeStorage(RISK_KEY, 'true')
  }

  /** 重置风险确认（重新进入确认页） */
  function resetRisk(): void {
    riskAccepted.value = false
    try {
      localStorage.removeItem(RISK_KEY)
    } catch {
      // 忽略
    }
  }

  /** 设置日志级别（占位） */
  function setLogLevel(level: LogLevel): void {
    logLevel.value = level
    writeStorage(LOG_KEY, level)
  }

  return {
    profileId,
    profiles,
    current,
    riskAccepted,
    logLevel,
    error,
    loadProfiles,
    selectProfile,
    acceptRisk,
    resetRisk,
    setLogLevel,
  }
})
