<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore, type LogLevel } from '../stores/settingsStore'

const settings = useSettingsStore()
const router = useRouter()

const showRiskModal = ref(false)
const reAgree = ref(false)

onMounted(() => {
  void settings.loadProfiles()
})

const logLevels: { value: LogLevel; label: string }[] = [
  { value: 'error', label: '仅错误' },
  { value: 'warn', label: '警告' },
  { value: 'info', label: '信息' },
  { value: 'debug', label: '调试' },
  { value: 'trace', label: '追踪' },
]

const profileMeta = computed(() => settings.current)
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
