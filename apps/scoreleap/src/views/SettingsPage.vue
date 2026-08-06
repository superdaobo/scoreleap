<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { useRouter } from 'vue-router'
import { useSettingsStore, type LogLevel } from '../stores/settingsStore'
import { useTranscriptionStore } from '../stores/transcriptionStore'
import { useModelStore } from '../stores/modelStore'
import * as api from '../services/api'
import { errorText } from '../utils/format'
import type { DiagnosticsView } from '../types'

const settings = useSettingsStore()
const router = useRouter()
const transcription = useTranscriptionStore()
const modelStore = useModelStore()
const diagnostics = ref<DiagnosticsView | null>(null)
const diagnosticsRunning = ref(false)

async function runDiagnostics(): Promise<void> {
  diagnosticsRunning.value = true
  diagnostics.value = null
  try {
    diagnostics.value = await api.diagnoseTranscription()
  } catch (e) {
    diagnostics.value = {
      model_status: 'failed',
      model_configured: false,
      model_installed_version: null,
      model_path: null,
      model_file_exists: false,
      sidecar_exe_path: null,
      sidecar_exe_exists: false,
      onnx_runtime_path: null,
      onnx_runtime_exists: false,
      onnx_runtime_version: null,
      jobs_dir: null,
      jobs_dir_writable: false,
      app_data_dir: null,
      error: errorText(e),
    }
  } finally {
    diagnosticsRunning.value = false
  }
}

const showRiskModal = ref(false)
const appVersion = ref('')
const reAgree = ref(false)

onMounted(() => {
  void settings.loadProfiles()
  void modelStore.subscribe()
  void modelStore.load()
  void transcription.loadEngineStatus()
  getVersion()
    .then((v) => (appVersion.value = v))
    .catch(() => (appVersion.value = '0.3.1'))
})

const logLevels: { value: LogLevel; label: string }[] = [
  { value: 'error', label: '仅错误' },
  { value: 'warn', label: '警告' },
  { value: 'info', label: '信息' },
  { value: 'debug', label: '调试' },
  { value: 'trace', label: '追踪' },
]

const profileMeta = computed(() => settings.current)
const modelStatusLabel = computed(() => {
  const labels: Record<string, string> = {
    unknown: '检查中',
    configuration_missing: '缺少可信发布配置',
    not_installed: '未安装',
    ready: '可用',
    update_available: '有更新',
    downloading: '下载中',
    failed: '失败',
  }
  return labels[modelStore.model.status] ?? modelStore.model.status
})
const modelStatusClass = computed(() => {
  switch (modelStore.model.status) {
    case 'ready': return 'border-secondary text-secondary bg-secondary-container/20'
    case 'downloading': return 'border-amber-500/50 text-amber-300 bg-amber-500/10'
    case 'failed': return 'border-error text-error bg-error-container/20'
    default: return 'border-outline-variant text-on-surface-variant bg-surface-container-high'
  }
})
const modelSizeLabel = computed(() =>
  modelStore.model.size_bytes == null
    ? '—'
    : `${(modelStore.model.size_bytes / 1024 / 1024).toFixed(1)} MB`,
)
const rangeText = computed(() =>
  profileMeta.value
    ? `${profileMeta.value.midi_low} – ${profileMeta.value.midi_high}`
    : '—',
)

function onProfileChange(): void {
  if (settings.profileId) void settings.selectProfile(settings.profileId)
}

function onLogLevelChange(): void {
  settings.setLogLevel(settings.logLevel)
}

function openRiskModal(): void {
  reAgree.value = false
  showRiskModal.value = true
}

async function confirmModelDownload(): Promise<void> {
  const version = modelStore.model.latest_version ?? '未知版本'
  const confirmed = window.confirm(
    `将从 ${modelStore.model.source ?? '签名目录声明的来源'} 下载模型 ${version}（${modelSizeLabel.value}）。音频不会上传。是否继续？`,
  )
  if (confirmed) await modelStore.download()
}

async function confirmRollback(): Promise<void> {
  if (window.confirm('确认回滚到上一个已验证模型版本？')) await modelStore.rollback()
}

async function confirmReAgree(): Promise<void> {
  if (!reAgree.value) return
  settings.resetRisk()
  showRiskModal.value = false
  await router.push('/risk')
}
</script>

<template>
  <div>
    <!-- 页面标题 -->
    <div>
      <h1 class="font-display text-headline-md text-primary">设置</h1>
      <p class="font-code-sm text-code-sm text-on-surface-variant">
        SYSTEM_CONFIGURATION_NODE // SCORELEAP_ENV
      </p>
    </div>

    <!-- 错误提示条 -->
    <div
      v-if="settings.error || modelStore.model.error"
      class="mt-4 flex items-center gap-3 border border-error bg-error-container/20 px-4 py-3 text-error"
    >
      <span class="material-symbols-outlined">error</span>
      <span class="flex-1 font-code-sm text-code-sm">{{ settings.error || modelStore.model.error }}</span>
    </div>

    <!-- Bento 布局 -->
    <div class="mt-6 grid grid-cols-1 gap-8 lg:grid-cols-12">
      <!-- 左列（8 列） -->
      <div class="space-y-8 lg:col-span-8">
        <!-- 演奏 Profile -->
        <section class="tech-border relative overflow-hidden rounded-lg bg-surface-container-low p-6">
          <div
            class="pointer-events-none absolute inset-0 bg-gradient-to-br from-transparent to-[rgba(46,204,113,0.02)]"
          ></div>
          <div class="mb-6 flex items-center justify-between border-b border-outline-variant pb-4">
            <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
              <span class="material-symbols-outlined text-[20px]">person</span>
              演奏 Profile
            </h3>
            <div class="flex items-center gap-3">
              <select
                :value="settings.profileId ?? ''"
                class="border border-outline-variant bg-surface px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary"
                @change="onProfileChange"
              >
                <option v-for="id in settings.profiles" :key="id" :value="id">
                  {{ id }}
                </option>
              </select>
              <span class="font-code-sm text-code-sm text-on-surface-variant"
                >{{ settings.profiles.length }} AVAILABLE</span
              >
            </div>
          </div>
          <div v-if="profileMeta" class="grid grid-cols-2 gap-4 md:grid-cols-4">
            <div class="tech-border flex flex-col gap-1 rounded bg-surface p-4">
              <span class="font-label-caps text-label-caps text-on-surface-variant">名称</span>
              <span class="font-code-sm text-code-sm text-primary">{{ profileMeta.display_name }}</span>
            </div>
            <div class="tech-border flex flex-col gap-1 rounded bg-surface p-4">
              <span class="font-label-caps text-label-caps text-on-surface-variant">键数</span>
              <span class="font-code-sm text-code-sm text-primary">{{ profileMeta.keys }}</span>
            </div>
            <div class="tech-border flex flex-col gap-1 rounded bg-surface p-4">
              <span class="font-label-caps text-label-caps text-on-surface-variant">音域</span>
              <span class="font-code-sm text-code-sm text-primary">{{ rangeText }}</span>
            </div>
            <div class="tech-border flex flex-col gap-1 rounded bg-surface p-4">
              <span class="font-label-caps text-label-caps text-on-surface-variant">复音上限</span>
              <span class="font-code-sm text-code-sm text-primary">{{ profileMeta.max_polyphony }}</span>
            </div>
          </div>
          <p
            v-if="profileMeta?.warning"
            class="mt-4 rounded border border-error bg-error-container/20 px-4 py-3 font-code-sm text-code-sm text-error"
          >
            ⚠️ {{ profileMeta.warning }}
          </p>
        </section>

        <!-- 音频转录模型 -->
        <section class="tech-border relative rounded-lg bg-surface-container-low p-6">
          <div class="mb-6 flex flex-wrap items-start justify-between gap-3 border-b border-outline-variant pb-4">
            <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
              <span class="material-symbols-outlined text-[20px]">memory</span>
              音频转录模型
            </h3>
            <div class="flex items-center gap-3">
              <span
                class="rounded border px-2.5 py-1 font-code-sm text-code-sm"
                :class="modelStatusClass"
                aria-live="polite"
              >
                {{ modelStatusLabel }}
              </span>
              <button
                type="button"
                class="border border-outline-variant px-3 py-1 font-code-sm text-code-sm text-on-surface-variant transition-colors hover:border-primary hover:text-primary disabled:opacity-50"
                :disabled="diagnosticsRunning"
                @click="runDiagnostics()"
              >
                {{ diagnosticsRunning ? '诊断中…' : '运行转录诊断' }}
              </button>
            </div>
          </div>

          <!-- 诊断结果 -->
          <div
            v-if="diagnostics"
            class="mb-5 rounded border border-outline-variant bg-surface-container-lowest p-4 font-code-sm text-code-sm"
          >
            <h4 class="mb-2 text-on-surface">转录环境诊断</h4>
            <dl class="grid grid-cols-1 gap-x-6 gap-y-1.5 sm:grid-cols-2">
              <div><dt class="inline text-on-surface-variant">模型状态：</dt><dd class="inline text-primary">{{ diagnostics.model_status }}（configured={{ diagnostics.model_configured }}，已装={{ diagnostics.model_installed_version ?? '无' }}）</dd></div>
              <div><dt class="inline text-on-surface-variant">模型文件：</dt><dd class="inline text-primary">{{ diagnostics.model_file_exists ? '存在' : '缺失' }}</dd></div>
              <div><dt class="inline text-on-surface-variant">转录器：</dt><dd class="inline text-primary">{{ diagnostics.sidecar_exe_exists ? '存在' : '缺失' }}</dd></div>
              <div><dt class="inline text-on-surface-variant">ONNX Runtime：</dt><dd class="inline text-primary">{{ diagnostics.onnx_runtime_exists ? '存在' : '缺失' }}（{{ diagnostics.onnx_runtime_version ?? '?' }}）</dd></div>
              <div><dt class="inline text-on-surface-variant">任务目录可写：</dt><dd class="inline text-primary">{{ diagnostics.jobs_dir_writable ? '是' : '否' }}</dd></div>
              <div><dt class="inline text-on-surface-variant">应用数据：</dt><dd class="inline text-primary">{{ diagnostics.app_data_dir ?? '?' }}</dd></div>
            </dl>
            <p v-if="diagnostics.error" class="mt-2 text-error">错误：{{ diagnostics.error }}</p>
          </div>

          <div class="flex flex-col items-start justify-between gap-6 md:flex-row md:items-center">
            <div class="flex-1 space-y-2">
              <div class="flex items-center gap-3">
                <h4 class="font-body-md font-semibold text-on-surface">ScoreLeap Transcribe</h4>
                <span
                  class="rounded border border-outline-variant bg-surface-container-high px-2 py-0.5 font-code-sm text-code-sm text-on-surface-variant"
                  >{{ modelStore.model.installed_version ?? '未安装' }}</span>
              </div>
              <p class="max-w-md font-code-sm text-code-sm text-on-surface-variant">
                最新版本 {{ modelStore.model.latest_version ?? '—' }} · 下载 {{ modelSizeLabel }} · 来源 {{ modelStore.model.source ?? '—' }}
              </p>
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                v-if="['not_installed', 'update_available', 'failed'].includes(modelStore.model.status)"
                type="button"
                class="flex items-center gap-2 rounded bg-primary px-6 py-3 font-label-caps text-label-caps text-on-primary transition-colors hover:bg-surface-tint disabled:opacity-50"
                :disabled="modelStore.busy || !modelStore.model.configured"
                aria-label="下载并验证转录模型"
                @click="confirmModelDownload"
              >
                <span class="material-symbols-outlined text-[18px]">download</span>
                {{ modelStore.model.status === 'update_available' ? '确认更新' : modelStore.model.status === 'failed' ? '重试下载' : '下载并安装' }}
              </button>
              <button
                v-if="modelStore.model.status === 'downloading'"
                type="button"
                class="rounded border border-error px-4 py-3 font-code-sm text-code-sm text-error hover:bg-error/10"
                aria-label="取消模型下载"
                @click="modelStore.cancel"
              >取消下载</button>
              <button
                type="button"
                class="rounded border border-outline-variant px-4 py-3 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary disabled:opacity-40"
                :disabled="modelStore.busy || modelStore.model.status === 'downloading'"
                @click="modelStore.checkUpdate"
              >检查更新</button>
              <button
                v-if="modelStore.model.can_rollback"
                type="button"
                class="rounded border border-surface-tint px-4 py-3 font-code-sm text-code-sm text-primary transition-colors hover:bg-primary/10 disabled:opacity-40"
                :disabled="modelStore.busy || modelStore.model.status === 'downloading'"
                @click="confirmRollback"
              >回滚版本</button>
            </div>
          </div>

          <!-- 下载进度 -->
          <div v-if="modelStore.model.status === 'downloading'" class="mt-6 space-y-2" aria-live="polite">
            <div class="flex justify-between font-code-sm text-code-sm">
              <span class="text-primary">Downloading... {{ modelStore.progressPercent }}%</span>
              <span class="text-on-surface-variant"
                >{{ modelStore.model.received_bytes.toLocaleString() }} / {{ modelStore.model.total_bytes?.toLocaleString() ?? '—' }} bytes</span>
            </div>
            <div class="h-1 w-full overflow-hidden bg-surface-container-highest">
              <div
                class="h-full bg-primary transition-all duration-300"
                :style="{ width: `${modelStore.progressPercent}%` }"
              ></div>
            </div>
          </div>
        </section>

        <!-- Basic Pitch 高级阈值；Transkun 使用模型自身的区间解码，不暴露这些阈值。 -->
        <section
          v-if="transcription.engine === 'fast'"
          class="tech-border rounded-lg bg-surface-container-low p-6"
        >
          <div class="mb-6 flex items-center justify-between border-b border-outline-variant pb-4">
            <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
              <span class="material-symbols-outlined text-[20px]">tune</span>
              高级阈值覆盖
            </h3>
            <label class="relative inline-flex cursor-pointer items-center">
              <input v-model="transcription.advancedEnabled" type="checkbox" class="peer sr-only" />
              <div
                class="peer h-5 w-9 rounded-full bg-surface-variant after:absolute after:left-[2px] after:top-[2px] after:h-4 after:w-4 after:rounded-full after:border after:border-on-surface after:bg-on-surface after:transition-all peer-checked:bg-primary peer-checked:after:translate-x-full peer-checked:after:border-white"
              ></div>
            </label>
          </div>
          <div class="grid gap-4 sm:grid-cols-3" :class="{ 'opacity-50': !transcription.advancedEnabled }">
            <label class="font-code-sm text-code-sm text-on-surface-variant" for="onset-threshold">
              起音阈值（{{ transcription.onsetThreshold.toFixed(2) }}）
              <input
                id="onset-threshold"
                v-model.number="transcription.onsetThreshold"
                type="range"
                min="0"
                max="1"
                step="0.01"
                class="mt-2 h-1 w-full cursor-pointer appearance-none rounded-lg bg-surface-variant accent-primary"
                :disabled="!transcription.advancedEnabled"
              />
            </label>
            <label class="font-code-sm text-code-sm text-on-surface-variant" for="frame-threshold">
              持续音阈值（{{ transcription.frameThreshold.toFixed(2) }}）
              <input
                id="frame-threshold"
                v-model.number="transcription.frameThreshold"
                type="range"
                min="0"
                max="1"
                step="0.01"
                class="mt-2 h-1 w-full cursor-pointer appearance-none rounded-lg bg-surface-variant accent-primary"
                :disabled="!transcription.advancedEnabled"
              />
            </label>
            <label class="font-code-sm text-code-sm text-on-surface-variant" for="minimum-note-ms">
              最短音符（毫秒）
              <input
                id="minimum-note-ms"
                v-model.number="transcription.minimumNoteMs"
                type="number"
                min="20"
                max="2000"
                step="1"
                class="mt-2 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
                :disabled="!transcription.advancedEnabled"
              />
            </label>
          </div>
        </section>

        <!-- 日志级别 + 风险与合规 + 关于 -->
        <section class="tech-border rounded-lg bg-surface-container-low p-6">
          <div class="mb-6 border-b border-outline-variant pb-4">
            <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
              <span class="material-symbols-outlined text-[20px]">terminal</span>
              日志级别
            </h3>
            <select
              :value="settings.logLevel"
              class="mt-3 border border-outline-variant bg-surface px-3 py-2 font-code-sm text-code-sm text-on-surface focus:border-primary"
              @change="onLogLevelChange"
            >
              <option v-for="l in logLevels" :key="l.value" :value="l.value">
                {{ l.label }}
              </option>
            </select>
            <p class="mt-2 text-xs text-on-surface-variant">
              当前版本日志级别仅记录在本地，后端日志由环境变量控制。
            </p>
          </div>

          <div class="flex flex-col gap-6 border-b border-outline-variant pb-6">
            <div>
              <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
                <span class="material-symbols-outlined text-[20px]">warning</span>
                风险与合规
              </h3>
              <p class="mt-2 text-sm text-on-surface-variant">
                查看风险说明并重新确认，或将重新进入确认页。
              </p>
              <button
                type="button"
                class="mt-3 border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
                @click="openRiskModal"
              >重新查看风险说明</button>
            </div>
          </div>

          <div class="mt-6">
            <h3 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
              <span class="material-symbols-outlined text-[20px]">info</span>
              关于
            </h3>
            <dl class="mt-3 space-y-2 font-code-sm text-code-sm">
              <div class="flex gap-2">
                <dt class="w-16 text-on-surface-variant">版本</dt>
                <dd class="text-on-surface">{{ appVersion || '0.3.1' }}</dd>
              </div>
              <div class="flex gap-2">
                <dt class="w-16 text-on-surface-variant">许可证</dt>
                <dd class="text-on-surface">GPL-3.0</dd>
              </div>
              <div class="flex gap-2">
                <dt class="w-16 text-on-surface-variant">隐私</dt>
                <dd class="text-on-surface">MIDI 文件仅在本机处理，不上传；不收集遥测数据。</dd>
              </div>
            </dl>
          </div>
        </section>
      </div>

      <!-- 右列（4 列）：转录引擎 + 识别预设 + 快捷键 -->
      <div class="space-y-8 lg:col-span-4">
        <!-- 转录引擎 -->
        <section class="tech-border rounded-lg bg-surface-container-low p-6">
          <h3 class="mb-6 flex items-center gap-2 border-b border-outline-variant pb-4 font-body-lg text-body-lg text-primary">
            <span class="material-symbols-outlined text-[20px]">neurology</span>
            转录引擎
          </h3>
          <div class="space-y-4">
            <label
              class="tech-border block cursor-pointer rounded p-4 transition-colors"
              :class="transcription.engine === 'fast' ? 'border-primary bg-surface' : 'bg-surface hover:bg-surface-container-highest'"
            >
              <div class="mb-2 flex items-center justify-between">
                <span class="font-body-md font-semibold text-on-surface">快速（Basic Pitch）</span>
                <span v-if="transcription.engine === 'fast'" class="material-symbols-outlined text-[18px] text-primary">check_circle</span>
              </div>
              <p class="font-code-sm text-code-sm text-on-surface-variant">启动快、体积小，适合简单钢琴与快速预览。</p>
              <input v-model="transcription.engine" type="radio" name="transcription-engine" value="fast" class="sr-only" />
            </label>

            <label
              class="tech-border block rounded p-4 transition-colors"
              :class="[
                transcription.engine === 'high_quality' ? 'border-primary bg-surface' : 'bg-surface',
                transcription.engineStatus.high_quality_available
                  ? 'cursor-pointer hover:bg-surface-container-highest'
                  : 'cursor-not-allowed opacity-50',
              ]"
            >
              <div class="mb-2 flex items-center justify-between">
                <span class="font-body-md font-semibold text-on-surface">高质量钢琴（Transkun v2）</span>
                <span v-if="transcription.engine === 'high_quality'" class="material-symbols-outlined text-[18px] text-primary">check_circle</span>
              </div>
              <p class="font-code-sm text-code-sm text-on-surface-variant">
                面向纯钢琴的 Transformer + Semi-CRF，CPU 本地运行，速度较慢但音符边界更准确。
              </p>
              <p v-if="!transcription.engineStatus.high_quality_available" class="mt-2 text-xs text-amber-300">
                {{ transcription.engineStatus.high_quality_error ?? '当前安装包未包含高质量组件' }}
              </p>
              <input
                v-model="transcription.engine"
                type="radio"
                name="transcription-engine"
                value="high_quality"
                class="sr-only"
                :disabled="!transcription.engineStatus.high_quality_available"
              />
            </label>
          </div>
        </section>

        <!-- 识别预设（Basic Pitch） -->
        <section v-if="transcription.engine === 'fast'" class="tech-border rounded-lg bg-surface-container-low p-6">
          <h3 class="mb-6 flex items-center gap-2 border-b border-outline-variant pb-4 font-body-lg text-body-lg text-primary">
            <span class="material-symbols-outlined text-[20px]">tune</span>
            识别预设
          </h3>
          <div class="space-y-4">
            <label
              v-for="item in [
                { value: 'balanced', label: '均衡 (Balanced)', detail: '准确率与杂音的默认平衡' },
                { value: 'detail', label: '细节 (Detail)', detail: '保留弱音，可能增加误音' },
                { value: 'noise_reduced', label: '降噪 (Denoise)', detail: '压制杂音，适合轻噪钢琴' },
              ]"
              :key="item.value"
              class="tech-border block cursor-pointer rounded p-4 transition-colors"
              :class="
                transcription.preset === item.value
                  ? 'border-primary bg-surface'
                  : 'bg-surface hover:bg-surface-container-highest'
              "
            >
              <div class="mb-2 flex items-center justify-between">
                <span
                  class="font-body-md font-semibold"
                  :class="transcription.preset === item.value ? 'text-primary' : 'text-on-surface'"
                  >{{ item.label }}</span
                >
                <span
                  v-if="transcription.preset === item.value"
                  class="material-symbols-outlined text-[18px] text-primary"
                  >check_circle</span
                >
              </div>
              <p class="font-code-sm text-code-sm text-on-surface-variant">{{ item.detail }}</p>
              <input
                v-model="transcription.preset"
                type="radio"
                name="transcription-preset"
                :value="item.value"
                class="sr-only"
              />
            </label>
          </div>
        </section>

        <!-- 快捷键 -->
        <section class="tech-border rounded-lg bg-surface-container-low p-6">
          <h3 class="mb-6 flex items-center gap-2 border-b border-outline-variant pb-4 font-body-lg text-body-lg text-primary">
            <span class="material-symbols-outlined text-[20px]">keyboard</span>
            快捷键
          </h3>
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <span class="font-code-sm text-code-sm text-on-surface-variant">紧急停止（释放全部按键）</span>
              <div class="flex items-center gap-1 font-code-sm text-code-sm text-primary">
                <kbd class="kbd-key">Ctrl</kbd>
                <span class="py-1">+</span>
                <kbd class="kbd-key">Alt</kbd>
                <span class="py-1">+</span>
                <kbd class="kbd-key">F9</kbd>
              </div>
            </div>
            <p class="pt-2 text-xs text-on-surface-variant">
              若注册失败（如被其他程序占用），请在应用内点击「紧急停止」按钮。
            </p>
          </div>
        </section>
      </div>
    </div>

    <!-- 风险弹窗 -->
    <div
      v-if="showRiskModal"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="showRiskModal = false"
    >
      <div
        class="w-full max-w-lg border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
      >
        <h3 class="font-display text-[22px] text-on-surface">风险与合规说明</h3>
        <ul class="mt-4 space-y-3 text-sm">
          <li class="flex gap-2 text-error">
            <span class="material-symbols-outlined text-[18px]">gavel</span>
            <span>第三方自动化工具可能违反游戏用户协议，存在封号风险。</span>
          </li>
          <li class="flex gap-2 text-primary">
            <span class="material-symbols-outlined text-[18px]">sports_esports</span>
            <span>仅用于自由演奏/个人空间等非竞技场景。</span>
          </li>
          <li class="flex gap-2 text-secondary">
            <span class="material-symbols-outlined text-[18px]">shield_lock</span>
            <span>本工具不提供反检测/隐藏自动化能力；隐私本地处理，文件不上传。</span>
          </li>
        </ul>
        <label class="mt-5 flex cursor-pointer items-start gap-2 text-sm text-on-surface">
          <input
            v-model="reAgree"
            type="checkbox"
            class="tech-checkbox mt-0.5 h-4 w-4 cursor-pointer appearance-none border border-outline-variant bg-surface-container-highest"
          />
          <span>我已阅读并理解风险，重新确认</span>
        </label>
        <div class="mt-5 flex gap-3">
          <button
            type="button"
            class="flex-1 border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
            @click="showRiskModal = false"
          >取消</button>
          <button
            type="button"
            class="flex-1 rounded bg-error px-4 py-2 font-label-caps text-label-caps text-on-error transition-colors hover:bg-error-container disabled:opacity-40"
            :disabled="!reAgree"
            @click="confirmReAgree"
          >重新确认</button>
        </div>
      </div>
    </div>
  </div>
</template>
