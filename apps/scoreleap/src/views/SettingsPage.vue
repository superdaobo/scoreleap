<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore, type LogLevel } from '../stores/settingsStore'
import { useTranscriptionStore } from '../stores/transcriptionStore'
import { useModelStore } from '../stores/modelStore'
<<<<<<< HEAD
import * as api from '../services/api'
import { errorText } from '../utils/format'
import type { DiagnosticsView } from '../types'
=======
>>>>>>> feat/onnx-product-integration

const settings = useSettingsStore()
const router = useRouter()
const transcription = useTranscriptionStore()
const modelStore = useModelStore()
<<<<<<< HEAD
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
=======
>>>>>>> feat/onnx-product-integration

const showRiskModal = ref(false)
const reAgree = ref(false)

onMounted(() => {
  void settings.loadProfiles()
  void modelStore.subscribe()
  void modelStore.load()
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
    unknown: '正在检查',
    configuration_missing: '缺少可信发布配置',
    not_installed: '未安装',
    ready: '可用',
    update_available: '有更新',
    downloading: '下载中',
    failed: '失败',
  }
  return labels[modelStore.model.status] ?? modelStore.model.status
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
  <div class="max-w-3xl">
    <h1 class="text-2xl font-bold text-white">设置</h1>

    <!-- Profile -->
    <section
      class="mt-6 rounded-xl border border-slate-800 bg-slate-800/40 p-6"
    >
      <h2 class="text-sm font-medium text-slate-300">演奏 Profile</h2>
      <div class="mt-3 flex flex-wrap items-center gap-3">
        <select
          :value="settings.profileId ?? ''"
          class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-200"
          @change="onProfileChange"
        >
          <option v-for="id in settings.profiles" :key="id" :value="id">
            {{ id }}
          </option>
        </select>
        <span class="text-xs text-slate-500"
          >{{ settings.profiles.length }} 个可用 Profile</span
        >
      </div>
      <dl
        v-if="profileMeta"
        class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4"
      >
        <div>
          <dt class="text-xs text-slate-500">名称</dt>
          <dd class="mt-1 text-sm text-slate-200">
            {{ profileMeta.display_name }}
          </dd>
        </div>
        <div>
          <dt class="text-xs text-slate-500">键数</dt>
          <dd class="mt-1 text-sm text-slate-200">{{ profileMeta.keys }}</dd>
        </div>
        <div>
          <dt class="text-xs text-slate-500">音域</dt>
          <dd class="mt-1 text-sm text-slate-200">{{ rangeText }}</dd>
        </div>
        <div>
          <dt class="text-xs text-slate-500">复音上限</dt>
          <dd class="mt-1 text-sm text-slate-200">
            {{ profileMeta.max_polyphony }}
          </dd>
        </div>
      </dl>
      <p
        v-if="profileMeta?.warning"
        class="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        ⚠️ {{ profileMeta.warning }}
      </p>
      <p v-if="settings.error" class="mt-3 text-xs text-red-300">
        {{ settings.error }}
      </p>
    </section>

    <!-- 音频转录与按需模型 -->
    <section class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-6">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 class="text-sm font-medium text-slate-300">音频转录模型</h2>
          <p class="mt-1 text-xs text-slate-500">
            模型仅在你确认后下载；MP3/WAV/FLAC 音频始终只在本机处理。
          </p>
        </div>
        <span class="rounded-full bg-slate-700/70 px-2.5 py-1 text-xs text-slate-200" aria-live="polite">
          {{ modelStatusLabel }}
        </span>
<<<<<<< HEAD
        <button
          type="button"
          class="rounded-lg border border-slate-600 px-3 py-1.5 text-xs text-slate-200 transition hover:border-slate-400 disabled:opacity-50"
          :disabled="diagnosticsRunning"
          @click="runDiagnostics()"
        >
          {{ diagnosticsRunning ? '诊断中…' : '运行转录诊断' }}
        </button>
      </div>
      <div
        v-if="diagnostics"
        class="mt-4 rounded-lg border border-slate-700 bg-slate-900/60 p-4 text-xs text-slate-300"
      >
        <h3 class="mb-2 text-sm font-medium text-slate-200">转录环境诊断</h3>
        <dl class="grid grid-cols-1 gap-x-6 gap-y-1.5 sm:grid-cols-2">
          <div><dt class="inline text-slate-500">模型状态：</dt><dd class="inline">{{ diagnostics.model_status }}（configured={{ diagnostics.model_configured }}，已装={{ diagnostics.model_installed_version ?? '无' }}）</dd></div>
          <div><dt class="inline text-slate-500">模型文件：</dt><dd class="inline">{{ diagnostics.model_file_exists ? '存在' : '缺失' }} {{ diagnostics.model_path ?? '' }}</dd></div>
          <div><dt class="inline text-slate-500">转录器：</dt><dd class="inline">{{ diagnostics.sidecar_exe_exists ? '存在' : '缺失' }} {{ diagnostics.sidecar_exe_path ?? '' }}</dd></div>
          <div><dt class="inline text-slate-500">ONNX Runtime：</dt><dd class="inline">{{ diagnostics.onnx_runtime_exists ? '存在' : '缺失' }}（{{ diagnostics.onnx_runtime_version ?? '?' }}）</dd></div>
          <div><dt class="inline text-slate-500">任务目录可写：</dt><dd class="inline">{{ diagnostics.jobs_dir_writable ? '是' : '否' }} {{ diagnostics.jobs_dir ?? '' }}</dd></div>
          <div><dt class="inline text-slate-500">应用数据：</dt><dd class="inline">{{ diagnostics.app_data_dir ?? '?' }}</dd></div>
        </dl>
        <p v-if="diagnostics.error" class="mt-2 text-red-300">错误：{{ diagnostics.error }}</p>
=======
>>>>>>> feat/onnx-product-integration
      </div>

      <dl class="mt-4 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
        <div><dt class="text-xs text-slate-500">已安装</dt><dd class="mt-1 text-slate-200">{{ modelStore.model.installed_version ?? '—' }}</dd></div>
        <div><dt class="text-xs text-slate-500">最新版本</dt><dd class="mt-1 text-slate-200">{{ modelStore.model.latest_version ?? '—' }}</dd></div>
        <div><dt class="text-xs text-slate-500">下载大小</dt><dd class="mt-1 text-slate-200">{{ modelSizeLabel }}</dd></div>
        <div><dt class="text-xs text-slate-500">当前来源</dt><dd class="mt-1 text-slate-200">{{ modelStore.model.source ?? '—' }}</dd></div>
      </dl>

      <div v-if="modelStore.model.status === 'downloading'" class="mt-4" aria-live="polite">
        <div class="flex justify-between text-xs text-slate-400">
          <span>正在下载并校验</span><span>{{ modelStore.progressPercent }}%</span>
        </div>
        <progress
          class="mt-2 h-2 w-full accent-indigo-500"
          :value="modelStore.model.received_bytes"
          :max="modelStore.model.total_bytes ?? 1"
          aria-label="模型下载进度"
        ></progress>
      </div>

      <p v-if="modelStore.model.error" class="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-300" role="alert">
        {{ modelStore.model.error }}
      </p>

      <div class="mt-4 flex flex-wrap gap-2">
        <button
          v-if="modelStore.model.status === 'not_installed' || modelStore.model.status === 'update_available' || modelStore.model.status === 'failed'"
          type="button"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm text-white hover:bg-indigo-500 disabled:opacity-40"
          :disabled="modelStore.busy || !modelStore.model.configured"
          aria-label="下载并验证转录模型"
          @click="confirmModelDownload"
        >
          {{ modelStore.model.status === 'update_available' ? '确认更新' : modelStore.model.status === 'failed' ? '重试下载' : '下载模型' }}
        </button>
        <button
          v-if="modelStore.model.status === 'downloading'"
          type="button"
          class="rounded-lg border border-red-500/40 px-4 py-2 text-sm text-red-300 hover:bg-red-500/10"
          aria-label="取消模型下载"
          @click="modelStore.cancel"
        >取消下载</button>
        <button
          type="button"
          class="rounded-lg border border-slate-600 px-4 py-2 text-sm text-slate-300 hover:bg-slate-700 disabled:opacity-40"
          :disabled="modelStore.busy || modelStore.model.status === 'downloading'"
          @click="modelStore.checkUpdate"
        >检查更新</button>
        <button
          v-if="modelStore.model.can_rollback"
          type="button"
          class="rounded-lg border border-amber-500/40 px-4 py-2 text-sm text-amber-300 hover:bg-amber-500/10 disabled:opacity-40"
          :disabled="modelStore.busy || modelStore.model.status === 'downloading'"
          @click="confirmRollback"
        >回滚版本</button>
      </div>

      <fieldset class="mt-6 border-t border-slate-700 pt-5">
        <legend class="text-sm font-medium text-slate-300">识别预设</legend>
        <div class="mt-3 grid gap-2 sm:grid-cols-3">
          <label v-for="item in [
            { value: 'balanced', label: '均衡', detail: '准确率与杂音的默认平衡' },
            { value: 'detail', label: '细节', detail: '保留弱音，可能增加误音' },
            { value: 'noise_reduced', label: '降噪', detail: '压制杂音，适合轻噪钢琴' },
          ]" :key="item.value" class="cursor-pointer rounded-lg border border-slate-700 p-3 has-[:checked]:border-indigo-500 has-[:checked]:bg-indigo-500/10">
            <input v-model="transcription.preset" type="radio" name="transcription-preset" :value="item.value" class="accent-indigo-500" />
            <span class="ml-2 text-sm text-slate-200">{{ item.label }}</span>
            <span class="mt-1 block text-xs text-slate-500">{{ item.detail }}</span>
          </label>
        </div>
      </fieldset>

      <label class="mt-5 flex items-center gap-2 text-sm text-slate-300">
        <input v-model="transcription.advancedEnabled" type="checkbox" class="accent-indigo-500" />
        启用高级阈值覆盖
      </label>
      <div class="mt-3 grid gap-4 sm:grid-cols-3" :class="{ 'opacity-50': !transcription.advancedEnabled }">
        <label class="text-xs text-slate-400" for="onset-threshold">起音阈值（{{ transcription.onsetThreshold.toFixed(2) }}）
          <input id="onset-threshold" v-model.number="transcription.onsetThreshold" type="range" min="0" max="1" step="0.01" class="mt-2 w-full accent-indigo-500" :disabled="!transcription.advancedEnabled" />
        </label>
        <label class="text-xs text-slate-400" for="frame-threshold">持续音阈值（{{ transcription.frameThreshold.toFixed(2) }}）
          <input id="frame-threshold" v-model.number="transcription.frameThreshold" type="range" min="0" max="1" step="0.01" class="mt-2 w-full accent-indigo-500" :disabled="!transcription.advancedEnabled" />
        </label>
        <label class="text-xs text-slate-400" for="minimum-note-ms">最短音符（毫秒）
<<<<<<< HEAD
          <input id="minimum-note-ms" v-model.number="transcription.minimumNoteMs" type="number" min="10" max="5000" class="mt-2 w-full rounded border border-slate-700 bg-slate-900 px-2 py-1.5 text-slate-200" :disabled="!transcription.advancedEnabled" />
=======
          <input id="minimum-note-ms" v-model.number="transcription.minimumNoteMs" type="number" min="20" max="2000" step="1" class="mt-2 w-full rounded border border-slate-700 bg-slate-900 px-2 py-1.5 text-slate-200" :disabled="!transcription.advancedEnabled" />
>>>>>>> feat/onnx-product-integration
        </label>
      </div>
    </section>

    <!-- 快捷键 -->
    <section
      class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-6"
    >
      <h2 class="text-sm font-medium text-slate-300">快捷键</h2>
      <p class="mt-3 text-sm text-slate-400">
        紧急停止（立即释放全部按键）
      </p>
      <p class="mt-2">
        <kbd
          class="rounded-lg border border-slate-600 bg-slate-900 px-3 py-1.5 font-mono text-sm text-slate-100"
          >Ctrl</kbd
        ><span class="mx-1 text-slate-500">+</span
        ><kbd
          class="rounded-lg border border-slate-600 bg-slate-900 px-3 py-1.5 font-mono text-sm text-slate-100"
          >Alt</kbd
        ><span class="mx-1 text-slate-500">+</span
        ><kbd
          class="rounded-lg border border-slate-600 bg-slate-900 px-3 py-1.5 font-mono text-sm text-slate-100"
          >F9</kbd
        >
      </p>
      <p class="mt-2 text-xs text-slate-500">
        若注册失败（如被其他程序占用），请在应用内点击「紧急停止」按钮。
      </p>
    </section>

    <!-- 日志级别 -->
    <section
      class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-6"
    >
      <h2 class="text-sm font-medium text-slate-300">日志级别</h2>
      <select
        :value="settings.logLevel"
        class="mt-3 rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-200"
        @change="onLogLevelChange"
      >
        <option v-for="l in logLevels" :key="l.value" :value="l.value">
          {{ l.label }}
        </option>
      </select>
      <p class="mt-2 text-xs text-slate-500">
        占位：当前版本日志级别仅记录在本地，后端日志由环境变量控制。
      </p>
    </section>

    <!-- 风险与合规 -->
    <section
      class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-6"
    >
      <h2 class="text-sm font-medium text-slate-300">风险与合规</h2>
      <p class="mt-3 text-sm text-slate-400">
        查看风险说明并重新确认，或将重新进入确认页。
      </p>
      <button
        type="button"
        class="mt-3 rounded-lg border border-slate-600 px-4 py-2 text-sm text-slate-200 hover:bg-slate-700"
        @click="openRiskModal"
      >
        重新查看风险说明
      </button>
    </section>

    <!-- 关于 -->
    <section
      class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-6"
    >
      <h2 class="text-sm font-medium text-slate-300">关于</h2>
      <dl class="mt-3 space-y-2 text-sm">
        <div class="flex gap-2">
          <dt class="w-16 text-slate-500">版本</dt>
          <dd class="text-slate-200">0.1.0</dd>
        </div>
        <div class="flex gap-2">
          <dt class="w-16 text-slate-500">许可证</dt>
          <dd class="text-slate-200">GPL-3.0</dd>
        </div>
        <div class="flex gap-2">
          <dt class="w-16 text-slate-500">隐私</dt>
          <dd class="text-slate-200">
            MIDI 文件仅在本机处理，不上传；不收集遥测数据。
          </dd>
        </div>
        <div class="flex gap-2">
          <dt class="w-16 text-slate-500">项目</dt>
          <dd class="text-slate-200">
            <a
              class="text-indigo-400 hover:underline"
              href="https://github.com/"
              target="_blank"
              rel="noreferrer"
              >github.com（占位链接）</a
            >
          </dd>
        </div>
      </dl>
    </section>

    <!-- 风险弹窗 -->
    <div
      v-if="showRiskModal"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="showRiskModal = false"
    >
      <div
        class="w-full max-w-lg rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl"
      >
        <h3 class="text-lg font-bold text-white">风险与合规说明</h3>
        <ul class="mt-4 space-y-3 text-sm">
          <li class="flex gap-2 text-amber-200/90">
            <span>⚠️</span>
            <span>第三方自动化工具可能违反游戏用户协议，存在封号风险。</span>
          </li>
          <li class="flex gap-2 text-slate-300">
            <span>🎯</span>
            <span>仅用于自由演奏/个人空间等非竞技场景。</span>
          </li>
          <li class="flex gap-2 text-slate-300">
            <span>🔒</span>
            <span
              >本工具不提供反检测/隐藏自动化能力；隐私本地处理，文件不上传。</span
            >
          </li>
        </ul>
        <label class="mt-5 flex cursor-pointer items-start gap-2 text-sm text-slate-300">
          <input
            v-model="reAgree"
            type="checkbox"
            class="mt-0.5 h-4 w-4 accent-indigo-500"
          />
          <span>我已阅读并理解风险，重新确认</span>
        </label>
        <div class="mt-5 flex gap-3">
          <button
            type="button"
            class="flex-1 rounded-lg border border-slate-600 px-4 py-2 text-sm text-slate-200 hover:bg-slate-800"
            @click="showRiskModal = false"
          >
            取消
          </button>
          <button
            type="button"
            class="flex-1 rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500 disabled:opacity-40"
            :disabled="!reAgree"
            @click="confirmReAgree"
          >
            重新确认
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
