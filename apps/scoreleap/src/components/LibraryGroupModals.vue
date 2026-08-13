<script setup lang="ts">
import { ref, watch } from 'vue'
import { useLibraryStore } from '../stores/libraryStore'
import type { GroupInfo } from '../types'

const props = defineProps<{
  createOpen: boolean
  renameTarget: GroupInfo | null
  deleteTarget: GroupInfo | null
}>()

const emit = defineEmits<{
  (e: 'close-create'): void
  (e: 'close-rename'): void
  (e: 'close-delete'): void
  (e: 'created', group: GroupInfo): void
  (e: 'renamed'): void
  (e: 'deleted'): void
}>()

const store = useLibraryStore()

const createName = ref('')
const createError = ref<string | null>(null)
watch(
  () => props.createOpen,
  (open) => {
    if (open) {
      createName.value = ''
      createError.value = null
    }
  },
)

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

function groupCount(groupId: string): number {
  return store.documents.filter((d) => d.group_id === groupId).length
}

async function submitCreate(): Promise<void> {
  createError.value = null
  try {
    const group = await store.createGroup(createName.value)
    emit('created', group)
  } catch (e) {
    createError.value = e instanceof Error ? e.message : String(e)
  }
}

async function submitRename(): Promise<void> {
  const t = props.renameTarget
  if (!t) return
  renameError.value = null
  try {
    await store.renameGroup(t.group_id, renameName.value)
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
    await store.deleteGroup(t.group_id)
    emit('deleted')
  } catch (e) {
    deleteError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<template>
  <!-- 新建分组弹窗 -->
  <div
    v-if="props.createOpen"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-create')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">新建分组</h2>
      <p class="mt-1 text-sm text-on-surface-variant">
        创建后可在卡片操作中将曲谱移入该分组。
      </p>
      <input
        v-model="createName"
        type="text"
        placeholder="分组名称"
        class="mt-4 w-full border border-outline-variant bg-surface-container-lowest px-2 py-1.5 font-code-sm text-code-sm text-on-surface focus:border-primary"
        @keyup.enter="submitCreate"
      />
      <p v-if="createError" class="mt-2 font-code-sm text-code-sm text-error">
        {{ createError }}
      </p>
      <div class="mt-6 flex justify-end gap-3">
        <button
          type="button"
          class="border border-outline-variant px-4 py-2 font-code-sm text-code-sm text-on-surface transition-colors hover:border-primary hover:text-primary"
          @click="emit('close-create')"
        >取消</button>
        <button
          type="button"
          class="rounded bg-primary-container px-5 py-2 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
          :disabled="!createName.trim()"
          @click="submitCreate"
        >创建</button>
      </div>
    </div>
  </div>

  <!-- 重命名分组弹窗 -->
  <div
    v-if="props.renameTarget"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-rename')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">重命名分组</h2>
      <input
        v-model="renameName"
        type="text"
        placeholder="分组名称"
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

  <!-- 删除分组确认弹窗 -->
  <div
    v-if="props.deleteTarget"
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close-delete')"
  >
    <div
      class="w-full max-w-md border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
    >
      <h2 class="font-display text-[24px] text-on-surface">删除分组</h2>
      <p class="mt-2 text-sm text-on-surface-variant">
        确定删除分组「<span class="text-on-surface">{{ props.deleteTarget.name }}</span>」？
        组内 {{ groupCount(props.deleteTarget.group_id) }} 首曲谱将回到未分类，曲谱本身不会被删除。
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
        >删除分组</button>
      </div>
    </div>
  </div>
</template>
