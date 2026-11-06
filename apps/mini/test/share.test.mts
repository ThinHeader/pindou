import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import {
  HOME_PATH,
  SHARE_PAGES,
  SHARE_TITLE,
  buildShareAppMessage,
  buildShareTimeline,
  pathFromPage,
  queryFromPath,
  shareTitle,
  withQuery,
} from '../src/utils/share.ts'

const BANNED = ['AI', 'Seedream', 'seedream', '模型', '深度合成', '豆包', '方舟', '大模型']

const SHARE_PAGE_FILES = [
  'src/pages/home/index.vue',
  'src/pages/create/index.vue',
  'src/pages/create/preview.vue',
  'src/pages/library/index.vue',
  'src/pages/mine/index.vue',
  'src/pages/edit/index.vue',
  'src/pages/tutorials/index.vue',
  'src/pages/tutorials/detail.vue',
  'src/pages/draft/detail.vue',
]

test('default share card uses the product name and home', () => {
  const message = buildShareAppMessage()
  assert.deepEqual(message, { title: SHARE_TITLE, path: HOME_PATH })
  assert.equal(message.imageUrl, undefined)
  const timeline = buildShareTimeline()
  assert.deepEqual(timeline, { title: SHARE_TITLE, query: '' })
  for (const word of BANNED) {
    assert.equal(message.title.includes(word), false, word)
    assert.equal(shareTitle('窗边的猫').includes(word), false, word)
  }
})

test('share path keeps the current page query', () => {
  const message = buildShareAppMessage(
    { title: shareTitle('窗边的猫') },
    '/pages/draft/detail?id=abc',
  )
  assert.equal(message.title, '拼豆手账工坊 · 窗边的猫')
  assert.equal(message.path, '/pages/draft/detail?id=abc')
  assert.equal(buildShareTimeline({}, message.path).query, 'id=abc')
})

test('explicit path and image are passed through', () => {
  const message = buildShareAppMessage({
    path: 'pages/tutorials/index',
    imageUrl: ' https://example.com/cover.png ',
  })
  assert.equal(message.path, '/pages/tutorials/index')
  assert.equal(message.imageUrl, 'https://example.com/cover.png')
  const timeline = buildShareTimeline({
    path: '/pages/create/preview?id=1',
    query: '?id=override',
    imageUrl: 'https://example.com/cover.png',
  })
  assert.equal(timeline.query, 'id=override')
  assert.equal(timeline.imageUrl, 'https://example.com/cover.png')
})

test('untitled drafts stay on the product title', () => {
  assert.equal(shareTitle(''), SHARE_TITLE)
  assert.equal(shareTitle('   '), SHARE_TITLE)
  assert.equal(shareTitle('未命名草稿'), SHARE_TITLE)
  assert.equal(shareTitle(null), SHARE_TITLE)
})

test('withQuery drops empty params', () => {
  assert.equal(withQuery('/pages/draft/detail', { id: 'a b' }), '/pages/draft/detail?id=a%20b')
  assert.equal(withQuery('/pages/draft/detail', { id: '' }), '/pages/draft/detail')
  assert.equal(withQuery('/pages/tutorials/detail', { id: undefined }), '/pages/tutorials/detail')
  assert.equal(queryFromPath('/pages/home/index'), '')
  assert.equal(queryFromPath('/pages/draft/detail?id=1&from=share'), 'id=1&from=share')
})

test('page path prefers the launched fullPath', () => {
  assert.equal(pathFromPage(undefined), HOME_PATH)
  assert.equal(pathFromPage({ route: 'login' }), HOME_PATH)
  assert.equal(
    pathFromPage({ $page: { fullPath: '/pages/draft/detail?id=9' }, route: 'pages/home/index' }),
    '/pages/draft/detail?id=9',
  )
  assert.equal(
    pathFromPage({ route: '/pages/create/preview', options: { id: '42', empty: '' } }),
    '/pages/create/preview?id=42',
  )
})

test('listed routes exist and each page registers share', () => {
  const pagesJson = JSON.parse(readFileSync(new URL('../src/pages.json', import.meta.url), 'utf8')) as {
    pages: { path: string }[]
  }
  const routes = new Set(pagesJson.pages.map((page) => `/${page.path}`))
  for (const path of Object.values(SHARE_PAGES)) {
    assert.equal(routes.has(path), true, path)
  }
  for (const file of SHARE_PAGE_FILES) {
    const source = readFileSync(new URL(`../${file}`, import.meta.url), 'utf8')
    assert.match(source, /usePageShare\(/)
    assert.match(source, /onShareAppMessage/)
    assert.match(source, /onShareTimeline/)
    assert.match(source, /onCopyUrl/)
  }
})
