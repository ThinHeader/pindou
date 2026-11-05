import assert from 'node:assert/strict'
import { test } from 'node:test'
import { boardViewport } from '../src/utils/board-viewport.ts'

function centers(canvas: number, viewWidth: number, viewHeight: number) {
  const frame = boardViewport({ canvas, viewWidth, viewHeight })
  const dx = frame.scrollLeft + viewWidth / 2 - (frame.offsetX + canvas / 2)
  const dy = frame.scrollTop + viewHeight / 2 - (frame.offsetY + canvas / 2)
  return { frame, dx, dy }
}

test('a canvas smaller than the stage is centered without scrolling', () => {
  const { frame, dx, dy } = centers(312, 390, 420)
  assert.equal(frame.scrollLeft, 0)
  assert.equal(frame.scrollTop, 0)
  assert.equal(frame.offsetX, 39)
  assert.equal(frame.offsetY, 54)
  assert.ok(Math.abs(dx) <= 0.5)
  assert.ok(Math.abs(dy) <= 0.5)
})

test('a canvas larger than the stage is centered by the scroll offset', () => {
  const { frame, dx, dy } = centers(520, 390, 360)
  assert.equal(frame.offsetX, 0)
  assert.equal(frame.offsetY, 0)
  assert.equal(frame.scrollLeft, 65)
  assert.equal(frame.scrollTop, 80)
  assert.ok(Math.abs(dx) <= 0.5)
  assert.ok(Math.abs(dy) <= 0.5)
})

test('each axis centers on its own when only one side overflows', () => {
  const { frame, dx, dy } = centers(500, 300, 600)
  assert.equal(frame.offsetX, 0)
  assert.equal(frame.scrollLeft, 100)
  assert.equal(frame.offsetY, 50)
  assert.equal(frame.scrollTop, 0)
  assert.ok(Math.abs(dx) <= 0.5)
  assert.ok(Math.abs(dy) <= 0.5)
})
