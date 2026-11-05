export interface PaintCtx {
  fillStyle: string
  strokeStyle: string
  lineWidth: number
  font: string
  textAlign: string
  textBaseline: string
  clearRect(x: number, y: number, w: number, h: number): void
  fillRect(x: number, y: number, w: number, h: number): void
  beginPath(): void
  moveTo(x: number, y: number): void
  lineTo(x: number, y: number): void
  stroke(): void
  fillText(text: string, x: number, y: number): void
  setTransform?(a: number, b: number, c: number, d: number, e: number, f: number): void
}

export interface BoardSwatch {
  code: string
  hex: string
}

export interface BoardPaint {
  cells: Uint8Array
  width: number
  cell: number
  palette: BoardSwatch[]
  showCodes: boolean
}

function inkOn(hex: string): string {
  const h = hex.trim().replace(/^#/, '')
  if (h.length !== 6 || /[^0-9a-f]/i.test(h)) return '#2C2A29'
  const lum =
    0.299 * parseInt(h.slice(0, 2), 16) +
    0.587 * parseInt(h.slice(2, 4), 16) +
    0.114 * parseInt(h.slice(4, 6), 16)
  return lum > 170 ? '#2C2A29' : '#FFFFFF'
}

const BOARD = '#F7F3F0'
const GRID = 'rgba(44,42,41,0.16)'
const MAJOR = 'rgba(255,107,91,0.55)'

export function codeVisible(cell: number): boolean {
  return cell >= 16
}

/** dirty 为空时整板重画。笔画过程只补改过的格子，104 板不用每帧铺满一万格。 */
export function paintBoard(ctx: PaintCtx, board: BoardPaint, dirty: Array<{ x: number; y: number }> | null) {
  const { width, cell, cells, palette, showCodes } = board
  const side = width * cell
  if (!dirty) {
    ctx.fillStyle = BOARD
    ctx.fillRect(0, 0, side, side)
    for (let y = 0; y < width; y += 1) {
      for (let x = 0; x < width; x += 1) drawCell(ctx, board, x, y)
    }
  } else {
    for (const spot of dirty) drawCell(ctx, board, spot.x, spot.y)
  }
  if (cell >= 6) drawGrid(ctx, width, cell)
  if (showCodes && codeVisible(cell)) {
    const spots = dirty ?? allSpots(width)
    for (const spot of spots) drawCode(ctx, cells, palette, width, cell, spot.x, spot.y)
  }
}

function allSpots(width: number): Array<{ x: number; y: number }> {
  const spots: Array<{ x: number; y: number }> = []
  for (let y = 0; y < width; y += 1) {
    for (let x = 0; x < width; x += 1) spots.push({ x, y })
  }
  return spots
}

function drawCell(ctx: PaintCtx, board: BoardPaint, x: number, y: number) {
  const { cell, cells, palette, width } = board
  if (x < 0 || y < 0 || x >= width || y >= width) return
  const swatch = palette[cells[y * width + x]]
  const pad = cell >= 8 ? 1 : 0
  const originX = x * cell
  const originY = y * cell
  ctx.fillStyle = BOARD
  ctx.fillRect(originX, originY, cell, cell)
  ctx.fillStyle = swatch?.hex || '#000000'
  ctx.fillRect(originX + pad, originY + pad, Math.max(1, cell - pad * 2), Math.max(1, cell - pad * 2))
}

function drawCode(
  ctx: PaintCtx,
  cells: Uint8Array,
  palette: BoardSwatch[],
  width: number,
  cell: number,
  x: number,
  y: number,
) {
  const swatch = palette[cells[y * width + x]]
  if (!swatch) return
  ctx.fillStyle = inkOn(swatch.hex)
  ctx.font = `bold ${Math.max(8, Math.floor(cell * 0.42))}px sans-serif`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(swatch.code, x * cell + cell / 2, y * cell + cell / 2)
}

function drawGrid(ctx: PaintCtx, width: number, cell: number) {
  const side = width * cell
  ctx.beginPath()
  ctx.strokeStyle = GRID
  ctx.lineWidth = 1
  for (let i = 0; i <= width; i += 1) {
    const p = i * cell + 0.5
    ctx.moveTo(p, 0)
    ctx.lineTo(p, side)
    ctx.moveTo(0, p)
    ctx.lineTo(side, p)
  }
  ctx.stroke()
  if (cell < 8) return
  ctx.beginPath()
  ctx.strokeStyle = MAJOR
  for (let i = 0; i <= width; i += 10) {
    const p = i * cell + 0.5
    ctx.moveTo(p, 0)
    ctx.lineTo(p, side)
    ctx.moveTo(0, p)
    ctx.lineTo(side, p)
  }
  ctx.stroke()
}
