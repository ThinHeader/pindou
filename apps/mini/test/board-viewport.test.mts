import assert from 'node:assert/strict'
import { test } from 'node:test'
import { boardCssSize, fitCellSize } from '../src/utils/board-viewport.ts'

test('52, 78, and 104 boards fill the workspace width', () => {
  for (const side of [52, 78, 104]) {
    for (const width of [320, 375, 390, 414, 428, 430]) {
      const cell = fitCellSize(side, width)
      assert.equal(boardCssSize(side, cell), width)
    }
  }
})

test('a 104 board no longer stays at the old 3px cell and a side gap', () => {
  const cell = fitCellSize(104, 390)
  assert.ok(cell > 3)
  assert.equal(boardCssSize(104, cell), 390)
  assert.notEqual(boardCssSize(104, 3), 390)
})

test('the backing store stays inside the canvas cap on a wide workspace', () => {
  const cell = fitCellSize(52, 5000, 2048)
  assert.equal(cell, Math.floor(2048 / 52))
  assert.ok(boardCssSize(52, cell) <= 2048)
})

test('empty measurements still produce a finite cell', () => {
  const cell = fitCellSize(0, -10)
  assert.ok(cell > 0)
  assert.equal(Number.isFinite(cell), true)
  assert.equal(boardCssSize(0, cell) >= 1, true)
})
