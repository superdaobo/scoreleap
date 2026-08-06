<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { checkForeground, listKeymap, testKey } from '../services/api'
import { errorText } from '../utils/format'
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
    lastError.value = errorText(e)
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
    lastError.value = errorText(e)
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
      lastError.value = errorText(e)
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
  <div>
    <!-- 页面标题 -->
    <div>
      <h1 class="font-display text-headline-md text-primary">Windows 按键测试</h1>
      <p class="font-code-sm text-code-sm text-on-surface-variant">
        INPUT_INJECTION_DIAGNOSTICS // 建议打开记事本或游戏自由演奏页进行测试
      </p>
    </div>

    <!-- 前台窗口与提权状态 -->
    <section class="bento-item mt-6 rounded-xl p-5">
      <div class="flex items-center justify-between border-b border-outline-variant pb-3">
        <h2 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
          <span class="material-symbols-outlined text-[20px]">desktop_windows</span>
          当前前台窗口
        </h2>
        <span class="font-code-sm text-code-sm text-on-surface-variant">每秒自动刷新</span>
      </div>
      <p class="mt-3 rounded border border-outline-variant bg-surface-container-lowest px-4 py-3 font-code-sm text-code-sm text-on-surface-variant">
        {{ fg ? `${fg.title}（PID ${fg.pid}）` : '检测中…' }}
        <template v-if="fg">
          <span
            class="ml-2 rounded border px-1.5 py-0.5 text-xs"
            :class="fg.elevated ? 'border-amber-500/50 text-amber-300' : 'border-secondary text-secondary'"
          >
            {{ fg.elevated ? '管理员' : '普通权限' }}
          </span>
          <span class="ml-1 rounded border border-outline-variant px-1.5 py-0.5 text-xs">
            本程序：{{ fg.our_elevated ? '管理员' : '普通权限' }}
          </span>
        </template>
      </p>
      <p
        v-if="uipiWarning()"
        class="mt-2 rounded border border-error bg-error-container/20 px-3 py-2 font-code-sm text-code-sm text-error"
      >
        ⚠️ {{ uipiWarning() }}
      </p>
    </section>

    <div class="mt-4 grid gap-4 lg:grid-cols-2">
      <!-- 单键测试 -->
      <section class="bento-item rounded-xl p-5">
        <h2 class="font-body-lg text-body-lg text-primary">发送测试按键 A</h2>
        <p class="mt-2 font-code-sm text-code-sm text-on-surface-variant">
          注入到当前前台窗口（目标窗口需已聚焦）。
        </p>
        <button
          type="button"
          class="mt-3 flex items-center gap-2 rounded bg-primary-container px-5 py-2.5 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
          :disabled="testing"
          @click="sendKey(0x1e, 'A')"
        >
          <span class="material-symbols-outlined text-[18px]">keyboard</span>
          {{ testing ? '发送中…' : '发送测试按键 A' }}
        </button>
        <p
          v-if="lastHint"
          class="mt-3 rounded border border-secondary bg-secondary-container/20 px-3 py-2 font-code-sm text-code-sm text-secondary"
        >
          ✅ {{ lastHint }}
        </p>
        <p
          v-if="lastError"
          class="mt-3 rounded border border-error bg-error-container/20 px-3 py-2 font-code-sm text-code-sm text-error"
        >
          ❌ {{ lastError }}
        </p>
      </section>

      <!-- 排查指引 -->
      <section class="bento-item rounded-xl p-5">
        <h2 class="font-body-lg text-body-lg text-primary">游戏无反应？排查</h2>
        <ul class="mt-3 list-disc space-y-2 pl-5 font-code-sm text-code-sm text-on-surface-variant">
          <li>
            先在<b class="text-on-surface">记事本</b>测试「A」：出现字母 a
            说明注入正常；无反应说明注入被系统阻止（看上方前台窗口权限提示）。
          </li>
          <li>
            游戏收不到但记事本能收到：多半是<b class="text-on-surface"
              >键位映射与游戏实际键位不符</b
            >——用下方「Profile 键位逐键测试」，在游戏里逐个点击，找到游戏响应的键。
          </li>
          <li>开始演奏后不要切回本程序（前台必须是游戏）。</li>
        </ul>
      </section>
    </div>

    <!-- Profile 键位逐键测试 -->
    <section class="bento-item mt-4 rounded-xl p-5">
      <h2 class="font-body-lg text-body-lg text-primary">
        Profile 键位逐键测试（identity-v，{{ keymap.length }} 键）
      </h2>
      <p class="mt-1 font-code-sm text-code-sm text-on-surface-variant">
        点击某个键 → 注入到前台窗口。在游戏乐器界面逐个点击：<b class="text-on-surface"
          >游戏里能发声/亮起的键</b
        >才是正确键位；若与下方标注不一致，说明 Profile 键位需校准。
      </p>
      <div class="mt-3 flex flex-wrap gap-2">
        <button
          v-for="k in keymap"
          :key="k.note"
          type="button"
          class="rounded border px-3 py-2 font-code-sm text-code-sm transition-colors disabled:opacity-50"
          :class="
            testingKey === k.scan
              ? 'border-primary bg-primary/30 text-primary'
              : 'border-outline-variant bg-surface-container-lowest text-on-surface-variant hover:border-primary hover:text-primary'
          "
          :disabled="testing"
          :title="`MIDI ${k.note} → 扫描码 0x${k.scan.toString(16)}${k.extended ? '（扩展）' : ''}`"
          @click="sendKey(k.scan, k.label)"
        >
          {{ k.label }}
          <span class="ml-1 text-[10px] text-on-surface-variant/60">{{ k.note }}</span>
        </button>
      </div>
    </section>
  </div>
</template>
