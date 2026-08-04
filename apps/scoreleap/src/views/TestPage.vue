<script setup lang="ts">
import { ref } from 'vue'
import { testKey } from '../services/api'

const testing = ref(false)
const lastHint = ref<string | null>(null)
const lastError = ref<string | null>(null)

/** 发送 A 键（扫描码 0x1E）测试注入 */
async function sendTestKey(): Promise<void> {
  if (testing.value) return
  testing.value = true
  lastHint.value = null
  lastError.value = null
  try {
    lastHint.value = await testKey(0x1e)
  } catch (e) {
    lastError.value = String(e)
  } finally {
    testing.value = false
  }
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
        <h2 class="text-sm font-medium text-slate-300">发送测试按键</h2>
        <p class="mt-2 text-xs text-slate-500">
          点击后向前台窗口注入一次「A」键（按下+抬起）。目标窗口需已聚焦。
        </p>
        <button
          type="button"
          class="mt-3 rounded-lg bg-indigo-500 px-5 py-2.5 text-sm font-medium text-white hover:bg-indigo-400 disabled:opacity-50"
          :disabled="testing"
          @click="sendTestKey"
        >
          {{ testing ? '发送中…' : '发送测试按键 A' }}
        </button>
        <p
          v-if="lastHint"
          class="mt-3 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-300"
        >
          ✅ {{ lastHint }}
        </p>
        <p
          v-if="lastError"
          class="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-300"
        >
          ❌ {{ lastError }}
        </p>
      </section>

      <section
        class="rounded-xl border border-slate-800 bg-slate-800/40 p-5"
      >
        <h2 class="text-sm font-medium text-slate-300">游戏无反应？排查</h2>
        <ul class="mt-3 list-disc space-y-2 pl-5 text-sm text-slate-400">
          <li>
            先打开<b class="text-slate-200">记事本</b>聚焦后点「发送测试按键 A」：记事本出现字母 a
            说明注入正常，问题在键位映射（需在 Profile 文件中校准）；无反应则说明注入被系统阻止。
          </li>
          <li>
            注入被阻止最常见原因：<b class="text-slate-200">游戏以管理员身份运行</b>（UIPI 隔离）。
            解决：以管理员身份运行本程序（右键 → 以管理员身份运行）。
          </li>
          <li>
            确认前台窗口确实是游戏（开始演奏后不要切回本程序窗口；可在倒计时 3
            秒内切到游戏）。
          </li>
        </ul>
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
        <li>
          停止/暂停响应不超过 100ms；若界面仍卡死，请在
          <code class="rounded bg-slate-700 px-1 font-mono text-xs"
            >%APPDATA%\com.superdaobo.scoreleap\logs</code
          >
          查看日志。
        </li>
      </ol>
    </section>
  </div>
</template>
