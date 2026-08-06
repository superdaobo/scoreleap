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
    <RouterLink
      to="/"
      class="flex items-center gap-2 text-sm text-on-surface-variant transition-colors hover:text-primary"
    >
      <span class="material-symbols-outlined text-[18px]">arrow_back</span>
      返回曲谱库
    </RouterLink>

    <div v-if="loading" class="py-20 text-center font-code-sm text-code-sm text-on-surface-variant">
      LOADING_SYSTEM...
    </div>

    <div v-else-if="!doc" class="py-20 text-center">
      <p class="text-on-surface-variant">未找到曲谱信息，可能已被移除。</p>
      <RouterLink
        to="/"
        class="mt-4 inline-block rounded bg-primary-container px-4 py-2 font-label-caps text-label-caps text-on-primary-container hover:bg-primary-fixed"
        >返回曲谱库</RouterLink
      >
    </div>

    <template v-else>
      <!-- 错误提示条 -->
      <div
        v-if="store.error"
        class="mt-4 flex items-center gap-3 border border-error bg-error-container/20 px-4 py-3 text-error"
      >
        <span class="material-symbols-outlined">error</span>
        <span class="flex-1 font-code-sm text-code-sm">{{ store.error }}</span>
      </div>

      <div class="mt-4 grid gap-6 lg:grid-cols-[1fr_320px]">
        <!-- 基本信息 + 轨道列表 -->
        <section class="bento-item rounded-xl p-6">
          <h1 class="font-display text-[26px] text-on-surface">{{ doc.name }}</h1>
          <dl class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
            <div class="rounded border border-outline-variant bg-surface p-4">
              <dt class="font-label-caps text-label-caps text-on-surface-variant">FORMAT</dt>
              <dd class="mt-1 font-code-sm text-code-sm text-primary">{{ doc.format }}</dd>
            </div>
            <div class="rounded border border-outline-variant bg-surface p-4">
              <dt class="font-label-caps text-label-caps text-on-surface-variant">BPM RANGE</dt>
              <dd class="mt-1 font-code-sm text-code-sm text-primary">
                {{ doc.bpm_range[0].toFixed(0) }} – {{ doc.bpm_range[1].toFixed(0) }}
              </dd>
            </div>
            <div class="rounded border border-outline-variant bg-surface p-4">
              <dt class="font-label-caps text-label-caps text-on-surface-variant">NOTES</dt>
              <dd class="mt-1 font-code-sm text-code-sm text-primary">
                {{ doc.note_count.toLocaleString() }}
              </dd>
            </div>
            <div class="rounded border border-outline-variant bg-surface p-4">
              <dt class="font-label-caps text-label-caps text-on-surface-variant">DURATION</dt>
              <dd class="mt-1 font-code-sm text-code-sm text-primary">
                {{ formatDuration(doc.duration_ms) }}
              </dd>
            </div>
          </dl>

          <div class="mt-6">
            <div class="mb-3 flex items-center justify-between border-b border-outline-variant pb-3">
              <h2 class="font-body-lg text-body-lg text-primary flex items-center gap-2">
                <span class="material-symbols-outlined text-[20px]">queue_music</span>
                轨道（{{ store.tracks.length }}）
              </h2>
              <div class="flex gap-2 font-code-sm text-code-sm">
                <button
                  type="button"
                  class="px-2 py-1 text-on-surface-variant transition-colors hover:bg-surface-container-highest hover:text-primary"
                  @click="store.setAllTracks(docId, true)"
                >全选</button>
                <button
                  type="button"
                  class="px-2 py-1 text-on-surface-variant transition-colors hover:bg-surface-container-highest hover:text-primary"
                  @click="store.setAllTracks(docId, false)"
                >全不选</button>
              </div>
            </div>
            <ul class="space-y-2">
              <li
                v-for="track in store.tracks"
                :key="track.id"
                class="flex items-center gap-3 rounded border border-outline-variant bg-surface-container-lowest px-4 py-2.5"
              >
                <input
                  type="checkbox"
                  class="tech-checkbox h-4 w-4 cursor-pointer appearance-none border border-outline-variant bg-surface-container-highest"
                  :checked="store.enabledTrackIds(docId).includes(track.id)"
                  @change="store.toggleTrack(docId, track.id)"
                />
                <span class="flex-1 truncate font-code-sm text-code-sm text-on-surface">
                  {{ track.name || `TRACK_${track.id + 1}` }}
                </span>
                <span class="font-code-sm text-code-sm text-on-surface-variant"
                  >{{ track.note_count.toLocaleString() }} notes</span
                >
              </li>
            </ul>
          </div>
        </section>

        <!-- 进入编排 -->
        <aside class="bento-item h-fit rounded-xl p-6">
          <h2 class="font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest">
            Arrangement
          </h2>
          <p class="mt-3 font-code-sm text-code-sm text-on-surface-variant">
            已启用 <strong class="text-primary">{{ enabledCount }}</strong> /
            {{ store.tracks.length }} 条轨道
          </p>
          <p
            v-if="allDisabled"
            class="mt-2 rounded border border-error/40 bg-error-container/20 px-3 py-2 font-code-sm text-code-sm text-error"
          >
            至少启用一条轨道才能进入编排。
          </p>
          <button
            v-if="allDisabled"
            type="button"
            disabled
            class="mt-4 flex w-full items-center justify-center gap-2 rounded bg-surface-variant px-4 py-2.5 font-label-caps text-label-caps text-on-surface-variant opacity-50"
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
            class="mt-4 flex w-full items-center justify-center gap-2 rounded bg-primary-container px-4 py-2.5 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed"
          >
            进入编排 →
          </RouterLink>
        </aside>
      </div>
    </template>
  </div>
</template>
