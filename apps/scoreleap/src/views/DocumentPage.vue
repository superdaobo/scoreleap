<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { formatDuration } from '../utils/format'

const route = useRoute()
const store = useLibraryStore()
const loading = ref(true)

const docId = computed(() => String(route.params.docId ?? ''))
const doc = computed(
  () => store.documents.find((d) => d.doc_id === docId.value) ?? null,
)
const enabledCount = computed(() => store.enabledTrackIds(docId.value).length)
const allDisabled = computed(
  () => store.tracks.length > 0 && enabledCount.value === 0,
)

onMounted(async () => {
  loading.value = true
  try {
    if (docId.value) await store.selectDocument(docId.value)
  } catch {
    // 错误已写入 store.error
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div>
    <RouterLink to="/" class="text-sm text-indigo-400 hover:text-indigo-300"
      >← 返回曲谱库</RouterLink
    >

    <div v-if="loading" class="py-20 text-center text-slate-500">加载中…</div>

    <div v-else-if="!doc" class="py-20 text-center">
      <p class="text-slate-400">未找到曲谱信息，可能已被移除。</p>
      <RouterLink
        to="/"
        class="mt-4 inline-block rounded-lg bg-indigo-500 px-4 py-2 text-sm text-white"
        >返回曲谱库</RouterLink
      >
    </div>

    <template v-else>
      <div
        v-if="store.error"
        class="mt-4 flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        <span>⚠️</span><span class="flex-1">{{ store.error }}</span>
      </div>

      <div class="mt-4 grid gap-6 lg:grid-cols-[1fr_320px]">
        <!-- 基本信息 + 轨道列表 -->
        <section
          class="rounded-xl border border-slate-800 bg-slate-800/40 p-6"
        >
          <h1 class="text-xl font-bold text-white">{{ doc.name }}</h1>
          <dl class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div>
              <dt class="text-xs text-slate-500">格式</dt>
              <dd class="mt-1 font-mono text-sm text-slate-200">
                {{ doc.format }}
              </dd>
            </div>
            <div>
              <dt class="text-xs text-slate-500">BPM 范围</dt>
              <dd class="mt-1 font-mono text-sm text-slate-200">
                {{ doc.bpm_range[0].toFixed(0) }} – {{ doc.bpm_range[1].toFixed(0) }}
              </dd>
            </div>
            <div>
              <dt class="text-xs text-slate-500">音符数</dt>
              <dd class="mt-1 font-mono text-sm text-slate-200">
                {{ doc.note_count.toLocaleString() }}
              </dd>
            </div>
            <div>
              <dt class="text-xs text-slate-500">时长</dt>
              <dd class="mt-1 font-mono text-sm text-slate-200">
                {{ formatDuration(doc.duration_ms) }}
              </dd>
            </div>
          </dl>

          <div class="mt-6">
            <div class="mb-3 flex items-center justify-between">
              <h2 class="text-sm font-medium text-slate-300">
                轨道（{{ store.tracks.length }}）
              </h2>
              <div class="flex gap-2 text-xs">
                <button
                  type="button"
                  class="rounded px-2 py-1 text-slate-400 hover:bg-slate-700 hover:text-slate-200"
                  @click="store.setAllTracks(docId, true)"
                >
                  全选
                </button>
                <button
                  type="button"
                  class="rounded px-2 py-1 text-slate-400 hover:bg-slate-700 hover:text-slate-200"
                  @click="store.setAllTracks(docId, false)"
                >
                  全不选
                </button>
              </div>
            </div>
            <ul class="space-y-2">
              <li
                v-for="track in store.tracks"
                :key="track.id"
                class="flex items-center gap-3 rounded-lg border border-slate-800 bg-slate-900/60 px-4 py-2.5"
              >
                <input
                  type="checkbox"
                  class="h-4 w-4 accent-indigo-500"
                  :checked="store.enabledTrackIds(docId).includes(track.id)"
                  @change="store.toggleTrack(docId, track.id)"
                />
                <span class="flex-1 truncate text-sm text-slate-200">
                  {{ track.name || `轨道 ${track.id + 1}` }}
                </span>
                <span class="text-xs text-slate-500"
                  >{{ track.note_count.toLocaleString() }} 音符</span
                >
              </li>
            </ul>
          </div>
        </section>

        <!-- 进入编排 -->
        <aside
          class="h-fit rounded-xl border border-slate-800 bg-slate-800/40 p-6"
        >
          <h2 class="text-sm font-medium text-slate-300">编排</h2>
          <p class="mt-2 text-sm text-slate-400">
            已启用 <strong class="text-indigo-300">{{ enabledCount }}</strong> /
            {{ store.tracks.length }} 条轨道
          </p>
          <p
            v-if="allDisabled"
            class="mt-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-300"
          >
            至少启用一条轨道才能进入编排。
          </p>
          <button
            v-if="allDisabled"
            type="button"
            disabled
            class="mt-4 w-full rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-4 py-2.5 text-center font-medium text-white opacity-40"
          >
            进入编排 →
          </button>
          <RouterLink
            v-else
            :to="{
              name: 'arrange',
              params: { seqId: 'pending' },
              query: { docId },
            }"
            class="mt-4 block rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-4 py-2.5 text-center font-medium text-white hover:opacity-90"
          >
            进入编排 →
          </RouterLink>
        </aside>
      </div>
    </template>
  </div>
</template>
