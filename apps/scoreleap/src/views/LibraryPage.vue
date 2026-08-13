<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { useTranscriptionStore } from '../stores/transcriptionStore'
import { useModelStore } from '../stores/modelStore'
import { getAudioFileInfo, pickAudioFile, pickMidiFile } from '../services/api'
import { formatDuration } from '../utils/format'
import LibraryDocModals from '../components/LibraryDocModals.vue'
import LibraryGroupModals from '../components/LibraryGroupModals.vue'
import type { DocumentSummary, GroupInfo } from '../types'

const router = useRouter()
const store = useLibraryStore()
const txStore = useTranscriptionStore()
const modelStore = useModelStore()
const importing = ref(false)
const dragging = ref(false)

/** 当前视图：全部 / 未分类 / 分组 id */
const activeView = ref<'all' | 'ungrouped' | string>('all')

const hasDocuments = computed(() => store.documents.length > 0)

/** 当前视图内的曲谱（按全局顺序过滤） */
const viewDocs = computed<DocumentSummary[]>(() => {
  if (activeView.value === 'all') return store.documents
  if (activeView.value === 'ungrouped') {
    return store.documents.filter((d) => d.group_id === null)
  }
  return store.documents.filter((d) => d.group_id === activeView.value)
})

const ungroupedCount = computed(
  () => store.documents.filter((d) => d.group_id === null).length,
)

function groupCount(groupId: string): number {
  return store.documents.filter((d) => d.group_id === groupId).length
}

// 转录命令只负责启动异步任务；真正完成导入后再刷新曲谱库。
watch(
  () => txStore.job?.status,
  (status, previous) => {
    if (status === 'Completed' && previous !== 'Completed') void store.loadDocuments()
  },
)

// 进入页面时从后端持久化曲谱库加载，并订阅转录事件
onMounted(() => {
  void store.loadAll()
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
    if (summary.duplicated) return
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

/** 完成态：跳转曲谱详情（去重跳过时 doc_id 为空，不跳转） */
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

// ---------------------------------------------------------------------------
// 分组管理（弹窗交互在 LibraryGroupModals / LibraryDocModals 子组件）
// ---------------------------------------------------------------------------

const newGroupOpen = ref(false)
/** 待重命名的分组（null = 弹窗关闭） */
const renameGroupTarget = ref<GroupInfo | null>(null)
/** 待删除确认的分组（null = 弹窗关闭） */
const deleteGroupTarget = ref<GroupInfo | null>(null)

function openRenameGroup(groupId: string): void {
  const g = store.groups.find((x) => x.group_id === groupId)
  if (g) renameGroupTarget.value = g
}

function openDeleteGroup(groupId: string): void {
  const g = store.groups.find((x) => x.group_id === groupId)
  if (g) deleteGroupTarget.value = g
}

/** 新建分组成功：切换视图并提示 */
function onGroupCreated(group: GroupInfo): void {
  newGroupOpen.value = false
  activeView.value = group.group_id
  store.notice = `已创建分组「${group.name}」`
}

function onGroupRenamed(): void {
  renameGroupTarget.value = null
  store.notice = '分组已重命名'
}

function onGroupDeleted(): void {
  const t = deleteGroupTarget.value
  if (t && activeView.value === t.group_id) activeView.value = 'all'
  deleteGroupTarget.value = null
  store.notice = t ? `已删除分组「${t.name}」，组内曲谱回到未分类` : '分组已删除'
}

// ---------------------------------------------------------------------------
// 曲谱管理：删除 / 重命名 / 移动分组
// ---------------------------------------------------------------------------

const deleteDocTarget = ref<DocumentSummary | null>(null)
const renameDocTarget = ref<DocumentSummary | null>(null)
const moveDocTarget = ref<DocumentSummary | null>(null)

function openDeleteDoc(doc: DocumentSummary): void {
  deleteDocTarget.value = doc
}

function openRenameDoc(doc: DocumentSummary): void {
  renameDocTarget.value = doc
}

function openMoveDoc(doc: DocumentSummary): void {
  moveDocTarget.value = doc
}

function onDocRenamed(): void {
  renameDocTarget.value = null
  store.notice = '曲谱已重命名'
}

function onDocDeleted(): void {
  const t = deleteDocTarget.value
  deleteDocTarget.value = null
  store.notice = t ? `已删除「${t.name}」` : '曲谱已删除'
}

function onDocMoved(toGroup: boolean): void {
  moveDocTarget.value = null
  store.notice = toGroup ? '已移动到分组' : '已移回未分类'
}

// ---------------------------------------------------------------------------
// 排序：拖拽 + 上移/下移
// ---------------------------------------------------------------------------

const dragDocId = ref<string | null>(null)

function onCardDragStart(docId: string, e: DragEvent): void {
  dragDocId.value = docId
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}

function onCardDragOver(_targetId: string, e: DragEvent): void {
  e.preventDefault()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
}

/** 拖拽放置：把拖拽源移动到目标位置（视图内） */
function onCardDrop(targetId: string): void {
  const from = dragDocId.value
  dragDocId.value = null
  if (!from || from === targetId) return
  const ids = viewDocs.value.map((d) => d.doc_id)
  const fromIdx = ids.indexOf(from)
  const toIdx = ids.indexOf(targetId)
  if (fromIdx < 0 || toIdx < 0) return
  ids.splice(fromIdx, 1)
  ids.splice(toIdx, 0, from)
  void store.reorderInView(ids)
}

/** 上移/下移：交换视图内相邻两项并提交 */
function moveCard(docId: string, offset: -1 | 1): void {
  const ids = viewDocs.value.map((d) => d.doc_id)
  const idx = ids.indexOf(docId)
  const target = idx + offset
  if (idx < 0 || target < 0 || target >= ids.length) return
  ;[ids[idx], ids[target]] = [ids[target], ids[idx]]
  void store.reorderInView(ids)
}
</script>

<template>
  <div>
    <!-- Banner：成功提示 -->
    <div
      v-if="store.notice"
      class="mb-4 flex items-center gap-3 border border-secondary bg-secondary/10 px-4 py-3 text-secondary"
      role="status"
    >
      <span class="material-symbols-outlined">check_circle</span>
      <span class="flex-1 font-code-sm text-code-sm">{{ store.notice }}</span>
      <button
        type="button"
        class="text-secondary hover:opacity-80"
        @click="store.clearNotice()"
      >
        <span class="material-symbols-outlined text-[18px]">close</span>
      </button>
    </div>

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
        @click="store.clearError(); txStore.clearError()"
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
            <span class="text-secondary">✓ {{ txStore.job.result_doc_id ? '已导入' : '已跳过重复' }}</span>
            <button
              v-if="txStore.job.result_doc_id"
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
        <div
          v-if="txStore.job.status === 'Completed' && txStore.job.raw_stats"
          class="mt-3 border-t border-outline-variant/40 pt-2 font-code-sm text-code-sm text-on-surface-variant"
        >
          <p class="text-xs">
            原始输出 {{ txStore.job.raw_stats.raw_note_count.toLocaleString() }} 音符
            （{{ txStore.job.raw_stats.raw_min_pitch ?? '—' }}–{{ txStore.job.raw_stats.raw_max_pitch ?? '—' }}）
          </p>
          <p class="text-xs">
            低音外 {{ txStore.job.raw_stats.low_outside_game }} · 可直接演奏
            {{ txStore.job.raw_stats.directly_playable }} · 高音外
            {{ txStore.job.raw_stats.high_outside_game }}
          </p>
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
            class="flex items-center gap-2 rounded border border-outline-variant px-3 py-2 font-label-caps text-label-caps text-on-surface transition-colors hover:border-primary hover:text-primary"
            @click="newGroupOpen = true"
          >
            <span class="material-symbols-outlined text-[16px]">create_new_folder</span>
            新建分组
          </button>
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

      <!-- 分组导航 -->
      <nav
        v-if="hasDocuments"
        class="col-span-12 flex flex-wrap items-center gap-2"
        aria-label="曲谱库分组"
      >
        <button
          type="button"
          class="flex items-center gap-2 border px-3 py-1.5 font-code-sm text-code-sm transition-colors"
          :class="activeView === 'all'
            ? 'border-primary-container bg-primary-container/15 text-primary-container'
            : 'border-outline-variant text-on-surface-variant hover:border-primary hover:text-primary'"
          @click="activeView = 'all'"
        >
          全部
          <span class="rounded bg-surface-container-high px-1.5 py-0.5 text-xs">{{
            store.documents.length
          }}</span>
        </button>
        <button
          type="button"
          class="flex items-center gap-2 border px-3 py-1.5 font-code-sm text-code-sm transition-colors"
          :class="activeView === 'ungrouped'
            ? 'border-primary-container bg-primary-container/15 text-primary-container'
            : 'border-outline-variant text-on-surface-variant hover:border-primary hover:text-primary'"
          @click="activeView = 'ungrouped'"
        >
          未分类
          <span class="rounded bg-surface-container-high px-1.5 py-0.5 text-xs">{{
            ungroupedCount
          }}</span>
        </button>
        <div
          v-for="group in store.groups"
          :key="group.group_id"
          class="group relative"
        >
          <button
            type="button"
            class="flex items-center gap-2 border px-3 py-1.5 font-code-sm text-code-sm transition-colors"
            :class="activeView === group.group_id
              ? 'border-primary-container bg-primary-container/15 text-primary-container'
              : 'border-outline-variant text-on-surface-variant hover:border-primary hover:text-primary'"
            @click="activeView = group.group_id"
          >
            {{ group.name }}
            <span class="rounded bg-surface-container-high px-1.5 py-0.5 text-xs">{{
              groupCount(group.group_id)
            }}</span>
          </button>
          <div
            class="absolute -top-2 right-1 hidden items-center gap-1 rounded border border-outline-variant bg-surface-container-lowest px-1 py-0.5 shadow-lg shadow-black/50 group-hover:flex"
          >
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-primary"
              title="重命名分组"
              @click="openRenameGroup(group.group_id)"
            >
              <span class="material-symbols-outlined text-[14px]">edit</span>
            </button>
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-error"
              title="删除分组"
              @click="openDeleteGroup(group.group_id)"
            >
              <span class="material-symbols-outlined text-[14px]">close</span>
            </button>
          </div>
        </div>
      </nav>

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

      <!-- 分组空态 -->
      <div v-else-if="viewDocs.length === 0" class="col-span-12 mt-2">
        <div
          class="flex flex-col items-center justify-center border border-dashed border-outline-variant px-6 py-12 text-center"
        >
          <span class="material-symbols-outlined text-on-surface-variant text-[40px]"
            >folder_open</span
          >
          <p class="mt-3 text-sm text-on-surface-variant">
            {{
              activeView === 'ungrouped'
                ? '未分类中暂无曲谱'
                : '该分组暂无曲谱，可通过卡片操作将曲谱移入'
            }}
          </p>
        </div>
      </div>

      <!-- 曲谱列表 -->
      <template v-else>
        <article
          v-for="doc in viewDocs"
          :key="doc.doc_id"
          class="bento-item group relative col-span-12 flex flex-col gap-4 rounded-lg p-5 transition-colors md:col-span-6 lg:col-span-4"
          :class="dragDocId === doc.doc_id ? 'border-primary opacity-60' : ''"
          draggable="true"
          @dragstart="onCardDragStart(doc.doc_id, $event)"
          @dragover="onCardDragOver(doc.doc_id, $event)"
          @drop="onCardDrop(doc.doc_id)"
          @dragend="dragDocId = null"
        >
          <!-- 悬停操作栏 -->
          <div
            class="absolute right-3 top-3 z-10 hidden items-center gap-1 rounded border border-outline-variant bg-surface-container-lowest px-1 py-0.5 shadow-lg shadow-black/50 group-hover:flex"
            @click.stop
          >
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-primary"
              title="上移"
              :disabled="viewDocs[0]?.doc_id === doc.doc_id"
              @click="moveCard(doc.doc_id, -1)"
            >
              <span class="material-symbols-outlined text-[15px]">arrow_upward</span>
            </button>
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-primary"
              title="下移"
              :disabled="viewDocs[viewDocs.length - 1]?.doc_id === doc.doc_id"
              @click="moveCard(doc.doc_id, 1)"
            >
              <span class="material-symbols-outlined text-[15px]">arrow_downward</span>
            </button>
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-primary"
              title="移动到分组"
              @click="openMoveDoc(doc)"
            >
              <span class="material-symbols-outlined text-[15px]">drive_file_move</span>
            </button>
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-primary"
              title="重命名"
              @click="openRenameDoc(doc)"
            >
              <span class="material-symbols-outlined text-[15px]">edit</span>
            </button>
            <button
              type="button"
              class="p-0.5 text-on-surface-variant hover:text-error"
              title="删除"
              @click="openDeleteDoc(doc)"
            >
              <span class="material-symbols-outlined text-[15px]">delete</span>
            </button>
          </div>
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
              <p
                v-if="doc.title || doc.artist"
                class="mt-1 truncate text-xs text-secondary"
              >
                {{ [doc.title, doc.artist].filter(Boolean).join(' — ') }}
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

    <!-- 分组管理弹窗（新建/重命名/删除） -->
    <LibraryGroupModals
      :create-open="newGroupOpen"
      :rename-target="renameGroupTarget"
      :delete-target="deleteGroupTarget"
      @close-create="newGroupOpen = false"
      @close-rename="renameGroupTarget = null"
      @close-delete="deleteGroupTarget = null"
      @created="onGroupCreated"
      @renamed="onGroupRenamed"
      @deleted="onGroupDeleted"
    />

    <!-- 曲谱管理弹窗（重命名/删除/移动） -->
    <LibraryDocModals
      :rename-target="renameDocTarget"
      :delete-target="deleteDocTarget"
      :move-target="moveDocTarget"
      @close-rename="renameDocTarget = null"
      @close-delete="deleteDocTarget = null"
      @close-move="moveDocTarget = null"
      @renamed="onDocRenamed"
      @deleted="onDocDeleted"
      @moved="onDocMoved"
    />

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
          <li>• 若检测到重复曲谱（同一首歌转录两次），将自动跳过导入。</li>
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
