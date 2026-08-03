/** 时长格式化：毫秒 → "m:ss" */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const m = Math.floor(total / 60)
  const s = total % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

/** 带符号整数（用于移调量等统计展示） */
export function formatSigned(n: number): string {
  return n >= 0 ? `+${n}` : `${n}`
}
