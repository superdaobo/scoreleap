<script setup lang="ts">
import { ref } from 'vue'

const foregroundWindow = ref('未知（当前版本未启用前台窗口检测）')
const lastHint = ref<string | null>(null)

function sendTestKey(): void {
  // mock 后端仅记录按键、不注入真实输入；真实注入请直接在游戏内使用
  lastHint.value =
    '测试模式使用 mock 后端记录按键，不注入真实输入；真实注入请直接在游戏内使用。'
}
</script>

<template>
  <div class="max-w-3xl">
    <h1 class="text-2xl font-bold text-white">Windows 按键测试</h1>
    <p class="mt-2 text-sm text-slate-400">
      验证按键注入是否正常工作。建议打开记事本或游戏自由演奏页进行测试。
    </p>

    <div class="mt-6 grid gap-4 sm:grid-cols-2">
      <section
        class="rounded-xl border border-slate-800 bg-slate-800/40 p-5"
      >
        <h2 class="text-sm font-medium text-slate-300">当前前台窗口</h2>
        <p
          class="mt-3 rounded-lg border border-slate-700 bg-slate-900/60 px-4 py-3 font-mono text-sm text-slate-400"
        >
          {{ foregroundWindow }}
        </p>
        <p class="mt-2 text-xs text-slate-500">
          提示：SendInput 注入的目标是当前前台窗口，请确保目标窗口已聚焦。
        </p>
      </section>

      <section
        class="rounded-xl border border-slate-800 bg-slate-800/40 p-5"
      >
        <h2 class="text-sm font-medium text-slate-300">发送测试按键</h2>
        <button
          type="button"
          class="mt-3 rounded-lg bg-indigo-500 px-5 py-2.5 text-sm font-medium text-white hover:bg-indigo-400"
          @click="sendTestKey"
        >
          发送测试按键 A
        </button>
        <p
          v-if="lastHint"
          class="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-300"
        >
          {{ lastHint }}
        </p>
      </section>
    </div>

    <section
      class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-5"
    >
      <h2 class="text-sm font-medium text-slate-300">测试引导</h2>
      <ol class="mt-3 list-decimal space-y-2 pl-5 text-sm text-slate-400">
        <li>打开记事本（或游戏自由演奏界面），并使其成为前台窗口。</li>
        <li>
          在「编排」页编译曲谱后点击「开始演奏」（倒计时 3 秒内切换到目标窗口）。
        </li>
        <li>
          紧急停止快捷键
          <kbd
            class="rounded bg-slate-700 px-1.5 py-0.5 font-mono text-xs text-slate-200"
            >Ctrl+Alt+F9</kbd
          >
          可随时中断并释放全部按键。
        </li>
      </ol>
      <div
        class="mt-4 rounded-lg border border-indigo-500/20 bg-indigo-500/10 px-4 py-3 text-xs text-indigo-300"
      >
        测试模式使用 mock 后端记录按键，不注入真实输入；真实注入请直接在游戏内使用。
      </div>
    </section>
  </div>
</template>
