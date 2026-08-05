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

/** 把未知错误（Tauri invoke 可能 reject 对象/Error/字符串）转为可读文本，避免 [object Object]。 */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e
  if (e instanceof Error) return e.message || String(e)
  if (e && typeof e === "object") {
    const obj = e as Record<string, unknown>
    const code = obj.code
    const message = obj.message
    if (code !== undefined) {
      const codeText = String(code)
      return message ? `${codeText}: ${String(message)}` : codeText
    }
    if (message !== undefined) return String(message)
    try {
      return JSON.stringify(obj)
    } catch {
      return String(e)
    }
  }
  return String(e)
}
