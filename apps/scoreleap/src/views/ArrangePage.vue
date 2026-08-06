<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { errorText } from '../utils/format'
import { usePlaybackStore } from '../stores/playbackStore'
import { compile as compileApi, currentProfile, getSequenceNotes, loadProfile, listProfiles } from '../services/api'
import type {
  ArrangementOptions,
  CompileSummary,
  NoteView,
  QuantizeGrid,
  RangeStrategy,
} from '../types'
import { formatDuration, formatSigned } from '../utils/format'

const route = useRoute()
const store = useLibraryStore()
const playback = usePlaybackStore()

const docId = computed(() =>
  typeof route.query.docId === 'string' ? route.query.docId : store.currentDocId ?? '',
)

// ---------------------------------------------------------------------------
// Profile 确保加载（后端 compile 也有兜底，这里提前加载以便显示更准确的错误）
// ---------------------------------------------------------------------------
async function ensureProfile(): Promise<void> {
  try {
    const cur = await currentProfile()
    if (cur) return
    const profiles = await listProfiles()
    if (profiles.length > 0) {
      await loadProfile(profiles[0])
    } else {
      store.error = '未找到可用的游戏 Profile（缺少 game-profiles 目录）。'
    }
  } catch (e) {
    store.error = errorText(e)
  }
}

// ---------------------------------------------------------------------------
// 转换参数
// ---------------------------------------------------------------------------
const autoFit = ref(true)
const transpose = ref(0)
const strategy = ref<RangeStrategy>('OctaveDown')
const polyphony = ref(4)
const quantize = ref<QuantizeGrid | null>(null)
const simplify = ref(true)

const compileSummary = ref<CompileSummary | null>(null)
const notesData = ref<NoteView[]>([])
const compiling = ref(false)
const mockBackend = ref(false)

async function doCompile(): Promise<void> {
  if (!docId.value) {
    store.error = '未选择曲谱，请先返回曲谱库导入并进入曲谱详情。'
    return
  }
  const ids = store.enabledTrackIds(docId.value)
  if (ids.length === 0) {
    store.error = '没有启用的轨道，请先启用至少一条轨道。'
    return
  }
  const options: ArrangementOptions = {
    transpose_semitones: transpose.value,
    auto_fit_range: autoFit.value,
    range_strategy: strategy.value,
    max_polyphony: polyphony.value,
    quantize_grid: quantize.value,
    simplify_chords: simplify.value,
  }
  compiling.value = true
  store.error = null
  notesData.value = []
  try {
    compileSummary.value = await compileApi(docId.value, ids, options)
    // 获取卷帘预览音符数据（编译缓存）
    try {
      notesData.value = await getSequenceNotes(compileSummary.value.seq_id)
    } catch (e) {
      // 音符数据获取失败不阻断编译（仅预览缺失）
      console.warn('获取卷帘音符失败:', e)
      notesData.value = []
    }
    // 新序列复位视图
    viewStartUs.value = 0
    zoom.value = 1
  } catch (e) {
    store.error = errorText(e)
  } finally {
    compiling.value = false
  }
}

// ---------------------------------------------------------------------------
// 播放控制
// ---------------------------------------------------------------------------
const stateText = computed(() => {
  switch (playback.state) {
    case 'Idle': return '空闲'
    case 'Countdown': return '倒计时'
    case 'Playing': return '演奏中'
    case 'Paused': return '已暂停'
    case 'Stopped': return '已停止'
    case 'Finished': return '已完成'
  }
})

const stateClass = computed(() => {
  switch (playback.state) {
    case 'Idle': return 'border-outline-variant text-on-surface-variant'
    case 'Countdown': return 'border-amber-500/50 text-amber-300'
    case 'Playing': return 'border-secondary text-secondary'
    case 'Paused': return 'border-sky-500/50 text-sky-300'
    case 'Stopped': return 'border-error text-error'
    case 'Finished': return 'border-primary text-primary'
  }
})

const countdownText = computed(() => Math.max(1, playback.countdownSeconds))

const canStart = computed(
  () =>
    !!compileSummary.value &&
    !['Playing', 'Paused', 'Countdown'].includes(playback.state),
)

const totalMs = computed(() => compileSummary.value?.duration_ms ?? 0)
const totalSec = computed(() => totalMs.value / 1000)

/** 进度条当前显示位置（毫秒）：拖拽中显示拖拽值，否则跟随播放 */
const scrubMs = ref(0)
const scrubbing = ref(false)
const progressMs = computed(() => (scrubbing.value ? scrubMs.value : playback.positionUs / 1000))

function onScrubInput(e: Event): void {
  const v = Number((e.target as HTMLInputElement).value)
  scrubbing.value = true
  scrubMs.value = v
}
function onScrubChange(): void {
  if (scrubbing.value) {
    void playback.seek(scrubMs.value * 1000)
  }
  scrubbing.value = false
}

async function startPlay(): Promise<void> {
  if (!compileSummary.value) {
    store.error = '请先编译曲谱。'
    return
  }
  const ok = window.confirm(
    '风险确认：自动演奏可能违反游戏用户协议，存在封号风险。仅用于自由演奏/个人空间等非竞技场景。是否继续？',
  )
  if (!ok) return
  try {
    await playback.start(
      compileSummary.value.seq_id,
      mockBackend.value ? 'mock' : 'sendinput',
    )
  } catch {
    // 错误已写入 playback.error
  }
}

// ---------------------------------------------------------------------------
// 钢琴卷帘预览（Canvas）——金色音符 / 暗色舞台 / 支持缩放与横向窗口
// ---------------------------------------------------------------------------
const PITCH_LOW = 48
const PITCH_HIGH = 83
const VIEW_W = 960
const VIEW_H = 320

const canvasRef = ref<HTMLCanvasElement | null>(null)
let rafId = 0

/** 缩放倍率（1 = 全曲一屏；越大越放大，可横向浏览） */
const zoom = ref(1)
/** 可视窗口左缘的逻辑时间（微秒） */
const viewStartUs = ref(0)
const ZOOM_MIN = 1
const ZOOM_MAX = 32

const winDurationUs = computed(() => {
  const total = totalMs.value * 1000
  return Math.max(total / zoom.value, 1)
})

/** 缩放自适应标尺刻度（秒） */
const tickIntervalSec = computed(() => {
  const sec = (winDurationUs.value / 1e6) / 5
  for (const t of [0.25, 0.5, 1, 2, 5, 10, 30, 60]) {
    if (sec >= t * 0.7) return t
  }
  return 120
})

function zoomBy(factor: number): void {
  const oldWin = winDurationUs.value
  const next = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom.value * factor))
  if (next === zoom.value) return
  // 保持窗口中心时间不变
  const centerUs = viewStartUs.value + oldWin / 2
  zoom.value = next
  const newWin = winDurationUs.value
  viewStartUs.value = Math.max(0, centerUs - newWin / 2)
}

function zoomReset(): void {
  zoom.value = 1
  viewStartUs.value = 0
}

function draw(): void {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  if (canvas.width !== VIEW_W * dpr) {
    canvas.width = VIEW_W * dpr
    canvas.height = VIEW_H * dpr
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)

  // 背景
  ctx.fillStyle = '#0a0a0a'
  ctx.fillRect(0, 0, VIEW_W, VIEW_H)

  const rowH = VIEW_H / (PITCH_HIGH - PITCH_LOW + 1)
  const winStartUs = viewStartUs.value
  const winEndUs = winStartUs + winDurationUs.value

  // 横向音高网格线（每半音一行）
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)'
  ctx.lineWidth = 1
  for (let pitch = PITCH_LOW; pitch <= PITCH_HIGH; pitch++) {
    const y = VIEW_H - (pitch - PITCH_LOW + 0.5) * rowH
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(VIEW_W, y)
    ctx.stroke()
  }
  // 八度边界线
  ctx.strokeStyle = 'rgba(255, 215, 0, 0.15)'
  for (let pitch = PITCH_LOW; pitch <= PITCH_HIGH; pitch += 12) {
    const y = VIEW_H - (pitch - PITCH_LOW + 0.5) * rowH
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(VIEW_W, y)
    ctx.stroke()
  }

  // 纵向时间网格（自适应刻度）
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)'
  const tickUs = tickIntervalSec.value * 1e6
  const firstTick = Math.ceil(winStartUs / tickUs) * tickUs
  for (let t = firstTick; t <= winEndUs; t += tickUs) {
    const x = ((t - winStartUs) / winDurationUs.value) * VIEW_W
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, VIEW_H)
    ctx.stroke()
  }

  // 标签：时间 / 音高
  ctx.fillStyle = '#64748b'
  ctx.font = '10px "JetBrains Mono", monospace'
  for (let t = firstTick; t <= winEndUs; t += tickUs) {
    const x = ((t - winStartUs) / winDurationUs.value) * VIEW_W
    const label = tickIntervalSec.value >= 1 ? `${(t / 1e6).toFixed(0)}s` : `${(t / 1e6).toFixed(2)}s`
    ctx.fillText(label, x + 3, 12)
  }
  ctx.textAlign = 'right'
  for (let pitch = PITCH_LOW; pitch <= PITCH_HIGH; pitch += 12) {
    ctx.fillText(
      String(pitch),
      VIEW_W - 4,
      VIEW_H - (pitch - PITCH_LOW + 0.5) * rowH + 4,
    )
  }
  ctx.textAlign = 'left'

  // 音符矩形（金色）——只画窗口内
  for (const n of notesData.value) {
    const startX = ((n.start_us - winStartUs) / winDurationUs.value) * VIEW_W
    const width = Math.max(1.5, (n.duration_us / winDurationUs.value) * VIEW_W)
    if (startX + width < 0 || startX > VIEW_W) continue
    const y = VIEW_H - (n.note - PITCH_LOW + 0.5) * rowH
    ctx.fillStyle = 'rgba(255, 215, 0, 0.8)'
    ctx.strokeStyle = '#ffd700'
    ctx.lineWidth = 1
    ctx.beginPath()
    ctx.rect(startX, y - rowH / 2 + 1, width, rowH - 2)
    ctx.fill()
    ctx.stroke()
  }

  // 当前播放位置竖线（在窗口内才画）
  const posUs = playback.positionUs
  if (posUs >= winStartUs && posUs <= winEndUs) {
    const x = ((posUs - winStartUs) / winDurationUs.value) * VIEW_W
    ctx.strokeStyle = '#ffb4ab'
    ctx.lineWidth = 2
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, VIEW_H)
    ctx.stroke()
    ctx.lineWidth = 1
  }

  // 播放时自动跟随：位置越过窗口右缘 90% 时窗口前移
  if (['Playing', 'Paused'].includes(playback.state) && posUs > winStartUs + winDurationUs.value * 0.9) {
    viewStartUs.value = Math.max(0, posUs - winDurationUs.value * 0.4)
  }
}

function loop(): void {
  draw()
  rafId = requestAnimationFrame(loop)
}

onMounted(() => {
  ensureProfile()
  loop()
})

onUnmounted(() => {
  cancelAnimationFrame(rafId)
})

const stats = computed(() => compileSummary.value?.stats ?? null)

/** 参数通俗解释（问号悬停提示） */
const paramHints: Record<string, string> = {
  polyphony: '同一时刻最多同时响几个音。调太小，和弦会被砍掉几个音；调太大，游戏乐器可能按不过来。一般 4 就够。',
  transpose: '把整首曲子整体升高或降低几个半音（一个八度 = 12 个半音），让旋律落在游戏琴键适合的范围内。',
  autoFit: '自动把音符整体移动，尽量让所有音都落在游戏乐器的音域里。开启后手动移调会被禁用。',
  strategy: '超出音域的音符怎么处理：降八度 = 整体下移一个八度接着弹；丢弃 = 干脆不弹；静音 = 保留节奏但不发音。',
  quantize: '把音符的起止时间对齐到节拍网格，让节奏更整齐。适合原始录音拍点不齐的情况。',
  simplify: '把复杂的和弦简化成更容易弹的形式（保留骨干音），降低演奏难度。',
}

/** 缩放按钮状态 */
const zoomLabel = computed(() => `${zoom.value.toFixed(1)}x`)
</script>

<template>
  <div class="flex h-[calc(100vh-96px)] flex-col gap-4">
    <!-- 页面头 -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-4">
        <RouterLink
          to="/"
          class="flex items-center gap-2 text-sm text-on-surface-variant transition-colors hover:text-primary"
        >
          <span class="material-symbols-outlined text-[18px]">arrow_back</span>
          返回曲谱库
        </RouterLink>
        <div class="h-4 w-px bg-outline-variant"></div>
        <h1 class="font-display text-[24px] text-primary">编排工作台</h1>
      </div>
      <span
        v-if="compileSummary"
        class="rounded border border-outline-variant bg-surface-container-high px-2 py-1 font-code-sm text-code-sm text-on-surface-variant"
        >SEQ: {{ compileSummary.seq_id }}</span
      >
    </div>

    <div class="flex min-h-0 flex-1 gap-4">
      <!-- 左栏：参数 + 播放控制 -->
      <aside
        class="flex w-[340px] shrink-0 flex-col gap-6 overflow-y-auto border border-outline-variant bg-surface-container-low/50 p-6 backdrop-blur-sm"
      >
        <div>
          <h2
            class="mb-6 flex items-center gap-2 font-label-caps text-label-caps uppercase tracking-widest text-on-surface-variant"
          >
            <span class="material-symbols-outlined text-[16px]">tune</span>
            编排参数
          </h2>
          <div class="space-y-6">
            <!-- 最大复音 -->
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                  最大复音
                  <span class="help-hint">
                    <span class="help-icon">?</span>
                    <span class="help-tip">{{ paramHints.polyphony }}</span>
                  </span>
                </label>
                <span class="font-code-sm text-code-sm text-primary">{{ polyphony }}</span>
              </div>
              <div class="grid grid-cols-8 gap-1">
                <button
                  v-for="n in 8"
                  :key="n"
                  type="button"
                  class="h-8 border font-code-sm text-xs transition-colors"
                  :class="
                    polyphony === n
                      ? 'border-primary bg-primary/10 text-primary'
                      : 'border-outline-variant text-on-surface-variant hover:border-primary hover:text-primary'
                  "
                  @click="polyphony = n"
                >
                  {{ n }}
                </button>
              </div>
            </div>

            <!-- 自动转调 -->
            <div
              class="flex items-center justify-between border border-outline-variant bg-surface p-3"
            >
              <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                自动转调
                <span class="help-hint">
                  <span class="help-icon">?</span>
                  <span class="help-tip">{{ paramHints.autoFit }}</span>
                </span>
              </label>
              <label class="relative inline-flex cursor-pointer items-center">
                <input v-model="autoFit" type="checkbox" class="peer sr-only" />
                <div
                  class="peer h-5 w-9 rounded-full bg-surface-variant after:absolute after:left-[2px] after:top-[2px] after:h-4 after:w-4 after:rounded-full after:border after:border-on-surface after:bg-on-surface after:transition-all peer-checked:bg-primary peer-checked:after:translate-x-full peer-checked:after:border-white"
                ></div>
              </label>
            </div>

            <!-- 手动移调 -->
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                  手动移调
                  <span class="help-hint">
                    <span class="help-icon">?</span>
                    <span class="help-tip">{{ paramHints.transpose }}</span>
                  </span>
                </label>
                <span class="font-code-sm text-code-sm text-primary"
                  >{{ formatSigned(transpose) }} 半音</span
                >
              </div>
              <input
                v-model.number="transpose"
                type="range"
                min="-12"
                max="12"
                step="1"
                class="h-1 w-full cursor-pointer appearance-none rounded-lg bg-surface-variant accent-primary disabled:opacity-40"
                :disabled="autoFit"
              />
              <div class="flex justify-between font-code-sm text-xs text-on-surface-variant">
                <span>-12</span><span>0</span><span>+12</span>
              </div>
              <p v-if="autoFit" class="text-xs text-on-surface-variant">
                自动转调开启时手动移调不可用
              </p>
            </div>

            <!-- 音域策略 -->
            <div class="space-y-2">
              <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                音域策略
                <span class="help-hint">
                  <span class="help-icon">?</span>
                  <span class="help-tip">{{ paramHints.strategy }}</span>
                </span>
              </label>
              <select
                v-model="strategy"
                class="w-full border border-outline-variant bg-surface-container-lowest px-3 py-2 font-code-sm text-code-sm text-on-surface focus:border-primary"
              >
                <option value="OctaveDown">降八度（整体下移一个八度接着弹）</option>
                <option value="Drop">丢弃（超出音域就不弹）</option>
                <option value="Mute">静音（保留节奏但不发音）</option>
              </select>
            </div>

            <!-- 量化 -->
            <div class="space-y-2">
              <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                量化
                <span class="help-hint">
                  <span class="help-icon">?</span>
                  <span class="help-tip">{{ paramHints.quantize }}</span>
                </span>
              </label>
              <select
                v-model="quantize"
                class="w-full border border-outline-variant bg-surface-container-lowest px-3 py-2 font-code-sm text-code-sm text-on-surface focus:border-primary"
              >
                <option :value="null">关闭（保持原始节奏）</option>
                <option value="Eighth">八分音符（对齐半拍网格）</option>
                <option value="Sixteenth">十六分音符（对齐四分之一拍网格）</option>
              </select>
            </div>

            <!-- 和弦简化 -->
            <div
              class="flex items-center justify-between border border-outline-variant bg-surface p-3"
            >
              <label class="flex items-center gap-2 font-code-sm text-code-sm text-on-surface">
                和弦简化
                <span class="help-hint">
                  <span class="help-icon">?</span>
                  <span class="help-tip">{{ paramHints.simplify }}</span>
                </span>
              </label>
              <label class="relative inline-flex cursor-pointer items-center">
                <input v-model="simplify" type="checkbox" class="peer sr-only" />
                <div
                  class="peer h-5 w-9 rounded-full bg-surface-variant after:absolute after:left-[2px] after:top-[2px] after:h-4 after:w-4 after:rounded-full after:border after:border-on-surface after:bg-on-surface after:transition-all peer-checked:bg-primary peer-checked:after:translate-x-full peer-checked:after:border-white"
                ></div>
              </label>
            </div>

            <button
              type="button"
              class="flex w-full items-center justify-center gap-2 rounded bg-primary-container py-2.5 font-label-caps text-label-caps text-on-primary-container transition-colors hover:bg-primary-fixed disabled:opacity-50"
              :disabled="compiling"
              @click="doCompile"
            >
              <span class="material-symbols-outlined text-[16px]">play_arrow</span>
              {{ compiling ? '编译中…' : '编译' }}
            </button>
          </div>

          <!-- 编译统计 -->
          <div
            v-if="stats"
            class="mt-5 border border-outline-variant bg-surface-container-lowest p-4"
          >
            <h2 class="font-label-caps text-label-caps uppercase tracking-widest text-on-surface-variant">
              编译统计
            </h2>
            <dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-2 font-code-sm text-code-sm">
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">移调</dt>
                <dd class="text-primary">{{ formatSigned(stats.applied_transpose) }}</dd>
              </div>
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">输出</dt>
                <dd class="text-on-surface">{{ stats.output_notes }}</dd>
              </div>
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">丢弃</dt>
                <dd class="text-on-surface">{{ stats.dropped_out_of_range }}</dd>
              </div>
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">折叠</dt>
                <dd class="text-on-surface">{{ stats.folded }}</dd>
              </div>
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">裁剪</dt>
                <dd class="text-on-surface">{{ stats.dropped_polyphony }}</dd>
              </div>
              <div class="flex justify-between">
                <dt class="text-on-surface-variant">静音</dt>
                <dd class="text-on-surface">{{ stats.muted }}</dd>
              </div>
            </dl>
            <p class="mt-2 border-t border-outline-variant pt-2 text-xs text-on-surface-variant">
              {{ compileSummary?.action_count }} 个动作 · {{ formatDuration(compileSummary?.duration_ms ?? 0) }}
            </p>
          </div>
        </div>

        <!-- 播放控制 -->
        <div class="mt-auto border-t border-outline-variant pt-6">
          <h2
            class="mb-4 flex items-center gap-2 font-label-caps text-label-caps uppercase tracking-widest text-on-surface-variant"
          >
            <span class="material-symbols-outlined text-[16px]">play_circle</span>
            播放控制
          </h2>
          <div class="grid grid-cols-2 gap-2">
            <button
              type="button"
              class="flex flex-col items-center justify-center border border-outline-variant bg-surface p-3 text-on-surface transition-colors hover:border-primary hover:text-primary disabled:opacity-40"
              :disabled="!canStart"
              @click="startPlay"
            >
              <span class="material-symbols-outlined mb-1">play_arrow</span>
              <span class="font-code-sm text-[10px]">开始</span>
            </button>
            <button
              v-if="playback.state === 'Playing'"
              type="button"
              class="flex flex-col items-center justify-center border border-primary bg-primary/10 p-3 text-primary transition-colors"
              @click="playback.pause()"
            >
              <span class="material-symbols-outlined mb-1">pause</span>
              <span class="font-code-sm text-[10px]">暂停</span>
            </button>
            <button
              v-if="playback.state === 'Paused'"
              type="button"
              class="flex flex-col items-center justify-center border border-outline-variant bg-surface p-3 text-on-surface transition-colors hover:border-primary hover:text-primary"
              @click="playback.resume()"
            >
              <span class="material-symbols-outlined mb-1">play_circle</span>
              <span class="font-code-sm text-[10px]">继续</span>
            </button>
            <button
              v-if="['Playing', 'Paused', 'Countdown'].includes(playback.state)"
              type="button"
              class="flex flex-col items-center justify-center border border-outline-variant bg-surface p-3 text-on-surface transition-colors hover:border-error hover:text-error"
              @click="playback.stop()"
            >
              <span class="material-symbols-outlined mb-1">stop</span>
              <span class="font-code-sm text-[10px]">停止</span>
            </button>
            <button
              type="button"
              class="flex flex-col items-center justify-center border border-error bg-error/10 p-3 text-error transition-colors"
              @click="playback.emergency()"
            >
              <span class="material-symbols-outlined mb-1">emergency</span>
              <span class="font-code-sm text-[10px]">紧急停止</span>
            </button>
          </div>
        </div>
      </aside>

      <!-- 右栏：卷帘 + 播放状态 -->
      <section class="stage-lighting relative flex min-w-0 flex-1 flex-col overflow-hidden border border-outline-variant bg-[#0a0a0a]">
        <!-- 标尺 + 缩放控制 -->
        <div
          class="flex h-9 items-center justify-between border-b border-outline-variant bg-surface px-3"
        >
          <div class="flex items-center gap-2 font-code-sm text-[10px] text-on-surface-variant">
            <span>时间轴</span>
          </div>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded border border-outline-variant text-on-surface-variant transition-colors hover:border-primary hover:text-primary disabled:opacity-40"
              :disabled="zoom <= ZOOM_MIN"
              title="缩小"
              @click="zoomBy(1 / 1.5)"
            >
              <span class="material-symbols-outlined text-[14px]">zoom_out</span>
            </button>
            <span class="w-12 text-center font-code-sm text-code-sm text-primary">{{ zoomLabel }}</span>
            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded border border-outline-variant text-on-surface-variant transition-colors hover:border-primary hover:text-primary disabled:opacity-40"
              :disabled="zoom >= ZOOM_MAX"
              title="放大"
              @click="zoomBy(1.5)"
            >
              <span class="material-symbols-outlined text-[14px]">zoom_in</span>
            </button>
            <button
              type="button"
              class="rounded border border-outline-variant px-2 py-0.5 font-code-sm text-[10px] text-on-surface-variant transition-colors hover:border-primary hover:text-primary"
              title="回到全曲视图"
              @click="zoomReset"
            >适配全曲</button>
          </div>
        </div>

        <!-- 卷帘 -->
        <div class="relative min-h-0 flex-1">
          <canvas
            ref="canvasRef"
            class="absolute inset-0 h-full w-full"
          ></canvas>

          <!-- 倒计时覆盖层 -->
          <div
            v-if="playback.state === 'Countdown'"
            class="pointer-events-none absolute inset-0 z-30 flex items-center justify-center"
          >
            <span
              class="animate-pulse font-display text-[200px] text-primary opacity-20 mix-blend-screen select-none"
              >{{ countdownText }}</span
            >
          </div>
        </div>

        <!-- 状态 + 进度（可拖拽） -->
        <div class="border-t border-outline-variant bg-surface-container-low/50 p-4 backdrop-blur-sm">
          <div class="flex flex-wrap items-center gap-3">
            <span
              class="flex items-center gap-2 rounded border px-3 py-1 font-code-sm text-code-sm"
              :class="stateClass"
            >
              <span
                v-if="playback.state === 'Playing' || playback.state === 'Countdown'"
                class="h-2 w-2 animate-pulse rounded-full bg-secondary"
              ></span>
              {{ stateText }}
            </span>
            <span v-if="playback.state === 'Countdown'" class="text-2xl font-bold tabular-nums text-primary">
              {{ countdownText }}
            </span>
            <label class="ml-auto flex cursor-pointer items-center gap-2 font-code-sm text-xs text-on-surface-variant">
              <input
                v-model="mockBackend"
                type="checkbox"
                class="tech-checkbox h-3.5 w-3.5 cursor-pointer appearance-none border border-outline-variant bg-surface-container-highest"
              />
              测试模式 (mock)
            </label>
          </div>

          <!-- 可拖拽进度条 -->
          <div class="mt-3">
            <div class="flex justify-between font-code-sm text-xs text-on-surface-variant">
              <span>{{ (progressMs / 1000).toFixed(1) }}s</span>
              <span>{{ totalSec.toFixed(1) }}s</span>
            </div>
            <input
              type="range"
              class="mt-1 h-1.5 w-full cursor-pointer appearance-none rounded-full bg-surface-container-highest accent-primary"
              :min="0"
              :max="Math.max(totalMs, 1)"
              :step="100"
              :value="progressMs"
              :disabled="totalMs <= 0"
              @input="onScrubInput"
              @change="onScrubChange"
              title="拖动进度条跳转到任意位置；松开后从该位置继续播放"
            />
            <div class="mt-0.5 flex justify-between font-code-sm text-[10px] text-on-surface-variant/60">
              <span>0:00</span>
              <span>{{ formatDuration(totalMs) }}</span>
            </div>
          </div>

          <p
            v-if="playback.state === 'Countdown'"
            class="mt-3 rounded border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-center font-code-sm text-code-sm text-amber-300"
          >
            即将开始：{{ countdownText }} 秒，请切换到游戏窗口！
          </p>
          <p
            v-if="playback.error"
            class="mt-3 rounded border border-error bg-error-container/20 px-3 py-2 font-code-sm text-code-sm text-error"
          >
            {{ playback.error }}
          </p>
          <p
            v-if="store.error"
            class="mt-3 rounded border border-error bg-error-container/20 px-3 py-2 font-code-sm text-code-sm text-error"
          >
            {{ store.error }}
          </p>
          <p v-if="!compileSummary && !store.error" class="mt-2 font-code-sm text-xs text-on-surface-variant">
            先点击「编译」生成编排，再用上方按钮播放；拖拽进度条可跳转到任意位置重新播放。
          </p>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
/* 参数问号悬停提示 */
.help-hint {
  position: relative;
  display: inline-flex;
  align-items: center;
}
.help-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  border: 1px solid var(--color-outline-variant);
  border-radius: 9999px;
  font-size: 10px;
  line-height: 1;
  color: var(--color-on-surface-variant);
  cursor: help;
  transition: border-color 0.15s, color 0.15s;
}
.help-hint:hover .help-icon {
  border-color: var(--color-primary);
  color: var(--color-primary);
}
.help-tip {
  visibility: hidden;
  opacity: 0;
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  z-index: 40;
  width: 240px;
  padding: 10px 12px;
  border: 1px solid var(--color-outline-variant);
  background: var(--color-surface-container);
  color: var(--color-on-surface);
  font-size: 12px;
  line-height: 1.6;
  font-weight: 400;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
  transition: opacity 0.15s, visibility 0.15s;
  pointer-events: none;
}
.help-hint:hover .help-tip {
  visibility: visible;
  opacity: 1;
}
</style>
