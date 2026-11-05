/** 珠格是行优先的色板下标，和稿件 BYTEA / cells_b64 同一套。 */

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

export interface ReviseCard {
  badge: string
  is_official: boolean
  is_template: boolean
}

export function canRevise(card: ReviseCard): boolean {
  return card.badge === '我的' && !card.is_official && !card.is_template
}

export function encodeCellsB64(cells: Uint8Array): string {
  let out = ''
  let i = 0
  while (i + 3 <= cells.length) {
    const n = (cells[i] << 16) | (cells[i + 1] << 8) | cells[i + 2]
    out += B64[(n >> 18) & 63]
    out += B64[(n >> 12) & 63]
    out += B64[(n >> 6) & 63]
    out += B64[n & 63]
    i += 3
  }
  const rem = cells.length - i
  if (rem === 1) {
    const n = cells[i] << 16
    out += B64[(n >> 18) & 63]
    out += B64[(n >> 12) & 63]
    out += '=='
  } else if (rem === 2) {
    const n = (cells[i] << 16) | (cells[i + 1] << 8)
    out += B64[(n >> 18) & 63]
    out += B64[(n >> 12) & 63]
    out += B64[(n >> 6) & 63]
    out += '='
  }
  return out
}

export function decodeCellsB64(raw: string, expected?: number): Uint8Array {
  const trimmed = raw.trim().replace(/=+$/, '')
  if (!trimmed) return new Uint8Array()
  const bytes: number[] = []
  let buf = 0
  let bits = 0
  for (let i = 0; i < trimmed.length; i += 1) {
    const v = B64.indexOf(trimmed[i])
    if (v < 0) throw new Error('图纸数据有误')
    buf = (buf << 6) | v
    bits += 6
    if (bits >= 8) {
      bits -= 8
      bytes.push((buf >> bits) & 0xff)
    }
  }
  const cells = Uint8Array.from(bytes)
  if (expected != null && cells.length !== expected) {
    throw new Error('珠格数量和底板尺寸不一致')
  }
  return cells
}

export function cellsEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false
  }
  return true
}

export function recount(cells: Uint8Array, paletteLength: number): number[] {
  const counts = new Array<number>(paletteLength).fill(0)
  for (let i = 0; i < cells.length; i += 1) {
    const idx = cells[i]
    if (idx < paletteLength) counts[idx] += 1
  }
  return counts
}

export function dominantIndex(counts: number[]): number {
  let best = 0
  for (let i = 1; i < counts.length; i += 1) {
    if (counts[i] > counts[best]) best = i
  }
  return best
}

export function paintIndex(
  cells: Uint8Array,
  width: number,
  x: number,
  y: number,
  index: number,
): boolean {
  if (x < 0 || y < 0 || x >= width || width <= 0) return false
  const height = cells.length / width
  if (y >= height) return false
  const at = y * width + x
  if (cells[at] === index) return false
  cells[at] = index
  return true
}

export function lineCells(x0: number, y0: number, x1: number, y1: number): Array<[number, number]> {
  const out: Array<[number, number]> = []
  const dx = Math.abs(x1 - x0)
  const sx = x0 < x1 ? 1 : -1
  const dy = -Math.abs(y1 - y0)
  const sy = y0 < y1 ? 1 : -1
  let err = dx + dy
  let x = x0
  let y = y0
  for (;;) {
    out.push([x, y])
    if (x === x1 && y === y1) break
    const e2 = 2 * err
    if (e2 >= dy) {
      err += dy
      x += sx
    }
    if (e2 <= dx) {
      err += dx
      y += sy
    }
  }
  return out
}

export function floodFill(
  cells: Uint8Array,
  width: number,
  x: number,
  y: number,
  replacement: number,
): Array<{ x: number; y: number }> {
  const height = width > 0 ? cells.length / width : 0
  if (x < 0 || y < 0 || x >= width || y >= height) return []
  const target = cells[y * width + x]
  if (target === replacement) return []
  const dirty: Array<{ x: number; y: number }> = []
  const stack: Array<[number, number]> = [[x, y]]
  while (stack.length) {
    const [cx, cy] = stack.pop() as [number, number]
    if (cx < 0 || cy < 0 || cx >= width || cy >= height) continue
    const at = cy * width + cx
    if (cells[at] !== target) continue
    cells[at] = replacement
    dirty.push({ x: cx, y: cy })
    stack.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1])
  }
  return dirty
}

/** 画之前的格子留在栈里，笔画结束再压入新格子，撤销才能回到上一笔。 */
export class CellHistory {
  private stack: Uint8Array[] = []
  private index = 0
  private pending: Uint8Array | null = null
  private max: number

  constructor(max = 40) {
    this.max = max
  }

  reset(cells: Uint8Array) {
    this.stack = [new Uint8Array(cells)]
    this.index = 0
    this.pending = null
  }

  get canUndo() {
    return this.index > 0
  }

  get canRedo() {
    return this.index < this.stack.length - 1
  }

  begin(cells: Uint8Array) {
    this.pending = new Uint8Array(cells)
  }

  end(cells: Uint8Array) {
    const before = this.pending
    this.pending = null
    if (!before || cellsEqual(before, cells)) return
    this.stack = this.stack.slice(0, this.index + 1)
    this.stack.push(new Uint8Array(cells))
    if (this.stack.length > this.max) this.stack.shift()
    this.index = this.stack.length - 1
  }

  undo(): Uint8Array | null {
    this.pending = null
    if (!this.canUndo) return null
    this.index -= 1
    return new Uint8Array(this.stack[this.index])
  }

  redo(): Uint8Array | null {
    this.pending = null
    if (!this.canRedo) return null
    this.index += 1
    return new Uint8Array(this.stack[this.index])
  }
}

export function parseHex(hex: string): [number, number, number] | null {
  const h = hex.trim().replace(/^#/, '')
  if (h.length !== 6 || /[^0-9a-f]/i.test(h)) return null
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]
}

export function inkOn(hex: string): '#2C2A29' | '#FFFFFF' {
  const rgb = parseHex(hex)
  if (!rgb) return '#2C2A29'
  const lum = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
  return lum > 170 ? '#2C2A29' : '#FFFFFF'
}
