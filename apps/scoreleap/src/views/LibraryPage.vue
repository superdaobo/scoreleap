<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { useTranscriptionStore } from '../stores/transcriptionStore'
import { useModelStore } from '../stores/modelStore'
import { getAudioFileInfo, pickAudioFile, pickMidiFile } from '../services/api'
import { formatDuration } from '../utils/format'

const router = useRouter()
const store = useLibraryStore()
const txStore = useTranscriptionStore()
const modelStore = useModelStore()
const importing = ref(false)
const dragging = ref(false)

const hasDocuments = computed(() => store.documents.length > 0)

// 转录命令只负责启动异步任务；真正完成导入后再刷新曲谱库。
watch(
  () => txStore.job?.status,
  (status, previous) => {
    if (status === 'Completed' && previous !== 'Completed') void store.loadDocuments()
  },
)

// 进入页面时从后端持久化曲谱库加载，并订阅转录事件
onMounted(() => {
  void store.loadDocuments()
  void txStore.subscribe()
  void txStore.restore()
  void modelStore.subscribe()
  void modelStore.load()
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

/** 从音频转录：选择受支持格式 → 确认界面 → 启动。音频始终只传本地路径。 */
async function chooseAudio(): Promise<void> {
  if (txStore.starting || txStore.running) return
  try {
    const path = await pickAudioFile()
    if (!path) return
    const info = await getAudioFileInfo(path)
    txStore.askConfirm(path, info.name, info.size_bytes)
  } catch {
    // 错误已写入 txStore.error，由错误提示条展示
  }
}

/** 确认后启动转录 */
async function confirmStart(): Promise<void> {
  const p = txStore.pendingConfirm
  if (!p) return
  if (!modelStore.ready) {
    txStore.error = '首次转录前需要在设置中确认并下载模型'
    await router.push({ name: 'settings' })
    return
  }
  await txStore.start(p.path)
}

/** 完成态：跳转曲谱详情 */
function goToDoc(docId: string | null): void {
  if (docId) void router.push({ name: 'document', params: { docId } })
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
    <!-- Banner：错误提示条 -->
    <div
      v-if="store.error || txStore.error"
      class="mb-4 flex items-center gap-3 border border-error bg-error-container/20 px-4 py-3 text-error"
    >
      <span class="material-symbols-outlined">error</span>
      <span class="flex-1 font-code-sm text-code-sm">{{ store.error || txStore.error }}</span>
      <button
        type="button"
        class="text-error hover:opacity-80"
        @click="store.error = null; txStore.clearError()"
      >
        <span class="material-symbols-outlined text-[18px]">close</span>
      </button>
    </div>

    <!-- Banner：模型未装提示 -->
    <div
      v-if="!modelStore.ready && modelStore.model.status !== 'unknown'"
      class="mb-4 flex flex-wrap items-center justify-between gap-3 border border-surface-tint bg-surface-tint/10 px-4 py-3 text-primary-fixed"
      role="status"
    >
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-primary-container">update</span>
        <div>
          <p class="font-code-sm text-code-sm">
            首次音频转录需要按需下载模型
          </p>
          <p class="text-xs text-on-surface-variant">
            下载前会显示版本、大小和来源并等待你确认；音频不会上传。
          </p>
        </div>
      </div>
      <button
        type="button"
        class="border border-primary/40 px-3 py-2 font-code-sm text-code-sm text-primary hover:bg-primary/10"
        @click="router.push({ name: 'settings' })"
      >前往设置</button>
    </div>

    <!-- Bento 网格 -->
    <div class="grid grid-cols-12 gap-6">
      <!-- Hero：品牌 + 导入入口 -->
      <section
        class="bento-item relative col-span-12 flex flex-col items-center gap-8 overflow-hidden rounded-xl p-8 md:col-span-8 md:flex-row"
      >
        <div
          class="absolute -right-20 -top-20 h-64 w-64 rounded-full bg-primary-container/10 blur-3xl"
        ></div>
        <div
          class="flex h-32 w-32 shrink-0 items-center justify-center rounded-lg border border-outline-variant bg-surface-container-lowest"
        >
          <span class="material-symbols-outlined text-primary text-[64px]">graphic_eq</span>
        </div>
        <div class="z-10 flex flex-col gap-4">
          <h1 class="font-display text-display-lg-mobile text-primary md:text-display-lg">
            ScoreLeap
          </h1>
          <p class="max-w-xl text-on-surface-variant">
            Advanced AI-driven music transcription and MIDI generation library.
            管理你的曲谱、监控实时转录过程，并以精确的编排组织你的音乐数据。
          </p>
          <div class="mt-4 flex flex-wrap gap-4">
            <button
              type="button"
              class="flex items-center gap-2 rounded bg-primary-container px-6 py-3 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
              :disabled="importing"
              @click="chooseFile"
            >
              <span class="material-symbols-outlined text-[18px]">upload_file</span>
              {{ importing ? '导入中…' : '选择 MIDI 文件' }}
            </button>
            <button
              type="button"
              class="flex items-center gap-2 rounded border border-outline-variant px-6 py-3 font-label-caps text-label-caps text-on-surface transition-colors hover:border-primary-container hover:text-primary-container disabled:opacity-50"
              :disabled="txStore.starting || txStore.running"
              @click="chooseAudio"
            >
              <span class="material-symbols-outlined text-[18px]">mic</span>
              从音频转录
            </button>
          </div>
          <p class="text-xs text-on-surface-variant">
            选择 MP3、WAV 或 FLAC，自动识别音符并生成 MIDI。音频始终在本地处理。
          </p>
        </div>
      </section>

      <!-- 转录进度卡 -->
      <section
        v-if="txStore.job"
        class="bento-item col-span-12 flex flex-col justify-center gap-4 rounded-xl p-6 md:col-span-4"
      >
        <div class="mb-2 flex items-center gap-3">
          <span class="material-symbols-outlined animate-pulse text-primary-container text-[28px]"
            >headphones</span
          >
          <div class="min-w-0">
            <h3 class="font-code-sm text-code-sm uppercase text-primary">
              Active Process
              <span
                class="ml-2 rounded border border-outline-variant bg-surface px-1.5 py-0.5 text-on-surface-variant"
                >{{ txStore.job.status }}</span
              >
            </h3>
            <p class="truncate text-on-surface">{{ txStore.job.source_name }}</p>
            <p class="text-xs text-on-surface-variant">
              {{ txStore.stageLabel }}
              <span v-if="txStore.job.note_count != null" class="text-on-surface-variant"
                >· {{ txStore.job.note_count.toLocaleString() }} 音符</span
              >
            </p>
          </div>
        </div>
        <div v-if="txStore.running" class="mb-1 h-2 w-full overflow-hidden rounded-full bg-surface-container-high">
          <div
            class="h-2 rounded-full bg-primary-container transition-all"
            :class="txStore.indeterminate ? 'w-1/3 animate-pulse' : 'w-2/3'"
          ></div>
        </div>
        <div class="flex items-center justify-between">
          <button
            v-if="txStore.running"
            type="button"
            class="rounded border border-outline-variant px-3 py-1.5 font-code-sm text-code-sm text-on-surface-variant transition-colors hover:border-error hover:text-error"
            @click="txStore.cancel()"
          >取消</button>
          <span
            v-else-if="txStore.job.status === 'Completed'"
            class="flex items-center gap-2 font-code-sm text-code-sm"
          >
            <span class="text-secondary">✓ 已导入</span>
            <button
              type="button"
              class="rounded bg-primary-container px-3 py-1.5 font-code-sm text-code-sm text-on-primary-container hover:bg-primary-fixed"
              @click="goToDoc(txStore.job.result_doc_id)"
            >查看曲谱</button>
          </span>
          <span
            v-else-if="txStore.job.status === 'Failed'"
            class="max-w-[200px] font-code-sm text-code-sm text-error"
            >{{ txStore.errorLabel(txStore.job.error_code, txStore.job.error_message || '转录失败') }}</span
          >
        </div>
      </section>

      <!-- 曲谱库标题 -->
      <div
        class="col-span-12 mt-8 flex items-end justify-between border-b border-outline-variant pb-4"
      >
        <h2 class="font-display text-headline-md text-primary">曲谱库 (Library)</h2>
        <div class="flex items-center gap-3">
          <span
            v-if="hasDocuments"
            class="rounded border border-outline-variant bg-surface-container-high px-2 py-1 font-code-sm text-code-sm text-on-surface-variant"
            >Total: {{ store.documents.length }} Items</span>
          <button
            v-if="hasDocuments"
            type="button"
            class="flex items-center gap-2 rounded border border-outline-variant px-3 py-2 font-label-caps text-label-caps text-on-surface transition-colors hover:border-primary hover:text-primary disabled:opacity-50"
            :disabled="txStore.starting || txStore.running"
            @click="chooseAudio"
          >
            <span class="material-symbols-outlined text-[16px]">mic</span>
            从音频转录
          </button>
          <button
            v-if="hasDocuments"
            type="button"
            class="flex items-center gap-2 rounded bg-primary-container px-3 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
            :disabled="importing"
            @click="chooseFile"
          >
            <span class="material-symbols-outlined text-[16px]">add</span>
            {{ importing ? '导入中…' : '导入 MIDI' }}
          </button>
        </div>
      </div>

      <!-- 空态 -->
      <div v-if="!hasDocuments" class="col-span-12 mt-2">
        <div
          class="flex flex-col items-center justify-center border-2 border-dashed border-outline-variant px-6 py-16 text-center transition-colors"
          :class="dragging ? 'border-primary bg-primary/10' : 'hover:border-surface-tint'"
          @dragenter="onDragEnter"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
        >
          <span class="material-symbols-outlined text-primary text-[56px]"
            >music_note</span
          >
          <p class="mt-4 font-display text-[22px] text-on-surface">
            将 MIDI 文件拖到这里
          </p>
          <p class="mt-1 text-sm text-on-surface-variant">
            或使用上方按钮选择文件（.mid / .midi）
          </p>
        </div>
      </div>

      <!-- 曲谱列表 -->
      <template v-else>
        <article
          v-for="doc in store.documents"
          :key="doc.doc_id"
          class="bento-item col-span-12 flex flex-col gap-4 rounded-lg p-5 md:col-span-6 lg:col-span-4"
        >
          <RouterLink :to="{ name: 'document', params: { docId: doc.doc_id } }">
            <div class="flex items-start justify-between">
              <div
                class="rounded border border-outline-variant bg-surface-container-highest p-2"
                :class="doc.source_type === 'audio_transcription' ? 'text-secondary' : 'text-primary'"
              >
                <span class="material-symbols-outlined">{{
                  doc.source_type === 'audio_transcription' ? 'headphones' : 'music_note'
                }}</span>
              </div>
              <span
                class="rounded border border-outline-variant bg-surface px-2 py-1 font-code-sm text-code-sm text-on-surface-variant"
                >{{ doc.source_type === 'audio_transcription' ? 'AUDIO' : 'MIDI' }}</span
              >
            </div>
            <div class="mt-3">
              <h4 class="mb-1 truncate font-body-lg text-body-lg text-on-surface">
                {{ doc.name }}
              </h4>
              <p class="font-code-sm text-code-sm text-on-surface-variant">
                {{ doc.format }}
              </p>
            </div>
            <div
              class="mt-2 grid grid-cols-3 gap-2 border-t border-outline-variant/50 pt-4"
            >
              <div class="flex flex-col">
                <span class="font-label-caps text-label-caps text-on-surface-variant"
                  >BPM</span
                >
                <span class="font-code-sm text-code-sm text-on-surface"
                  >{{ doc.bpm_range[0].toFixed(0) }}–{{ doc.bpm_range[1].toFixed(0) }}</span
                >
              </div>
              <div class="flex flex-col">
                <span class="font-label-caps text-label-caps text-on-surface-variant"
                  >TRACKS</span
                >
                <span class="font-code-sm text-code-sm text-on-surface"
                  >{{ doc.track_count }}</span
                >
              </div>
              <div class="flex flex-col">
                <span class="font-label-caps text-label-caps text-on-surface-variant"
                  >DUR</span
                >
                <span class="font-code-sm text-code-sm text-on-surface"
                  >{{ formatDuration(doc.duration_ms) }}</span
                >
              </div>
            </div>
          </RouterLink>
        </article>
      </template>
    </div>

    <!-- 转录确认弹窗 -->
    <div
      v-if="txStore.pendingConfirm"
      class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      @click.self="txStore.dismissConfirm()"
    >
      <div
        class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
      >
        <h2 class="font-display text-[24px] text-on-surface">确认转录</h2>
        <p class="mt-1 truncate text-sm text-on-surface">{{ txStore.pendingConfirm.name }}</p>
        <p class="mt-1 font-code-sm text-code-sm text-on-surface-variant">
          {{ (txStore.pendingConfirm.size_bytes / 1024 / 1024).toFixed(1) }} MB
        </p>
        <ul class="mt-4 space-y-2 text-sm text-on-surface-variant">
          <li>• 音频仅在本地处理，不会上传或联网。</li>
          <li>• 预计耗时：首次约 30–60 秒（含模型加载），之后约 10 秒。</li>
          <li>• 钢琴独奏或旋律清晰的音频效果最佳；完整歌曲可能出现杂音符。</li>
          <li>• 支持 MP3/WAV/FLAC（≤200MB，≤10 分钟）。</li>
          <li>
            • 当前引擎：{{ txStore.engine === 'high_quality' ? '高质量钢琴（Transkun v2）' : '快速（Basic Pitch）' }}。
          </li>
          <li v-if="txStore.engine === 'fast'">
            • 当前预设：{{ txStore.preset === 'balanced' ? '均衡' : txStore.preset === 'detail' ? '细节' : '降噪' }}（可在设置中调整）。
          </li>
        </ul>
        <div class="mt-6 flex justify-end gap-3">
          <button
            type="button"
            class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
            @click="txStore.dismissConfirm()"
          >取消</button>
          <button
            type="button"
            class="flex items-center gap-2 rounded bg-primary-container px-5 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
            :disabled="txStore.starting || !modelStore.ready"
            @click="confirmStart"
          >
            {{ !modelStore.ready ? '请先下载模型' : txStore.starting ? '启动中…' : '开始转录' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
