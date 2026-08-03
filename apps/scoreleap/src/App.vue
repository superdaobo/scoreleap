<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

interface NavItem {
  to: string
  label: string
  active: boolean
}

const navItems = computed<NavItem[]>(() => {
  const p = route.path
  return [
    {
      to: '/',
      label: '曲谱库',
      active: p === '/' || p.startsWith('/doc') || p.startsWith('/arrange'),
    },
    { to: '/test', label: '按键测试', active: p.startsWith('/test') },
    { to: '/settings', label: '设置', active: p.startsWith('/settings') },
  ]
})
</script>

<template>
  <div class="flex min-h-screen flex-col bg-slate-900 text-slate-200">
    <header
      class="sticky top-0 z-20 border-b border-slate-800 bg-slate-900/90 backdrop-blur"
    >
      <nav class="mx-auto flex h-14 max-w-6xl items-center gap-2 px-4">
        <RouterLink
          to="/"
          class="mr-4 flex items-center gap-2 text-lg font-bold text-white"
        >
          <span
            class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-violet-600 text-sm"
            >♪</span
          >
          <span>ScoreLeap 谱跃</span>
        </RouterLink>
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          :to="item.to"
          class="rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
          :class="
            item.active
              ? 'bg-indigo-500/15 text-indigo-300'
              : 'text-slate-400 hover:bg-slate-800 hover:text-slate-200'
          "
        >
          {{ item.label }}
        </RouterLink>
      </nav>
    </header>
    <main class="mx-auto w-full max-w-6xl flex-1 px-4 py-6">
      <RouterView />
    </main>
  </div>
</template>
