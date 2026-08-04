<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { useTranscriptionStore } from '../stores/transcriptionStore'
import { pickAudioFile, pickMidiFile } from '../services/api'
import { formatDuration } from '../utils/format'

const router = useRouter()
const store = useLibraryStore()
const txStore = useTranscriptionStore()
const importing = ref(false)
const dragging = ref(false)

const hasDocuments = computed(() => store.documents.length > 0)

// 进入页面时从后端持久化曲谱库加载，并订阅转录事件
onMounted(() => {
  void store.loadDocuments()
  void txStore.subscribe()
  void txStore.restore()
})

async function chooseFile(): Promise<void> {
  if (importing.value) return
  importing.value = true
  try {
    const path = await pickMidiFile()
    if (!path) return
    const summary = await store.importFile(path)
    await router.push({ name: 'document', params: { docId: summary.doc_id } })
  } catch {
    // 错误信息已写入 store.error，由错误提示条展示
  } finally {
    importing.value = false
  }
}

/** 从音频转录：选择 MP3 → 启动转录（进度卡片显示；完成后自动刷新曲谱库） */
async function chooseAudio(): Promise<void> {
  if (txStore.starting || txStore.running) return
  try {
    const path = await pickAudioFile()
    if (!path) return
    await txStore.start(path)
  } catch {
    // 错误已写入 txStore.error，由错误提示条展示
  }
}

function onDragEnter(e: DragEvent): void {
  e.preventDefault()
  dragging.value = true
}

function onDragOver(e: DragEvent): void {
  e.preventDefault()
}

function onDragLeave(e: DragEvent): void {
  e.preventDefault()
  dragging.value = false
}

function onDrop(e: DragEvent): void {
  e.preventDefault()
  dragging.value = false
  // Tauri 2 中拖拽路径需通过事件获取；当前版本拖拽区仅作视觉引导
  store.error = '请点击「选择 MIDI 文件」按钮进行导入（拖拽导入将在后续版本支持）'
}
</script>

<template>
  <div>
    <h1 class="text-2xl font-bold text-white">曲谱库</h1>
    <p class="mt-1 text-sm text-slate-400">
      导入 MIDI 文件，转换为游戏乐器可演奏的编排。
    </p>

    <!-- 错误提示条 -->
    <div
      v-if="store.error || txStore.error"
      class="mt-4 flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
    >
      <span>⚠️</span>
      <span class="flex-1">{{ store.error || txStore.error }}</span>
      <button
        type="button"
        class="text-red-400 hover:text-red-200"
        @click="store.error = null; txStore.clearError()"
      >
        ✕
      </button>
    </div>

    <!-- 转录进度卡片 -->
    <div
      v-if="txStore.job"
      class="mt-4 rounded-xl border border-slate-700 bg-slate-800/40 px-4 py-3"
    >
      <div class="flex items-center gap-3">
        <span class="text-xl">🎧</span>
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium text-slate-200">
            {{ txStore.job.source_name }}
            <span
              class="ml-2 rounded bg-slate-700/60 px-1.5 py-0.5 text-xs text-slate-300"
              >{{ txStore.job.status }}</span
            >
          </p>
          <p class="mt-0.5 text-xs text-slate-400">
            {{ txStore.stageLabel }}
            <span v-if="txStore.job.note_count != null" class="text-slate-500"
              >· {{ txStore.job.note_count.toLocaleString() }} 音符</span
            >
          </p>
          <div
            v-if="txStore.running"
            class="mt-2 h-1 overflow-hidden rounded-full bg-slate-700"
          >
            <div
              class="h-full rounded-full bg-gradient-to-r from-indigo-500 to-violet-500 transition-all"
              :class="txStore.indeterminate ? 'w-1/3 animate-pulse' : 'w-2/3'"
            ></div>
          </div>
        </div>
        <button
          v-if="txStore.running"
          type="button"
          class="rounded-lg border border-slate-600 px-3 py-1.5 text-xs text-slate-300 hover:border-red-500/50 hover:text-red-300"
          @click="txStore.cancel()"
        >
          取消
        </button>
        <span v-else-if="txStore.job.status === 'Completed'" class="text-green-400"
          >✓ 已导入</span
        >
        <span
          v-else-if="txStore.job.status === 'Failed'"
          class="text-xs text-red-400"
          >{{ txStore.job.error_message || '转录失败' }}</span
        >
      </div>
    </div>

    <!-- 空状态：拖拽区 + 选择按钮 -->
    <div v-if="!hasDocuments" class="mt-8">
      <div
        class="flex flex-col items-center justify-center rounded-2xl border-2 border-dashed px-6 py-16 text-center transition-colors"
        :class="
          dragging
            ? 'border-indigo-400 bg-indigo-500/10'
            : 'border-slate-700 bg-slate-800/30 hover:border-slate-500'
        "
        @dragenter="onDragEnter"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <div class="text-5xl">🎼</div>
        <p class="mt-4 text-lg font-medium text-slate-200">
          将 MIDI 文件拖到这里
        </p>
        <p class="mt-1 text-sm text-slate-500">
          或使用下方按钮选择文件（.mid / .midi）
        </p>
        <button
          type="button"
          class="mt-6 rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-6 py-2.5 font-medium text-white hover:opacity-90 disabled:opacity-50"
          :disabled="importing"
          @click="chooseFile"
        >
          {{ importing ? '导入中…' : '选择 MIDI 文件' }}
        </button>
        <button
          type="button"
          class="mt-3 rounded-lg border border-indigo-500/40 px-6 py-2.5 font-medium text-indigo-300 hover:bg-indigo-500/10 disabled:opacity-50"
          :disabled="txStore.starting || txStore.running"
          @click="chooseAudio"
        >
          🎧 从音频转录（实验）
        </button>
        <p class="mt-3 max-w-md text-xs text-slate-500">
          选择 MP3，自动识别音符并生成可演奏的 MIDI 曲谱。转录在本地完成，不上传任何数据。
        </p>
      </div>
    </div>

    <!-- 已导入列表 -->
    <div v-else class="mt-6">
      <div class="mb-4 flex items-center justify-between gap-2">
        <p class="text-sm text-slate-400">共 {{ store.documents.length }} 首曲谱</p>
        <div class="flex gap-2">
          <button
            type="button"
            class="rounded-lg border border-indigo-500/40 px-3 py-2 text-sm font-medium text-indigo-300 hover:bg-indigo-500/10 disabled:opacity-50"
            :disabled="txStore.starting || txStore.running"
            @click="chooseAudio"
          >
            🎧 从音频转录（实验）
          </button>
          <button
            type="button"
            class="rounded-lg bg-indigo-500/15 px-4 py-2 text-sm font-medium text-indigo-300 hover:bg-indigo-500/25 disabled:opacity-50"
            :disabled="importing"
            @click="chooseFile"
          >
            {{ importing ? '导入中…' : '+ 导入 MIDI' }}
          </button>
        </div>
      </div>
      <ul class="space-y-2">
        <li v-for="doc in store.documents" :key="doc.doc_id">
          <RouterLink
            :to="{ name: 'document', params: { docId: doc.doc_id } }"
            class="flex items-center gap-4 rounded-xl border border-slate-800 bg-slate-800/40 px-4 py-3 transition-colors hover:border-indigo-500/40 hover:bg-slate-800"
          >
            <span class="text-xl">{{ doc.source_type === 'audio_transcription' ? '🎧' : '🎵' }}</span>
            <div class="min-w-0 flex-1">
              <p class="truncate font-medium text-slate-100">
                {{ doc.name }}
                <span
                  v-if="doc.source_type === 'audio_transcription'"
                  class="ml-2 rounded bg-violet-500/20 px-1.5 py-0.5 align-middle text-xs text-violet-300"
                  >转录</span
                >
              </p>
              <p class="text-xs text-slate-500">
                {{ doc.format }} · {{ doc.bpm_range[0].toFixed(0) }}–{{
                  doc.bpm_range[1].toFixed(0)
                }}
                BPM
              </p>
            </div>
            <div class="hidden text-right text-sm text-slate-400 sm:block">
              <p>{{ doc.track_count }} 轨道</p>
              <p class="text-xs text-slate-500">
                {{ doc.note_count.toLocaleString() }} 音符
              </p>
            </div>
            <span
              class="w-16 text-right text-sm tabular-nums text-slate-400"
              >{{ formatDuration(doc.duration_ms) }}</span
            >
            <span class="text-slate-600">›</span>
          </RouterLink>
        </li>
      </ul>
    </div>
  </div>
</template>
