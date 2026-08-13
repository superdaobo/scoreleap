<script setup lang="ts">
import { ref, watch } from 'vue'
import { useLibraryStore } from '../stores/libraryStore'
import type { DocumentSummary } from '../types'

const props = defineProps<{
  renameTarget: DocumentSummary | null
  deleteTarget: DocumentSummary | null
  moveTarget: DocumentSummary | null
}>()

const emit = defineEmits<{
  (e: 'close-rename'): void
  (e: 'close-delete'): void
  (e: 'close-move'): void
  (e: 'renamed'): void
  (e: 'deleted'): void
  (e: 'moved', toGroup: boolean): void
}>()

const store = useLibraryStore()

const renameName = ref('')
const renameError = ref<string | null>(null)
watch(
  () => props.renameTarget,
  (target) => {
    renameName.value = target?.name ?? ''
    renameError.value = null
  },
)

const deleteError = ref<string | null>(null)
watch(
  () => props.deleteTarget,
  () => {
    deleteError.value = null
  },
)

/** 移动分组选择（空串 = 未分类） */
const moveGroupId = ref('')
const moveError = ref<string | null>(null)
watch(
  () => props.moveTarget,
  (target) => {
    moveGroupId.value = target?.group_id ?? ''
    moveError.value = null
  },
)

async function submitRename(): Promise<void> {
  const t = props.renameTarget
  if (!t) return
  renameError.value = null
  try {
    await store.renameDocument(t.doc_id, renameName.value)
    emit('renamed')
  } catch (e) {
    renameError.value = e instanceof Error ? e.message : String(e)
  }
}

async function submitDelete(): Promise<void> {
  const t = props.deleteTarget
  if (!t) return
  deleteError.value = null
  try {
    await store.deleteDocument(t.doc_id)
    emit('deleted')
  } catch (e) {
    deleteError.value = e instanceof Error ? e.message : String(e)
  }
}

async function submitMove(): Promise<void> {
  const t = props.moveTarget
  if (!t) return
  moveError.value = null
  try {
    await store.moveToGroup(t.doc_id, moveGroupId.value || null)
    emit('moved', Boolean(moveGroupId.value))
  } catch (e) {
    moveError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<template>
  <!-- 重命名曲谱弹窗 -->
  <div
    v-if="props.renameTarget"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-rename')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">重命名曲谱</h2>
      <p class="mt-1 truncate text-sm text-on-surface-variant">{{ props.renameTarget.name }}</p>
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
          @click="emit('close-rename')"
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
    v-if="props.deleteTarget"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-delete')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">删除曲谱</h2>
      <p class="mt-2 text-sm text-on-surface-variant">
        确定删除「<span class="text-on-surface">{{ props.deleteTarget.name }}</span>」？
        曲谱文件将从曲谱库中永久移除，此操作不可撤销。
      </p>
      <p v-if="deleteError" class="mt-2 font-code-sm text-code-sm text-error">
        {{ deleteError }}
      </p>
      <div class="mt-6 flex justify-end gap-3">
        <button
          type="button"
          class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
          @click="emit('close-delete')"
        >取消</button>
        <button
          type="button"
          class="rounded bg-error px-5 py-2 font-label-caps text-label-caps text-on-error-container transition-colors hover:opacity-80"
          @click="submitDelete"
        >删除</button>
      </div>
    </div>
  </div>

  <!-- 移动到分组弹窗 -->
  <div
    v-if="props.moveTarget"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-move')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">移动到分组</h2>
      <p class="mt-1 truncate text-sm text-on-surface-variant">{{ props.moveTarget.name }}</p>
      <select
        v-model="moveGroupId"
        class="mt-4 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
      >
        <option value="">未分类</option>
        <option v-for="g in store.groups" :key="g.group_id" :value="g.group_id">
          {{ g.name }}
        </option>
      </select>
      <p v-if="moveError" class="mt-2 font-code-sm text-code-sm text-error">
        {{ moveError }}
      </p>
      <div class="mt-6 flex justify-end gap-3">
        <button
          type="button"
          class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
          @click="emit('close-move')"
        >取消</button>
        <button
          type="button"
          class="rounded bg-primary-container px-5 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed"
          @click="submitMove"
        >移动</button>
      </div>
    </div>
  </div>
</template>
