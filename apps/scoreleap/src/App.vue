<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getVersion } from '@tauri-apps/api/app'
import { getCrashFlag, emergencyStop } from './services/api'
import { usePlaybackStore } from './stores/playbackStore'

const route = useRoute()
const crashed = ref(false)
const appVersion = ref('')
const playback = usePlaybackStore()

interface NavItem {
  to: string
  label: string
  icon: string
  active: boolean
}

const navItems = computed<NavItem[]>(() => {
  const p = route.path
  return [
    {
      to: '/',
      label: 'Library',
      icon: 'library_music',
      active: p === '/' || p.startsWith('/doc') || p.startsWith('/arrange'),
    },
    { to: '/test', label: 'Test', icon: 'quiz', active: p.startsWith('/test') },
    {
      to: '/settings',
      label: 'Settings',
      icon: 'settings',
      active: p.startsWith('/settings'),
    },
  ]
})

/** 是否处于演奏相关页面（顶栏紧急按钮演奏中高亮） */
const performing = computed(() =>
  ['Playing', 'Paused', 'Countdown'].includes(playback.state),
)

/** 页面背景图：与 stitch 模板各页面对应（本地打包，离线可用） */
const bgImage = computed(() => {
  const p = route.path
  if (p.startsWith('/arrange')) return '/bg-arrange.webp'
  if (p.startsWith('/settings')) return '/bg-settings.webp'
  if (p.startsWith('/doc')) return '/bg-library2.webp'
  if (p.startsWith('/test')) return '/bg-library3.webp'
  if (p === '/') return '/bg-library.webp'
  return null // risk 页模板为纯网格背景
})

async function onEmergencyStop(): Promise<void> {
  try {
    await emergencyStop()
  } catch {
    // 后端错误由调用方展示；此处静默
  }
}

onMounted(async () => {
  try {
    crashed.value = await getCrashFlag()
  } catch {
    crashed.value = false
  }
  try {
    appVersion.value = await getVersion()
  } catch {
    appVersion.value = '0.3.1'
  }
})
</script>

<template>
  <div class="flex min-h-screen flex-col bg-background text-on-background font-body">
    <!-- 桌面顶栏 -->
    <header
      v-if="route.name !== 'risk'"
      class="fixed top-0 z-50 hidden h-16 w-full items-center justify-between border-b border-outline-variant bg-surface-container/80 px-gutter-desktop backdrop-blur-md md:flex"
    >
      <div class="flex items-center gap-8">
        <RouterLink to="/" class="flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[26px]"
            >graphic_eq</span
          >
          <span class="font-display text-[24px] font-bold tracking-tight text-primary"
            >ScoreLeap (谱跃)</span
          >
        </RouterLink>
        <nav class="flex gap-6">
          <RouterLink
            v-for="item in navItems"
            :key="item.to"
            :to="item.to"
            class="font-label-caps text-label-caps pb-1 transition-colors"
            :class="
              item.active
                ? 'border-b-2 border-primary text-primary'
                : 'text-on-surface-variant hover:text-primary'
            "
          >
            {{ item.label }}
          </RouterLink>
        </nav>
      </div>
      <div class="flex items-center gap-4">
        <button
          type="button"
          class="flex items-center gap-2 rounded bg-primary-container px-4 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed"
          :class="performing ? 'animate-pulse' : ''"
          @click="onEmergencyStop"
        >
          <span class="material-symbols-outlined text-[16px]">emergency</span>
          Emergency Stop
        </button>
        <span
          class="hidden rounded border border-outline-variant bg-surface-container-high px-2 py-1 font-code-sm text-code-sm text-on-surface-variant lg:inline"
          >v{{ appVersion || '0.3.1' }}</span
        >
      </div>
    </header>

    <!-- 页面背景图（模板素材，本地打包） -->
    <div
      v-if="bgImage"
      class="pointer-events-none fixed inset-0 z-[-1] bg-cover bg-center opacity-30"
      :style="{ backgroundImage: `url(${bgImage})` }"
    ></div>

    <!-- 崩溃提示 -->
    <div
      v-if="crashed"
      class="fixed top-16 z-40 w-full border-b border-error/40 bg-error-container/20 px-4 py-2 text-center text-sm text-error"
    >
      ⚠ 检测到上次会话异常退出：若游戏内按键卡住，请重启游戏；演奏前建议先暂停并重新聚焦游戏窗口。
    </div>

    <!-- 主内容 -->
    <main
      class="mx-auto w-full max-w-[1280px] flex-1 px-gutter-mobile pb-24 pt-20 md:px-gutter-desktop md:pb-12"
      :class="route.name === 'risk' ? 'p-0 md:p-0 max-w-none' : ''"
    >
      <RouterView />
    </main>

    <!-- 移动端底栏导航 -->
    <nav
      v-if="route.name !== 'risk'"
      class="fixed bottom-0 left-0 right-0 z-50 flex h-16 items-center justify-around border-t border-outline-variant bg-surface md:hidden"
    >
      <RouterLink
        v-for="item in navItems"
        :key="item.to"
        :to="item.to"
        class="flex flex-col items-center gap-0.5"
        :class="item.active ? 'text-primary' : 'text-on-surface-variant'"
      >
        <span class="material-symbols-outlined" :style="item.active ? 'font-variation-settings: \'FILL\' 1' : ''">{{
          item.icon
        }}</span>
        <span class="font-label-caps text-[10px]">{{ item.label }}</span>
      </RouterLink>
      <button
        type="button"
        class="flex flex-col items-center gap-0.5 text-error"
        @click="onEmergencyStop"
      >
        <span class="material-symbols-outlined">emergency</span>
        <span class="font-label-caps text-[10px]">STOP</span>
      </button>
    </nav>
  </div>
</template>
