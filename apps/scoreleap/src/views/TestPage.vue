<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { checkForeground, listKeymap, testKey } from '../services/api'
import type { ForegroundInfo, KeymapEntry } from '../types'

const testing = ref(false)
const lastHint = ref<string | null>(null)
const lastError = ref<string | null>(null)
const fg = ref<ForegroundInfo | null>(null)
const keymap = ref<KeymapEntry[]>([])
const testingKey = ref<number | null>(null)

/** 刷新前台窗口信息 */
async function refreshForeground(): Promise<void> {
  try {
    fg.value = await checkForeground()
  } catch (e) {
    fg.value = null
    lastError.value = String(e)
  }
}

/** 注入测试键 */
async function sendKey(scan: number, label: string): Promise<void> {
  if (testing.value) return
  testing.value = true
  testingKey.value = scan
  lastHint.value = null
  lastError.value = null
  try {
    const msg = await testKey(scan)
    lastHint.value = `已注入「${label}」：${msg}`
  } catch (e) {
    lastError.value = String(e)
  } finally {
    testing.value = false
    testingKey.value = null
  }
}

onMounted(() => {
  refreshForeground()
  listKeymap()
    .then((k) => {
      keymap.value = k
    })
    .catch((e) => {
      lastError.value = String(e)
    })
  // 前台窗口变化时自动刷新（1s 轮询，仅在测试页活跃时）
  const timer = setInterval(refreshForeground, 1000)
  // 页面卸载时清理
  window.addEventListener('beforeunload', () => clearInterval(timer))
})

const uipiWarning = (): string | null => {
  if (!fg.value) return null
  if (fg.value.elevated && !fg.value.our_elevated) {
    return '检测到前台窗口以管理员身份运行，而本程序不是——SendInput 注入会被系统阻止（UIPI）。请关闭本程序后「以管理员身份运行」再试。'
  }
  if (!fg.value.elevated && fg.value.our_elevated) {
    return '本程序以管理员运行，前台窗口未提权（通常无碍，但建议两边一致）。'
  }
  return null
}
</script>

<template>
  <div class="max-w-4xl">
    <h1 class="text-2xl font-bold text-white">Windows 按键测试</h1>
    <p class="mt-2 text-sm text-slate-400">
      验证按键注入与键位映射。建议打开记事本或游戏自由演奏页进行测试。
    </p>

    <!-- 前台窗口与提权状态 -->
    <section class="mt-6 rounded-xl border border-slate-800 bg-slate-800/40 p-5">
      <div class="flex items-center justify-between">
        <h2 class="text-sm font-medium text-slate-300">当前前台窗口</h2>
        <span class="text-xs text-slate-500">每秒自动刷新</span>
      </div>
      <p class="mt-3 rounded-lg border border-slate-700 bg-slate-900/60 px-4 py-3 font-mono text-sm text-slate-400">
        {{ fg ? `${fg.title}（PID ${fg.pid}）` : '检测中…' }}
        <template v-if="fg">
          <span
            class="ml-2 rounded px-1.5 py-0.5 text-xs"
            :class="fg.elevated ? 'bg-amber-500/20 text-amber-300' : 'bg-emerald-500/20 text-emerald-300'"
          >
            {{ fg.elevated ? '管理员' : '普通权限' }}
          </span>
          <span class="rounded bg-slate-700 px-1.5 py-0.5 text-xs">
            本程序：{{ fg.our_elevated ? '管理员' : '普通权限' }}
          </span>
        </template>
      </p>
      <p
        v-if="uipiWarning()"
        class="mt-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-300"
      >
        ⚠️ {{ uipiWarning() }}
      </p>
    </section>

    <div class="mt-4 grid gap-4 lg:grid-cols-2">
      <!-- 单键测试 -->
      <section class="rounded-xl border border-slate-800 bg-slate-800/40 p-5">
        <h2 class="text-sm font-medium text-slate-300">发送测试按键 A</h2>
        <p class="mt-2 text-xs text-slate-500">
          注入到当前前台窗口（目标窗口需已聚焦）。
        </p>
        <button
          type="button"
          class="mt-3 rounded-lg bg-indigo-500 px-5 py-2.5 text-sm font-medium text-white hover:bg-indigo-400 disabled:opacity-50"
          :disabled="testing"
          @click="sendKey(0x1e, 'A')"
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

      <!-- 排查指引 -->
      <section class="rounded-xl border border-slate-800 bg-slate-800/40 p-5">
        <h2 class="text-sm font-medium text-slate-300">游戏无反应？排查</h2>
        <ul class="mt-3 list-disc space-y-2 pl-5 text-sm text-slate-400">
          <li>
            先在<b class="text-slate-200">记事本</b>测试「A」：出现字母 a
            说明注入正常；无反应说明注入被系统阻止（看上方前台窗口权限提示）。
          </li>
          <li>
            游戏收不到但记事本能收到：多半是<b class="text-slate-200"
              >键位映射与游戏实际键位不符</b
            >——用下方「Profile 键位逐键测试」，在游戏里逐个点击，找到游戏响应的键。
          </li>
          <li>开始演奏后不要切回本程序（前台必须是游戏）。</li>
        </ul>
      </section>
    </div>

    <!-- Profile 键位逐键测试 -->
    <section class="mt-4 rounded-xl border border-slate-800 bg-slate-800/40 p-5">
      <h2 class="text-sm font-medium text-slate-300">
        Profile 键位逐键测试（identity-v，{{ keymap.length }} 键）
      </h2>
      <p class="mt-1 text-xs text-slate-500">
        点击某个键 → 注入到前台窗口。在游戏乐器界面逐个点击：<b
          class="text-slate-300"
          >游戏里能发声/亮起的键</b
        >才是正确键位；若与下方标注不一致，说明 Profile 键位需校准。
      </p>
      <div class="mt-3 flex flex-wrap gap-2">
        <button
          v-for="k in keymap"
          :key="k.note"
          type="button"
          class="rounded-lg border px-3 py-2 font-mono text-sm transition-colors disabled:opacity-50"
          :class="
            testingKey === k.scan
              ? 'border-indigo-400 bg-indigo-500/30 text-white'
              : 'border-slate-700 bg-slate-900/60 text-slate-300 hover:border-indigo-500/50 hover:text-white'
          "
          :disabled="testing"
          :title="`MIDI ${k.note} → 扫描码 0x${k.scan.toString(16)}${k.extended ? '（扩展）' : ''}`"
          @click="sendKey(k.scan, k.label)"
        >
          {{ k.label }}
          <span class="ml-1 text-[10px] text-slate-500">{{ k.note }}</span>
        </button>
      </div>
    </section>
  </div>
</template>
