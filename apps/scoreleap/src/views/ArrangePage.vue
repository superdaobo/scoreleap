<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useLibraryStore } from '../stores/libraryStore'
import { usePlaybackStore } from '../stores/playbackStore'
import { compile as compileApi } from '../services/api'
import type {
  ArrangementOptions,
  CompileSummary,
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
// 转换参数
// ---------------------------------------------------------------------------
const autoFit = ref(true)
const transpose = ref(0)
const strategy = ref<RangeStrategy>('OctaveDown')
const polyphony = ref(4)
const quantize = ref<QuantizeGrid | null>(null)
const simplify = ref(true)

const compileSummary = ref<CompileSummary | null>(null)
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
  try {
    compileSummary.value = await compileApi(docId.value, ids, options)
  } catch (e) {
    store.error = String(e)
  } finally {
    compiling.value = false
  }
}

// ---------------------------------------------------------------------------
// 播放控制
// ---------------------------------------------------------------------------
const stateText = computed(() => {
  switch (playback.state) {
    case 'Idle':
      return '空闲'
    case 'Countdown':
      return '倒计时'
    case 'Playing':
      return '演奏中'
    case 'Paused':
      return '已暂停'
    case 'Stopped':
      return '已停止'
    case 'Finished':
      return '已完成'
  }
})

const stateColor = computed(() => {
  switch (playback.state) {
    case 'Idle':
      return 'bg-slate-700 text-slate-300'
    case 'Countdown':
      return 'bg-amber-500/20 text-amber-300'
    case 'Playing':
      return 'bg-emerald-500/20 text-emerald-300'
    case 'Paused':
      return 'bg-sky-500/20 text-sky-300'
    case 'Stopped':
      return 'bg-red-500/20 text-red-300'
    case 'Finished':
      return 'bg-indigo-500/20 text-indigo-300'
  }
})

const countdownText = computed(() => Math.max(1, playback.countdownSeconds))

const canStart = computed(
  () =>
    !!compileSummary.value &&
    !['Playing', 'Paused', 'Countdown'].includes(playback.state),
)

const totalSec = computed(() => (compileSummary.value?.duration_ms ?? 0) / 1000)
const progressSec = computed(() => playback.positionUs / 1e6)
const progressPercent = computed(() => {
  const total = compileSummary.value?.duration_ms ?? 0
  if (total <= 0) return 0
  return Math.min(100, Math.max(0, (playback.positionUs / 1000 / total) * 100))
})

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
// 钢琴卷帘预览（Canvas）
// ---------------------------------------------------------------------------
const PITCH_LOW = 48
const PITCH_HIGH = 83
const VIEW_W = 960
const VIEW_H = 320

const canvasRef = ref<HTMLCanvasElement | null>(null)
let rafId = 0

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
  ctx.fillStyle = '#0b1220'
  ctx.fillRect(0, 0, VIEW_W, VIEW_H)

  const durationSec = Math.max(totalSec.value, 1)
  const rowH = VIEW_H / (PITCH_HIGH - PITCH_LOW + 1)

  // 横向音高网格线（每半音一行）
  ctx.strokeStyle = '#1e293b'
  ctx.lineWidth = 1
  for (let pitch = PITCH_LOW; pitch <= PITCH_HIGH; pitch++) {
    const y = VIEW_H - (pitch - PITCH_LOW + 0.5) * rowH
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(VIEW_W, y)
    ctx.stroke()
  }
  // 八度边界线
  ctx.strokeStyle = '#334155'
  for (let pitch = PITCH_LOW; pitch <= PITCH_HIGH; pitch += 12) {
    const y = VIEW_H - (pitch - PITCH_LOW + 0.5) * rowH
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(VIEW_W, y)
    ctx.stroke()
  }

  // 纵向时间网格（每 5 秒）
  ctx.strokeStyle = '#1e293b'
  for (let s = 0; s <= durationSec; s += 5) {
    const x = (s / durationSec) * VIEW_W
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, VIEW_H)
    ctx.stroke()
  }

  // 标签：秒 / 音高
  ctx.fillStyle = '#64748b'
  ctx.font = '10px system-ui, sans-serif'
  for (let s = 0; s <= durationSec; s += 5) {
    ctx.fillText(`${s}s`, (s / durationSec) * VIEW_W + 3, 12)
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

  // 音符矩形：后端当前未开放序列音符查询接口，数据源暂为空（预留）
  // const notes: { startUs: number; durationUs: number; note: number }[] = []
  // for (const n of notes) {
  //   const x = (n.startUs / 1e6 / durationSec) * VIEW_W
  //   const w = Math.max(2, (n.durationUs / 1e6 / durationSec) * VIEW_W)
  //   const y = VIEW_H - (n.note - PITCH_LOW + 0.5) * rowH
  //   ctx.fillStyle = 'rgba(99, 102, 241, 0.85)'
  //   ctx.fillRect(x, y - rowH / 2 + 1, w, rowH - 2)
  // }

  // 当前播放位置竖线（来自 playbackStore.positionUs）
  if (playback.positionUs > 0) {
    const x = (playback.positionUs / 1e6 / durationSec) * VIEW_W
    ctx.strokeStyle = '#f43f5e'
    ctx.lineWidth = 2
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, VIEW_H)
    ctx.stroke()
    ctx.lineWidth = 1
  }
}

function loop(): void {
  draw()
  rafId = requestAnimationFrame(loop)
}

onMounted(() => {
  loop()
})

onUnmounted(() => {
  cancelAnimationFrame(rafId)
})

const stats = computed(() => compileSummary.value?.stats ?? null)
</script>

<template>
  <div>
    <RouterLink to="/" class="text-sm text-indigo-400 hover:text-indigo-300"
      >← 返回曲谱库</RouterLink
    >

    <div class="mt-4 grid gap-6 lg:grid-cols-[320px_1fr]">
      <!-- 左栏：转换参数 -->
      <section
        class="h-fit rounded-xl border border-slate-800 bg-slate-800/40 p-5"
      >
        <h1 class="text-lg font-bold text-white">编排参数</h1>

        <div class="mt-4 space-y-5">
          <label class="flex items-center justify-between">
            <span class="text-sm text-slate-300">自动转调（适配音域）</span>
            <input
              v-model="autoFit"
              type="checkbox"
              class="h-4 w-4 accent-indigo-500"
            />
          </label>

          <div>
            <div class="flex justify-between text-sm">
              <span class="text-slate-300">手动移调</span>
              <span class="font-mono text-slate-400"
                >{{ formatSigned(transpose) }} 半音</span
              >
            </div>
            <input
              v-model.number="transpose"
              type="range"
              min="-24"
              max="24"
              step="1"
              class="mt-2 w-full accent-indigo-500 disabled:opacity-40"
              :disabled="autoFit"
            />
            <p v-if="autoFit" class="mt-1 text-xs text-slate-500">
              自动转调开启时手动移调不可用
            </p>
          </div>

          <div>
            <label class="block text-sm text-slate-300">音域策略</label>
            <select
              v-model="strategy"
              class="mt-2 w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-200"
            >
              <option value="OctaveDown">降八度</option>
              <option value="Drop">丢弃</option>
              <option value="Mute">静音</option>
            </select>
          </div>

          <div>
            <div class="flex justify-between text-sm">
              <span class="text-slate-300">最大复音</span>
              <span class="font-mono text-slate-400">{{ polyphony }}</span>
            </div>
            <input
              v-model.number="polyphony"
              type="range"
              min="1"
              max="8"
              step="1"
              class="mt-2 w-full accent-indigo-500"
            />
          </div>

          <div>
            <label class="block text-sm text-slate-300">量化</label>
            <select
              v-model="quantize"
              class="mt-2 w-full rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-slate-200"
            >
              <option :value="null">关闭</option>
              <option value="Eighth">八分音符</option>
              <option value="Sixteenth">十六分音符</option>
            </select>
          </div>

          <label class="flex items-center justify-between">
            <span class="text-sm text-slate-300">和弦简化</span>
            <input
              v-model="simplify"
              type="checkbox"
              class="h-4 w-4 accent-indigo-500"
            />
          </label>

          <button
            type="button"
            class="w-full rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-4 py-2.5 font-medium text-white hover:opacity-90 disabled:opacity-50"
            :disabled="compiling"
            @click="doCompile"
          >
            {{ compiling ? '编译中…' : '编译' }}
          </button>
        </div>

        <!-- 统计徽标 -->
        <div
          v-if="stats"
          class="mt-5 rounded-lg border border-slate-700 bg-slate-900/60 p-4"
        >
          <h2 class="text-xs font-medium uppercase tracking-wide text-slate-500">
            编译统计
          </h2>
          <dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-2 text-sm">
            <div class="flex justify-between">
              <dt class="text-slate-500">移调量</dt>
              <dd class="font-mono text-indigo-300">
                {{ formatSigned(stats.applied_transpose) }}
              </dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-slate-500">输出音符</dt>
              <dd class="font-mono text-slate-200">{{ stats.output_notes }}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-slate-500">丢弃</dt>
              <dd class="font-mono text-slate-200">
                {{ stats.dropped_out_of_range }}
              </dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-slate-500">折叠</dt>
              <dd class="font-mono text-slate-200">{{ stats.folded }}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-slate-500">裁剪</dt>
              <dd class="font-mono text-slate-200">
                {{ stats.dropped_polyphony }}
              </dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-slate-500">静音</dt>
              <dd class="font-mono text-slate-200">{{ stats.muted }}</dd>
            </div>
          </dl>
          <p class="mt-2 border-t border-slate-800 pt-2 text-xs text-slate-500">
            共 {{ compileSummary?.action_count }} 个动作 ·
            {{ formatDuration(compileSummary?.duration_ms ?? 0) }}
          </p>
        </div>
      </section>

      <!-- 右栏：预览与播放 -->
      <section class="flex flex-col gap-4">
        <div
          class="rounded-xl border border-slate-800 bg-slate-800/40 p-5"
        >
          <div class="flex items-center justify-between">
            <h2 class="text-sm font-medium text-slate-300">
              钢琴卷帘预览（MIDI {{ PITCH_LOW }}–{{ PITCH_HIGH }}）
            </h2>
            <span v-if="compileSummary" class="text-xs text-slate-500"
              >序列 {{ compileSummary.seq_id }}</span
            >
          </div>
          <canvas
            ref="canvasRef"
            class="mt-3 w-full rounded-lg border border-slate-800"
            style="height: 320px"
          ></canvas>
          <p class="mt-2 text-xs text-slate-500">
            音符数据由后端序列接口提供（当前版本暂未开放），此处显示时间轴与播放进度。
          </p>
        </div>

        <!-- 播放控制 -->
        <div
          class="rounded-xl border border-slate-800 bg-slate-800/40 p-5"
        >
          <div class="flex flex-wrap items-center gap-3">
            <span
              class="rounded-full px-3 py-1 text-xs font-medium"
              :class="stateColor"
              >{{ stateText }}</span
            >
            <span
              v-if="playback.state === 'Countdown'"
              class="text-2xl font-bold tabular-nums text-violet-300"
              >{{ countdownText }}</span
            >
            <label
              class="ml-auto flex items-center gap-2 text-xs text-slate-400"
            >
              <input
                v-model="mockBackend"
                type="checkbox"
                class="h-3.5 w-3.5 accent-indigo-500"
              />
              测试模式（mock，不注入真实输入）
            </label>
          </div>

          <div class="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              class="rounded-lg bg-gradient-to-r from-indigo-500 to-violet-600 px-4 py-2 text-sm font-medium text-white hover:opacity-90 disabled:opacity-40"
              :disabled="!canStart"
              @click="startPlay"
            >
              开始演奏
            </button>
            <button
              v-if="playback.state === 'Playing'"
              type="button"
              class="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600"
              @click="playback.pause()"
            >
              暂停
            </button>
            <button
              v-if="playback.state === 'Paused'"
              type="button"
              class="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600"
              @click="playback.resume()"
            >
              继续
            </button>
            <button
              v-if="
                playback.state === 'Playing' ||
                playback.state === 'Paused' ||
                playback.state === 'Countdown'
              "
              type="button"
              class="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600"
              @click="playback.stop()"
            >
              停止
            </button>
            <button
              type="button"
              class="ml-auto rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500"
              @click="playback.emergency()"
            >
              紧急停止（Ctrl+Alt+F9）
            </button>
          </div>

          <div class="mt-4">
            <div class="flex justify-between text-xs text-slate-500">
              <span>{{ progressSec.toFixed(1) }}s</span>
              <span>{{ totalSec.toFixed(1) }}s</span>
            </div>
            <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-slate-700">
              <div
                class="h-full rounded-full bg-gradient-to-r from-indigo-500 to-violet-500 transition-[width] duration-200"
                :style="{ width: `${progressPercent}%` }"
              ></div>
            </div>
          </div>

          <p
            v-if="playback.state === 'Countdown'"
            class="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-center text-sm text-amber-300"
          >
            即将开始：{{ countdownText }} 秒，请切换到游戏窗口！
          </p>
          <p
            v-if="playback.error"
            class="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-300"
          >
            {{ playback.error }}
          </p>
          <p
            v-if="store.error"
            class="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-300"
          >
            {{ store.error }}
          </p>
        </div>
      </section>
    </div>
  </div>
</template>
