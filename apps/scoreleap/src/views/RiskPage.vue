<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore } from '../stores/settingsStore'

const router = useRouter()
const settings = useSettingsStore()
const agreed = ref(false)

function acceptAndContinue(): void {
  if (!agreed.value) return
  settings.acceptRisk()
  void router.replace('/')
}
</script>

<template>
  <div class="mx-auto max-w-2xl py-10">
    <div
      class="rounded-2xl border border-slate-800 bg-slate-900 p-8 shadow-xl"
    >
      <h1 class="text-2xl font-bold text-white">风险与合规确认</h1>
      <p class="mt-2 text-sm text-slate-400">
        使用 ScoreLeap 谱跃前，请阅读并确认以下内容：
      </p>

      <ul class="mt-6 space-y-4">
        <li
          class="flex gap-3 rounded-lg border border-amber-500/20 bg-amber-500/5 p-4"
        >
          <span class="text-lg">⚠️</span>
          <p class="text-sm text-amber-200/90">
            第三方自动化工具可能违反游戏用户协议，存在<strong
              class="text-amber-300"
              >封号风险</strong
            >。请自行评估并承担使用后果。
          </p>
        </li>
        <li
          class="flex gap-3 rounded-lg border border-slate-700 bg-slate-800/50 p-4"
        >
          <span class="text-lg">🎯</span>
          <p class="text-sm text-slate-300">
            仅用于<strong class="text-indigo-300">自由演奏 / 个人空间</strong
            >等非竞技场景，请勿在排位、竞技模式中使用。
          </p>
        </li>
        <li
          class="flex gap-3 rounded-lg border border-slate-700 bg-slate-800/50 p-4"
        >
          <span class="text-lg">🔒</span>
          <p class="text-sm text-slate-300">
            本工具<strong class="text-indigo-300"
              >不提供反检测 / 隐藏自动化能力</strong
            >；MIDI 文件仅在本地处理，不上传，不收集遥测数据。
          </p>
        </li>
      </ul>

      <label
        class="mt-8 flex cursor-pointer items-start gap-3 text-sm text-slate-300"
      >
        <input
          v-model="agreed"
          type="checkbox"
          class="mt-0.5 h-4 w-4 accent-indigo-500"
        />
        <span>我已阅读并理解上述风险与合规说明</span>
      </label>

      <button
        type="button"
        :disabled="!agreed"
        class="mt-6 w-full rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-4 py-3 font-medium text-white transition-opacity disabled:cursor-not-allowed disabled:opacity-40"
        @click="acceptAndContinue"
      >
        同意并继续
      </button>
    </div>
  </div>
</template>
