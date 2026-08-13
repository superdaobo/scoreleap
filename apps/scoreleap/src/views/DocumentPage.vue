<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { formatDateTime, formatDuration } from '../utils/format'

const route = useRoute()
const router = useRouter()
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

/** 引擎标识 → 展示名 */
function engineLabel(engine: string): string {
  if (engine.includes('transkun')) return '高质量引擎（Transkun v2）'
  if (engine.includes('basic-pitch')) return '快速引擎（Basic Pitch）'
  return engine
}

/** 转录耗时：毫秒 → "12.3s" / "1m05s" */
function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const total = Math.round(ms / 1000)
  if (total < 60) return `${total}s`
  return `${Math.floor(total / 60)}m${(total % 60).toString().padStart(2, '0')}s`
}

// ---------------------------------------------------------------------------
// 管理操作：重命名 / 删除 / 编辑曲目信息
// ---------------------------------------------------------------------------

const renameOpen = ref(false)
const renameName = ref('')
const renameError = ref<string | null>(null)

const deleteOpen = ref(false)
const deleteError = ref<string | null>(null)

const pieceOpen = ref(false)
const pieceTitle = ref('')
const pieceArtist = ref('')
const pieceError = ref<string | null>(null)

/** 侧栏移动分组（空串 = 未分类） */
const moveGroupId = ref('')
const moveGroupError = ref<string | null>(null)

async function submitMoveGroup(): Promise<void> {
  moveGroupError.value = null
  try {
    await store.moveToGroup(docId.value, moveGroupId.value || null)
    store.notice = moveGroupId.value ? '已移动到分组' : '已移回未分类'
  } catch (e) {
    moveGroupError.value = e instanceof Error ? e.message : String(e)
  }
}

function openRename(): void {
  if (!doc.value) return
  renameName.value = doc.value.name
  renameError.value = null
  renameOpen.value = true
}

async function submitRename(): Promise<void> {
  renameError.value = null
  try {
    await store.renameDocument(docId.value, renameName.value)
    renameOpen.value = false
    store.notice = '曲谱已重命名'
  } catch (e) {
    renameError.value = e instanceof Error ? e.message : String(e)
  }
}

function openDelete(): void {
  deleteError.value = null
  deleteOpen.value = true
}

async function submitDelete(): Promise<void> {
  deleteError.value = null
  try {
    await store.deleteDocument(docId.value)
    deleteOpen.value = false
    await router.push('/')
    store.notice = '曲谱已删除'
  } catch (e) {
    deleteError.value = e instanceof Error ? e.message : String(e)
  }
}

function openPiece(): void {
  if (!doc.value) return
  pieceTitle.value = doc.value.title ?? ''
  pieceArtist.value = doc.value.artist ?? ''
  pieceError.value = null
  pieceOpen.value = true
}

async function submitPiece(): Promise<void> {
  pieceError.value = null
  try {
    await store.updatePieceInfo(
      docId.value,
      pieceTitle.value.trim() || null,
      pieceArtist.value.trim() || null,
    )
    pieceOpen.value = false
    store.notice = '曲目信息已保存'
  } catch (e) {
    pieceError.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(async () => {
  loading.value = true
  try {
    // 直接进入详情页（非从库页面跳转）时先加载曲谱库，确保 doc 可查
    if (store.documents.length === 0) await store.loadDocuments()
    if (store.groups.length === 0) await store.loadGroups()
    if (docId.value) await store.selectDocument(docId.value)
    moveGroupId.value = doc.value?.group_id ?? ''
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

    <!-- 成功提示条 -->
    <div
      v-if="store.notice"
      class="mt-4 flex items-center gap-3 border border-secondary bg-secondary/10 px-4 py-3 text-secondary"
      role="status"
    >
      <span class="material-symbols-outlined">check_circle</span>
      <span class="flex-1 font-code-sm text-code-sm">{{ store.notice }}</span>
      <button type="button" class="text-secondary hover:opacity-80" @click="store.clearNotice()">
        <span class="material-symbols-outlined text-[18px]">close</span>
      </button>
    </div>

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
        <button
          type="button"
          class="text-error hover:opacity-80"
          @click="store.clearError()"
        >
          <span class="material-symbols-outlined text-[18px]">close</span>
        </button>
      </div>

      <div class="mt-4 grid gap-6 lg:grid-cols-[1fr_320px]">
        <!-- 基本信息 + 轨道列表 -->
        <section class="bento-item rounded-xl p-6">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <span
                  class="rounded border border-outline-variant bg-surface px-2 py-1 font-code-sm text-code-sm text-on-surface-variant"
                  >{{ doc.source_type === 'audio_transcription' ? 'AUDIO' : 'MIDI' }}</span
                >
                <span
                  v-if="doc.group_id"
                  class="flex items-center gap-1 rounded border border-outline-variant bg-surface px-2 py-1 font-code-sm text-code-sm text-secondary"
                >
                  <span class="material-symbols-outlined text-[14px]">folder</span>
                  {{ store.groups.find((g) => g.group_id === doc!.group_id)?.name ?? '分组' }}
                </span>
              </div>
              <h1 class="mt-2 font-display text-[26px] text-on-surface">{{ doc.name }}</h1>
              <p v-if="doc.title || doc.artist" class="mt-1 text-sm text-secondary">
                {{ [doc.title, doc.artist].filter(Boolean).join(' — ') }}
              </p>
              <p class="mt-1 font-code-sm text-code-sm text-on-surface-variant">
                IMPORTED {{ formatDateTime(doc.imported_at) }}
              </p>
            </div>
            <div class="flex items-center gap-1 rounded border border-outline-variant bg-surface-container-lowest px-1 py-1">
              <button
                type="button"
                class="flex items-center gap-1 px-2 py-1 font-code-sm text-code-sm text-on-surface-variant transition-colors hover:bg-surface-container-highest hover:text-primary"
                @click="openRename"
              >
                <span class="material-symbols-outlined text-[16px]">edit</span>
                重命名
              </button>
              <button
                type="button"
                class="flex items-center gap-1 px-2 py-1 font-code-sm text-code-sm text-on-surface-variant transition-colors hover:bg-surface-container-highest hover:text-error"
                @click="openDelete"
              >
                <span class="material-symbols-outlined text-[16px]">delete</span>
                删除
              </button>
            </div>
          </div>

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

          <!-- 转录信息 -->
          <div class="mt-6">
            <div class="mb-3 border-b border-outline-variant pb-3">
              <h2 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
                <span class="material-symbols-outlined text-[20px]">manage_search</span>
                转录信息
              </h2>
            </div>
            <div v-if="doc.transcription" class="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">ENGINE</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-secondary">
                  {{ engineLabel(doc.transcription.engine) }}
                </dd>
              </div>
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">MODEL</dt>
                <dd class="mt-1 truncate font-code-sm text-code-sm text-primary" :title="doc.transcription.model_file">
                  {{ doc.transcription.model_file }}
                </dd>
              </div>
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">ENGINE VER</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-primary">
                  v{{ doc.transcription.engine_version }}
                </dd>
              </div>
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">ELAPSED</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-primary">
                  {{ formatElapsed(doc.transcription.elapsed_ms) }}
                </dd>
              </div>
              <div class="col-span-2 rounded border border-outline-variant bg-surface p-4 sm:col-span-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">COMPLETED AT</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-primary">
                  {{ formatDateTime(doc.transcription.completed_at_ms) }}
                </dd>
              </div>
            </div>
            <p
              v-else-if="doc.source_type === 'audio_transcription'"
              class="rounded border border-dashed border-outline-variant px-4 py-3 font-code-sm text-code-sm text-on-surface-variant"
            >
              该曲谱为早期版本转录，转录元数据不可用。
            </p>
            <p
              v-else
              class="rounded border border-dashed border-outline-variant px-4 py-3 font-code-sm text-code-sm text-on-surface-variant"
            >
              直接导入的 MIDI 曲谱没有转录信息。
            </p>
          </div>

          <!-- 曲目信息 -->
          <div class="mt-6">
            <div class="mb-3 flex items-center justify-between border-b border-outline-variant pb-3">
              <h2 class="flex items-center gap-2 font-body-lg text-body-lg text-primary">
                <span class="material-symbols-outlined text-[20px]">library_music</span>
                曲目信息
              </h2>
              <button
                type="button"
                class="flex items-center gap-1 px-2 py-1 font-code-sm text-code-sm text-on-surface-variant transition-colors hover:bg-surface-container-highest hover:text-primary"
                @click="openPiece"
              >
                <span class="material-symbols-outlined text-[16px]">edit</span>
                {{ doc.title || doc.artist ? '编辑' : '添加' }}
              </button>
            </div>
            <dl class="grid grid-cols-2 gap-4">
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">TITLE</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-primary">
                  {{ doc.title || '—' }}
                </dd>
              </div>
              <div class="rounded border border-outline-variant bg-surface p-4">
                <dt class="font-label-caps text-label-caps text-on-surface-variant">ARTIST</dt>
                <dd class="mt-1 font-code-sm text-code-sm text-primary">
                  {{ doc.artist || '—' }}
                </dd>
              </div>
            </dl>
          </div>

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
          <div class="mt-6 border-t border-outline-variant/50 pt-4">
            <h3 class="font-label-caps text-label-caps text-on-surface-variant">分组</h3>
            <p class="mt-2 text-sm text-on-surface-variant">
              当前：{{
                doc.group_id
                  ? store.groups.find((g) => g.group_id === doc!.group_id)?.name ?? '—'
                  : '未分类'
              }}
            </p>
            <div class="mt-3 flex gap-2">
              <select
                v-model="moveGroupId"
                class="min-w-0 flex-1 border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
              >
                <option value="">未分类</option>
                <option v-for="g in store.groups" :key="g.group_id" :value="g.group_id">
                  {{ g.name }}
                </option>
              </select>
              <button
                type="button"
                class="flex items-center gap-1 rounded border border-outline-variant px-3 py-1.5 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
                @click="submitMoveGroup"
              >移动</button>
            </div>
            <p v-if="moveGroupError" class="mt-2 font-code-sm text-code-sm text-error">
              {{ moveGroupError }}
            </p>
            <RouterLink
              to="/"
              class="mt-3 flex w-full items-center justify-center gap-2 rounded border border-outline-variant px-4 py-2 font-label-caps text-label-caps text-on-surface transition-colors hover:border-primary hover:text-primary"
            >
              在曲谱库中管理分组 →
            </RouterLink>
          </div>
        </aside>
      </div>

      <!-- 重命名曲谱弹窗 -->
      <div
        v-if="renameOpen"
        class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
        @click.self="renameOpen = false"
      >
        <div
          class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
        >
          <h2 class="font-display text-[24px] text-on-surface">重命名曲谱</h2>
          <input
            v-model="renameName"
            type="text"
            placeholder="曲谱名称"
            class="mt-4 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
            @keyup.enter="submitRename"
          />
          <p v-if="renameError" class="mt-2 font-code-sm text-code-sm text-error">
            {{ renameError }}
          </p>
          <div class="mt-6 flex justify-end gap-3">
            <button
              type="button"
              class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
              @click="renameOpen = false"
            >取消</button>
            <button
              type="button"
              class="rounded bg-primary-container px-5 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
              :disabled="!renameName.trim()"
              @click="submitRename"
            >保存</button>
          </div>
        </div>
      </div>

      <!-- 删除曲谱确认弹窗 -->
      <div
        v-if="deleteOpen"
        class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
        @click.self="deleteOpen = false"
      >
        <div
          class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
        >
          <h2 class="font-display text-[24px] text-on-surface">删除曲谱</h2>
          <p class="mt-2 text-sm text-on-surface-variant">
            确定删除「<span class="text-on-surface">{{ doc.name }}</span>」？
            曲谱文件将从曲谱库中永久移除，此操作不可撤销。
          </p>
          <p v-if="deleteError" class="mt-2 font-code-sm text-code-sm text-error">
            {{ deleteError }}
          </p>
          <div class="mt-6 flex justify-end gap-3">
            <button
              type="button"
              class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
              @click="deleteOpen = false"
            >取消</button>
            <button
              type="button"
              class="rounded bg-error px-5 py-2 font-label-caps text-label-caps text-on-error-container transition-colors hover:opacity-80"
              @click="submitDelete"
            >删除</button>
          </div>
        </div>
      </div>

      <!-- 编辑曲目信息弹窗 -->
      <div
        v-if="pieceOpen"
        class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
        @click.self="pieceOpen = false"
      >
        <div
          class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
        >
          <h2 class="font-display text-[24px] text-on-surface">编辑曲目信息</h2>
          <p class="mt-1 text-sm text-on-surface-variant">
            音频转录的曲谱会自动读取源音频标签；此处可手动修正。留空表示未设置。
          </p>
          <label class="mt-4 block">
            <span class="font-label-caps text-label-caps text-on-surface-variant">TITLE</span>
            <input
              v-model="pieceTitle"
              type="text"
              placeholder="曲目标题"
              class="mt-1 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
            />
          </label>
          <label class="mt-3 block">
            <span class="font-label-caps text-label-caps text-on-surface-variant">ARTIST</span>
            <input
              v-model="pieceArtist"
              type="text"
              placeholder="艺术家"
              class="mt-1 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
            />
          </label>
          <p v-if="pieceError" class="mt-2 font-code-sm text-code-sm text-error">
            {{ pieceError }}
          </p>
          <div class="mt-6 flex justify-end gap-3">
            <button
              type="button"
              class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
              @click="pieceOpen = false"
            >取消</button>
            <button
              type="button"
              class="rounded bg-primary-container px-5 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed"
              @click="submitPiece"
            >保存</button>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
