import assert from 'node:assert/strict'
import { test } from 'node:test'
import { draftCardsFrom, errorText, readableError, unwrapBody } from '../src/api/payload.ts'

const cat = { id: '11111111-1111-4111-8111-111111111111', title: '窗边的猫', board_size: 52 }
const dog = { id: '22222222-2222-4222-8222-222222222222', title: '门口的狗', board_size: 52 }

test('draft lists are read from items, a bare array, or a nested message', () => {
  assert.deepEqual(draftCardsFrom({ items: [cat], next_cursor: null })?.map((item) => item.title), ['窗边的猫'])
  assert.deepEqual(draftCardsFrom([cat, dog])?.map((item) => item.title), ['窗边的猫', '门口的狗'])
  assert.deepEqual(draftCardsFrom({ data: { items: [cat] } })?.map((item) => item.id), [cat.id])
  assert.deepEqual(draftCardsFrom({ message: [cat, dog] })?.length, 2)
  assert.deepEqual(draftCardsFrom({ items: [] }), [])
  assert.equal(draftCardsFrom({ message: '请先登录', error: 'unauthorized' }), null)
  assert.equal(draftCardsFrom({ message: [{ foo: 1 }, { foo: 2 }] }), null)
  assert.deepEqual(
    draftCardsFrom(JSON.stringify({ items: [cat] }))?.map((item) => item.title),
    ['窗边的猫'],
  )
})

test('error text stays a readable string', () => {
  assert.equal(errorText({ message: '请先登录' }, '稿件加载失败'), '请先登录')
  assert.equal(errorText({ message: [{ foo: 1 }, { bar: 2 }] }, '稿件加载失败'), '稿件加载失败')
  assert.equal(errorText([cat, dog], '稿件加载失败'), '稿件加载失败')
  assert.equal(errorText('[object Object],[object Object]', '稿件加载失败'), '稿件加载失败')
  assert.equal(errorText({ message: ['色板无效', '尺寸不对'] }, '稿件加载失败'), '色板无效；尺寸不对')
  assert.equal(errorText({ message: [{ message: '分类无效' }] }, '稿件加载失败'), '分类无效')
  assert.equal(errorText({ message: [cat, dog] }, '稿件加载失败'), '稿件加载失败')
  assert.equal(readableError({ message: '请求失败' }, '稿件加载失败'), '稿件加载失败')
  assert.equal(readableError({ message: '请先登录' }, '稿件加载失败'), '请先登录')
  assert.deepEqual(unwrapBody('{"items":[]}'), { items: [] })
})
