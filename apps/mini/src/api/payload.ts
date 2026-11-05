import type { DraftCard } from './types'

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  return value as Record<string, unknown>
}

/** 微信有时把 JSON 当字符串交回来。能解析才解析。 */
export function unwrapBody(value: unknown): unknown {
  if (typeof value !== 'string') return value
  const trimmed = value.trim()
  if (!trimmed || (trimmed[0] !== '{' && trimmed[0] !== '[')) return value
  try {
    return JSON.parse(trimmed) as unknown
  } catch {
    return value
  }
}

function isDraftCard(value: unknown): value is DraftCard {
  const rec = asRecord(value)
  if (!rec) return false
  return typeof rec.id === 'string' && rec.id.trim() !== '' && typeof rec.title === 'string'
}

function cardsOf(value: unknown): DraftCard[] | null {
  if (!Array.isArray(value) || value.length === 0) return null
  if (!value.every(isDraftCard)) return null
  return value
}

/**
 * 从列表接口的各种包法里取出稿件。
 * 认 `{ items }`、裸数组、`{ data: { items } }`，以及误放在 `message` 里的稿件数组。
 * 认不出来返回 null。`{ items: [] }` 返回空数组，表示请求成功但没有稿。
 */
export function draftCardsFrom(value: unknown, depth = 0): DraftCard[] | null {
  if (depth > 4) return null
  const parsed = unwrapBody(value)
  const direct = cardsOf(parsed)
  if (direct) return direct
  if (Array.isArray(parsed)) return null
  const rec = asRecord(parsed)
  if (!rec) return null
  if (Array.isArray(rec.items)) {
    if (rec.items.length === 0) return []
    const cards = cardsOf(rec.items)
    if (cards) return cards
  }
  for (const key of ['data', 'result', 'list', 'drafts', 'message']) {
    if (!(key in rec)) continue
    const nested = draftCardsFrom(rec[key], depth + 1)
    if (!nested) continue
    if (nested.length === 0 && key === 'message') continue
    return nested
  }
  return null
}

function clip(text: string): string {
  const clean = text.replace(/\s+/g, ' ').trim()
  if (clean.length <= 80) return clean
  return `${clean.slice(0, 80)}…`
}

/** 只返回能直接显示的中文或原文。对象、对象数组不会变成 [object Object]。 */
export function errorText(value: unknown, fallback = '操作失败'): string {
  const parsed = unwrapBody(value)
  if (typeof parsed === 'string') {
    const text = parsed.trim()
    if (!text || text.includes('[object Object]')) return fallback
    return clip(text)
  }
  if (Array.isArray(parsed)) {
    const parts: string[] = []
    for (const item of parsed) {
      if (typeof item === 'string' && item.trim()) {
        parts.push(item.trim())
        continue
      }
      const rec = asRecord(item)
      const bit = rec?.message ?? rec?.msg
      if (typeof bit === 'string' && bit.trim()) parts.push(bit.trim())
    }
    if (!parts.length) return fallback
    return clip(parts.slice(0, 3).join('；'))
  }
  const rec = asRecord(parsed)
  if (!rec) return fallback
  if (typeof rec.message === 'string' && rec.message.trim()) return clip(rec.message.trim())
  if (Array.isArray(rec.message)) {
    const text = errorText(rec.message, '')
    if (text) return text
  }
  if (typeof rec.msg === 'string' && rec.msg.trim()) return clip(rec.msg.trim())
  return fallback
}

/** 服务端没有给出可读原因时，用调用方的中文摘要，避免把「请求失败」或对象数组摊到页面上。 */
export function readableError(value: unknown, fallback: string): string {
  const extracted = errorText(value, '')
  if (!extracted || extracted === '请求失败') return fallback
  return extracted
}
