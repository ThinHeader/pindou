import assert from 'node:assert/strict'
import { test } from 'node:test'
import { paintBoard, type PaintCtx } from '../src/utils/board-paint.ts'
import {
  CellHistory,
  canRevise,
  cellsEqual,
  cellsWithinPalette,
  normalizeDraftId,
  decodeCellsB64,
  dominantIndex,
  encodeCellsB64,
  floodFill,
  inkOn,
  lineCells,
  paintIndex,
  recount,
} from '../src/utils/pattern.ts'

test('base64 roundtrip matches palette-index bytes', () => {
  assert.equal(encodeCellsB64(new Uint8Array()), '')
  assert.equal(encodeCellsB64(Uint8Array.of(0, 1)), 'AAE=')
  assert.deepEqual([...decodeCellsB64('AAE=')], [0, 1])
  const cells = new Uint8Array(52 * 52)
  for (let i = 0; i < cells.length; i += 1) cells[i] = i % 7
  const encoded = encodeCellsB64(cells)
  assert.deepEqual([...decodeCellsB64(encoded, 52 * 52)], [...cells])
  assert.equal(Buffer.from(encoded, 'base64').compare(Buffer.from(cells)), 0)
  assert.throws(() => decodeCellsB64('@@@'), /图纸数据有误/)
  assert.throws(() => decodeCellsB64('AAE=', 4), /不一致/)
})

test('brush, line, fill, and counts stay on palette indexes', () => {
  const cells = Uint8Array.from([0, 0, 1, 0, 1, 0, 1, 0, 0])
  assert.equal(paintIndex(cells, 3, 0, 0, 0), false)
  assert.equal(paintIndex(cells, 3, 1, 0, 2), true)
  assert.equal(cells[1], 2)
  assert.deepEqual(lineCells(0, 0, 2, 0), [
    [0, 0],
    [1, 0],
    [2, 0],
  ])
  const board = Uint8Array.from([0, 1, 0, 1, 0, 1, 0, 1, 0])
  const dirty = floodFill(board, 3, 0, 0, 2)
  assert.deepEqual(dirty, [{ x: 0, y: 0 }])
  assert.equal(board[0], 2)
  const block = new Uint8Array(9)
  assert.equal(floodFill(block, 3, 1, 1, 4).length, 9)
  assert.equal(floodFill(block, 3, 1, 1, 4).length, 0)
  const counts = recount(Uint8Array.from([0, 1, 1, 0]), 3)
  assert.deepEqual(counts, [2, 2, 0])
  assert.equal(dominantIndex(counts), 0)
  assert.equal(dominantIndex([1, 5, 5]), 1)
})

test('undo restores the previous stroke and drops a no-op', () => {
  const history = new CellHistory(3)
  const cells = Uint8Array.of(0, 0)
  history.reset(cells)
  history.begin(cells)
  history.end(cells)
  assert.equal(history.canUndo, false)
  history.begin(cells)
  cells[0] = 1
  history.end(cells)
  assert.equal(history.canUndo, true)
  assert.deepEqual([...(history.undo() as Uint8Array)], [0, 0])
  assert.deepEqual([...(history.redo() as Uint8Array)], [1, 0])
  history.undo()
  const next = Uint8Array.of(0, 0)
  history.begin(next)
  next[1] = 2
  history.end(next)
  assert.equal(history.canRedo, false)
  assert.deepEqual([...(history.undo() as Uint8Array)], [0, 0])
  assert.equal(cellsEqual(Uint8Array.of(1), Uint8Array.of(1, 0)), false)
})

test('cells stay inside the palette without spreading the buffer', () => {
  assert.equal(cellsWithinPalette(Uint8Array.of(0, 1, 2), 3), true)
  assert.equal(cellsWithinPalette(Uint8Array.of(0, 3), 3), false)
  assert.equal(cellsWithinPalette(Uint8Array.of(0), 0), false)
  const big = new Uint8Array(104 * 104)
  big[big.length - 1] = 8
  assert.equal(cellsWithinPalette(big, 9), true)
  big[10] = 9
  assert.equal(cellsWithinPalette(big, 9), false)
})

test('draft ids from the route are trimmed and empty placeholders are dropped', () => {
  assert.equal(normalizeDraftId('  abc  '), 'abc')
  assert.equal(normalizeDraftId(undefined), '')
  assert.equal(normalizeDraftId('undefined'), '')
  assert.equal(normalizeDraftId('null'), '')
  assert.equal(normalizeDraftId(''), '')
})

test('only the owner draft can be revised', () => {
  assert.equal(canRevise({ badge: '我的', is_official: false, is_template: false }), true)
  assert.equal(canRevise({ badge: '官方', is_official: true, is_template: false }), false)
  assert.equal(canRevise({ badge: '收藏', is_official: false, is_template: false }), false)
  assert.equal(canRevise({ badge: '我的', is_official: false, is_template: true }), false)
  assert.equal(inkOn('#F7F1EA'), '#2C2A29')
  assert.equal(inkOn('#2C2A29'), '#FFFFFF')
})

test('full paint fills every cell and a dirty paint does not clear the board', () => {
  const calls: string[] = []
  const ctx: PaintCtx = {
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 1,
    font: '',
    textAlign: '',
    textBaseline: '',
    clearRect() {
      calls.push('clear')
    },
    fillRect(x, y, w, h) {
      calls.push(`fill:${this.fillStyle}:${x},${y},${w},${h}`)
    },
    beginPath() {},
    moveTo() {},
    lineTo() {},
    stroke() {
      calls.push('stroke')
    },
    fillText(text) {
      calls.push(`text:${text}`)
    },
  }
  const board = {
    cells: Uint8Array.of(0, 1, 1, 0),
    width: 2,
    cell: 16,
    palette: [
      { code: 'A1', hex: '#FF6B5B' },
      { code: 'W1', hex: '#F7F1EA' },
    ],
    showCodes: true,
  }
  paintBoard(ctx, board, null)
  assert.equal(calls.filter((item) => item.startsWith('text:')).length, 4)
  assert.ok(calls.some((item) => item.includes('#FF6B5B')))
  calls.length = 0
  paintBoard(ctx, board, [{ x: 1, y: 0 }])
  assert.equal(calls.filter((item) => item.startsWith('text:')).length, 1)
  assert.equal(calls[0].startsWith('fill:#F7F3F0:0,0'), false)
})
