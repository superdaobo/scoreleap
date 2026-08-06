<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore } from '../stores/settingsStore'

const router = useRouter()
const settings = useSettingsStore()
const agreed = ref(false)

function acceptAndContinue(): void {
  if (!agreed.value) return
  settings.acceptRisk()
  void router.replace('/')
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Enter' && agreed.value) acceptAndContinue()
}

onMounted(() => document.addEventListener('keydown', onKeydown))
onUnmounted(() => document.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div
    class="tech-grid relative flex min-h-[calc(100vh-0px)] items-center justify-center overflow-hidden bg-background px-gutter-mobile md:px-0"
  >
    <!-- 中心径向渐变 -->
    <div
      class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_center,transparent_0%,#131313_100%)]"
    ></div>

    <main class="relative z-10 w-full max-w-[540px]">
      <!-- 品牌锚点 -->
      <div class="mb-8 flex items-center justify-center gap-3">
        <span class="material-symbols-outlined text-primary text-[32px]"
          >graphic_eq</span
        >
        <span class="font-display text-[28px] font-bold tracking-tight text-primary"
          >ScoreLeap</span
        >
      </div>

      <div
        class="flex flex-col gap-10 border border-outline-variant bg-surface-container-lowest p-gutter-desktop shadow-2xl shadow-black/80"
      >
        <!-- 头部 -->
        <header class="space-y-3 text-center">
          <div
            class="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full border border-error/30 bg-error/10"
          >
            <span class="material-symbols-outlined text-error" style="font-variation-settings: 'FILL' 1"
              >warning</span
            >
          </div>
          <h1 class="font-display text-headline-md text-on-surface">
            风险与合规确认
          </h1>
          <p class="mx-auto max-w-[80%] text-on-surface-variant">
            在使用该工具前，您必须了解并接受以下核心运行原则与潜在风险。
          </p>
        </header>

        <!-- 风险条目 -->
        <div class="space-y-4">
          <div
            class="flex items-start gap-4 border border-outline-variant bg-surface-container-high/50 p-4 transition-colors hover:border-error/50"
          >
            <span class="material-symbols-outlined mt-0.5 text-error">gavel</span>
            <div>
              <h3
                class="mb-1 font-code-sm text-code-sm uppercase tracking-wider text-error"
              >
                封号风险 (Ban Risk)
              </h3>
              <p class="text-[14px] leading-relaxed text-on-surface-variant">
                第三方辅助工具可能违反目标平台的服务条款，存在账号被限制或永久封禁的实际风险。
              </p>
            </div>
          </div>
          <div
            class="flex items-start gap-4 border border-outline-variant bg-surface-container-high/50 p-4 transition-colors hover:border-primary/50"
          >
            <span class="material-symbols-outlined mt-0.5 text-primary"
              >sports_esports</span
            >
            <div>
              <h3
                class="mb-1 font-code-sm text-code-sm uppercase tracking-wider text-primary"
              >
                仅限非竞技 (Non-competitive)
              </h3>
              <p class="text-[14px] leading-relaxed text-on-surface-variant">
                本工具设计初衷为辅助练习与数据分析，严禁用于任何形式的排名、竞技或破坏公平性的场景。
              </p>
            </div>
          </div>
          <div
            class="flex items-start gap-4 border border-outline-variant bg-surface-container-high/50 p-4 transition-colors hover:border-secondary/50"
          >
            <span class="material-symbols-outlined mt-0.5 text-secondary"
              >shield_lock</span
            >
            <div>
              <h3
                class="mb-1 font-code-sm text-code-sm uppercase tracking-wider text-secondary"
              >
                本地隐私 (Local Privacy)
              </h3>
              <p class="text-[14px] leading-relaxed text-on-surface-variant">
                所有处理数据均在本地运行环境存储与计算，工具本身不主动向云端上传您的个人识别信息。
              </p>
            </div>
          </div>
        </div>

        <!-- 操作区 -->
        <div class="flex flex-col gap-6 border-t border-outline-variant pt-4">
          <label class="group flex cursor-pointer items-center gap-3">
            <input
              v-model="agreed"
              type="checkbox"
              class="tech-checkbox h-5 w-5 cursor-pointer appearance-none border border-outline-variant bg-surface-container-highest transition-all duration-200"
            />
            <span
              class="font-code-sm text-code-sm text-on-surface transition-colors group-hover:text-primary"
              >&gt; 我已阅读并理解风险 (I have read and understand the risks)</span
            >
          </label>
          <button
            type="button"
            class="w-full border py-4 px-6 font-code-sm text-code-sm uppercase tracking-widest transition-all duration-300 disabled:cursor-not-allowed disabled:opacity-50"
            :class="
              agreed
                ? 'border-primary bg-primary text-on-primary hover:bg-primary-fixed active:scale-[0.98]'
                : 'border-outline-variant bg-surface-variant text-on-surface-variant'
            "
            :disabled="!agreed"
            @click="acceptAndContinue"
          >
            确认并继续 [Enter]
          </button>
        </div>
      </div>

      <!-- 页脚元数据 -->
      <div class="mt-6 text-center">
        <span
          class="border border-outline-variant/30 bg-surface-container-highest px-2 py-1 font-label-caps text-label-caps text-on-surface-variant/50"
          >SCORELEAP_SYS_v0.2.4</span
        >
      </div>
    </main>
  </div>
</template>
