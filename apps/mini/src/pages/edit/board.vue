<script setup lang="ts">
import { onLoad, onShow } from '@dcloudio/uni-app'
import { computed, getCurrentInstance, nextTick, ref } from 'vue'
import { draftsApi } from '../../api/http'
import { readableError } from '../../api/payload'
import type { PaletteColor } from '../../api/types'
import { paintBoard, codeVisible, type PaintCtx } from '../../utils/board-paint'
import {
  CellHistory,
  canRevise,
  cellsEqual,
  cellsWithinPalette,
  decodeCellsB64,
  dominantIndex,
  encodeCellsB64,
  floodFill,
  lineCells,
  normalizeDraftId,
  paintIndex,
  recount,
} from '../../utils/pattern'
import { askText, checkedDraftTitle, showError, showTip } from '../../utils/ui'

type Tool = 'pencil' | 'eraser' | 'picker' | 'fill' | 'pan'
type Spot = { x: number; y: number }

interface MiniCanvas {
  width: number
  height: number
  getContext(kind: '2d'): PaintCtx
}

interface TouchPoint {
  clientX?: number
  clientY?: number
  x?: number
  y?: number
}

const proxy = getCurrentInstance()?.proxy
const tools: Array<{ id: Tool; label: string }> = [
  { id: 'pencil', label: '画笔' },
  { id: 'eraser', label: '橡皮' },
  { id: 'picker', label: '取色' },
  { id: 'fill', label: '填充' },
  { id: 'pan', label: '移动' },
]

const pageHeight = ref(600)
const stageHeight = ref(280)
const loading = ref(true)
const ready = ref(false)
const saving = ref(false)
const statusTitle = ref('正在读取稿件…')
const statusHint = ref('')
const canRetry = ref(false)
const title = ref('')
const tool = ref<Tool>('pencil')
const showCodes = ref(false)
const showMat = ref(false)
const cellPx = ref(6)
const colorIndex = ref(0)
const eraseIndex = ref(0)
const palette = ref<PaletteColor[]>([])
const canUndo = ref(false)
const canRedo = ref(false)
const unsaved = ref(false)
const scrollLeft = ref(0)
const scrollTop = ref(0)
const boardSize = ref(52)

const history = new CellHistory()
let draftId = ''
let requestedId = ''
let cells: Uint8Array<ArrayBufferLike> = new Uint8Array()
let saved: Uint8Array<ArrayBufferLike> = new Uint8Array()
let canvas: MiniCanvas | null = null
let ctx: PaintCtx | null = null
let rect = { left: 0, top: 0 }
let stroking = false
let last: [number, number] | null = null
let panLast: { x: number; y: number } | null = null
let viewWidth = 375

const cssSize = computed(() => boardSize.value * cellPx.value)
const pegs = computed(() => palette.value.reduce((sum, item) => sum + item.count, 0))
const usedColors = computed(() => palette.value.filter((item) => item.count > 0).length)
const ranked = computed(() =>
  palette.value
    .map((item, index) => ({ ...item, index }))
    .sort((a, b) => b.count - a.count || a.index - b.index),
)
const current = computed(() => palette.value[colorIndex.value])
const base = computed(() => palette.value[eraseIndex.value])
const codesReady = computed(() => codeVisible(cellPx.value))

onLoad((query) => {
  try {
    const info = uni.getSystemInfoSync()
    pageHeight.value = info.windowHeight
    viewWidth = info.windowWidth
  } catch {
    pageHeight.value = 600
  }
  void boot(readId(query as Record<string, unknown> | undefined))
})

onShow(() => {
  if (ready.value || loading.value || requestedId) return
  const id = readId(null)
  if (id) void boot(id)
})

function readId(query?: Record<string, unknown> | null): string {
  const fromQuery = normalizeDraftId(query?.id)
  if (fromQuery) return fromQuery
  try {
    const pages = getCurrentPages() as Array<{
      options?: Record<string, unknown>
      $page?: { options?: Record<string, unknown>; fullPath?: string }
    }>
    const current = pages[pages.length - 1]
    const bag = current?.options || current?.$page?.options
    const fromOptions = normalizeDraftId(bag?.id)
    if (fromOptions) return fromOptions
    const full = current?.$page?.fullPath || ''
    const matched = /[?&]id=([^&]+)/.exec(full)
    if (!matched) return ''
    try {
      return normalizeDraftId(decodeURIComponent(matched[1]))
    } catch {
      return normalizeDraftId(matched[1])
    }
  } catch {
    return ''
  }
}

function showIdle(title: string, hint: string, retry: boolean) {
  statusTitle.value = title
  statusHint.value = hint
  canRetry.value = retry
  ready.value = false
}

function retryLoad() {
  if (!requestedId || loading.value) return
  void boot(requestedId)
}

function goLibrary() {
  uni.switchTab({ url: '/pages/library/index' })
}

function goPick() {
  uni.switchTab({ url: '/pages/edit/index' })
}

async function boot(id: string) {
  requestedId = id
  if (!id) {
    loading.value = false
    showIdle('还没有选中要改的稿', '从稿件库选一张自己的稿，再进来改格子。', false)
    return
  }
  loading.value = true
  showIdle('正在读取稿件…', '', false)
  try {
    const detail = await draftsApi.detail(id)
    if (!canRevise(detail)) {
      const title = detail.is_official || detail.is_template ? '官方稿不能改' : '只能改自己的稿'
      showTip(title)
      showIdle(title, '请换一张自己出的稿。', false)
      return
    }
    const swatches = Array.isArray(detail.palette) ? detail.palette : []
    if (!detail.cells_b64 || !detail.board_size || !swatches.length) {
      throw new Error('这份稿还没有格子，不能改')
    }
    const side = detail.board_size
    cells = decodeCellsB64(detail.cells_b64, side * side)
    if (!cellsWithinPalette(cells, swatches.length)) {
      throw new Error('这份稿的色板和格子对不上，暂时不能改')
    }
    draftId = detail.id
    title.value = detail.title
    boardSize.value = side
    palette.value = swatches.map((item) => ({ ...item }))
    colorIndex.value = 0
    eraseIndex.value = dominantIndex(recount(cells, palette.value.length))
    saved = new Uint8Array(cells)
    history.reset(cells)
    cellPx.value = fitCell(side)
    syncHist()
    ready.value = true
    await nextTick()
    measureStage()
    await initCanvas()
  } catch (e) {
    showIdle(readableError(e, '稿件读不出来'), '可以重试，或回到稿件库另选一张。', true)
    showError(e, '稿件读不出来')
  } finally {
    loading.value = false
  }
}

function fitCell(side: number): number {
  const avail = Math.max(120, viewWidth - 16)
  const fitted = Math.floor(avail / side)
  return Math.min(zoomCap(side), Math.max(3, Math.min(fitted, 10)))
}

function zoomCap(side: number): number {
  return Math.max(3, Math.floor(2048 / side))
}

function zoom(dir: number) {
  const next = Math.min(zoomCap(boardSize.value), Math.max(3, cellPx.value + dir * 2))
  if (next === cellPx.value) {
    showTip(dir > 0 ? '已经放到最大' : '已经缩到最小')
    return
  }
  cellPx.value = next
  scrollLeft.value = 0
  scrollTop.value = 0
  nextTick(() => resizeAndPaint())
}

function toggleCodes() {
  showCodes.value = !showCodes.value
  if (showCodes.value && !codesReady.value) showTip('放大后色号才会标在格子里')
  paint(null)
}

function toggleMat() {
  showMat.value = !showMat.value
  nextTick(() => measureStage())
}

function measureStage() {
  if (!proxy) return
  const q = uni.createSelectorQuery().in(proxy)
  q.select('.page').boundingClientRect()
  q.select('.toolbar').boundingClientRect()
  q.select('.dock').boundingClientRect()
  q.exec((res) => {
    const rows = (res || []) as Array<{ height?: number } | null>
    const pageH = rows[0]?.height || pageHeight.value
    const toolH = rows[1]?.height || 0
    const dockH = rows[2]?.height || 0
    if (toolH && dockH) stageHeight.value = Math.max(160, pageH - toolH - dockH)
  })
}

function grabNode(): Promise<MiniCanvas | null> {
  return new Promise((resolve) => {
    if (!proxy) {
      resolve(null)
      return
    }
    uni
      .createSelectorQuery()
      .in(proxy)
      .select('#boardCanvas')
      .node((result: { node?: MiniCanvas } | null) => {
        resolve(result?.node ?? null)
      })
      .exec()
  })
}

async function initCanvas(attempt = 0) {
  const node = await grabNode()
  if (!node) {
    if (attempt < 6) {
      await wait(40)
      return initCanvas(attempt + 1)
    }
    showTip('画板没有准备好，请返回重进')
    return
  }
  canvas = node
  ctx = node.getContext('2d')
  resizeAndPaint()
}

function resizeAndPaint() {
  if (!canvas || !ctx) return
  const size = cssSize.value
  canvas.width = size
  canvas.height = size
  paint(null)
}

function paint(dirty: Spot[] | null) {
  if (!ctx) return
  paintBoard(
    ctx,
    {
      cells,
      width: boardSize.value,
      cell: cellPx.value,
      palette: palette.value,
      showCodes: showCodes.value,
    },
    dirty,
  )
}

function syncCounts() {
  const counts = recount(cells, palette.value.length)
  palette.value = palette.value.map((item, index) => ({ ...item, count: counts[index] || 0 }))
}

function syncHist() {
  canUndo.value = history.canUndo
  canRedo.value = history.canRedo
  unsaved.value = !cellsEqual(cells, saved)
}

function undo() {
  const next = history.undo()
  if (!next) {
    showTip('已经是最初的格子')
    return
  }
  cells = next
  syncCounts()
  syncHist()
  paint(null)
}

function redo() {
  const next = history.redo()
  if (!next) {
    showTip('已经是最新的格子')
    return
  }
  cells = next
  syncCounts()
  syncHist()
  paint(null)
}

function pickSwatch(index: number) {
  colorIndex.value = index
  if (tool.value === 'pan') tool.value = 'pencil'
}

function setBase() {
  eraseIndex.value = colorIndex.value
  const swatch = palette.value[eraseIndex.value]
  showTip(swatch ? `底色改为 ${swatch.code}` : '已设为底色')
}

function measureCanvas(): Promise<void> {
  return new Promise((resolve) => {
    if (!proxy) {
      resolve()
      return
    }
    uni
      .createSelectorQuery()
      .in(proxy)
      .select('#boardCanvas')
      .boundingClientRect((result) => {
        const info = Array.isArray(result) ? result[0] : result
        if (info?.width) rect = { left: info.left || 0, top: info.top || 0 }
        resolve()
      })
      .exec()
  })
}

function pointOf(e: unknown): { x: number; y: number } | null {
  const ev = e as { touches?: TouchPoint[]; changedTouches?: TouchPoint[] }
  const t = ev.touches?.[0] || ev.changedTouches?.[0]
  if (!t) return null
  const x = t.clientX ?? t.x
  const y = t.clientY ?? t.y
  if (x == null || y == null) return null
  return { x, y }
}

function cellAt(p: { x: number; y: number }): [number, number] {
  return [Math.floor((p.x - rect.left) / cellPx.value), Math.floor((p.y - rect.top) / cellPx.value)]
}

async function onStart(e: unknown) {
  const p = pointOf(e)
  if (!p) return
  if (tool.value === 'pan') {
    panLast = p
    return
  }
  await measureCanvas()
  const cell = cellAt(p)
  if (tool.value === 'picker') {
    pickFrom(cell[0], cell[1])
    return
  }
  if (tool.value === 'fill') {
    fillFrom(cell[0], cell[1])
    return
  }
  stroking = true
  last = cell
  history.begin(cells)
  applyLine(cell, cell)
}

function onMove(e: unknown) {
  const p = pointOf(e)
  if (!p) return
  if (tool.value === 'pan') {
    if (!panLast) return
    const maxLeft = Math.max(0, cssSize.value - viewWidth)
    const maxTop = Math.max(0, cssSize.value - stageHeight.value)
    scrollLeft.value = clamp(scrollLeft.value - (p.x - panLast.x), 0, maxLeft)
    scrollTop.value = clamp(scrollTop.value - (p.y - panLast.y), 0, maxTop)
    panLast = p
    return
  }
  if (!stroking || !last) return
  const cell = cellAt(p)
  if (cell[0] === last[0] && cell[1] === last[1]) return
  applyLine(last, cell)
  last = cell
}

function onEnd() {
  panLast = null
  if (!stroking) return
  stroking = false
  last = null
  history.end(cells)
  syncCounts()
  syncHist()
}

function applyLine(from: [number, number], to: [number, number]) {
  const index = tool.value === 'eraser' ? eraseIndex.value : colorIndex.value
  const dirty: Spot[] = []
  for (const [x, y] of lineCells(from[0], from[1], to[0], to[1])) {
    if (paintIndex(cells, boardSize.value, x, y, index)) dirty.push({ x, y })
  }
  if (dirty.length) paint(dirty)
}

function pickFrom(x: number, y: number) {
  if (x < 0 || y < 0 || x >= boardSize.value || y >= boardSize.value) return
  const idx = cells[y * boardSize.value + x]
  colorIndex.value = idx
  tool.value = 'pencil'
  const swatch = palette.value[idx]
  showTip(swatch ? `取色 ${swatch.code} ${swatch.name}` : '已取色')
}

function fillFrom(x: number, y: number) {
  history.begin(cells)
  const dirty = floodFill(cells, boardSize.value, x, y, colorIndex.value)
  history.end(cells)
  syncCounts()
  syncHist()
  if (!dirty.length) {
    showTip('这里已经是这个颜色')
    return
  }
  paint(dirty.length > 480 ? null : dirty)
}

async function save(saveAs: boolean) {
  if (saving.value || !draftId) return
  if (!saveAs && !unsaved.value) {
    showTip('没有需要保存的改动')
    return
  }
  let nextTitle: string | undefined
  if (saveAs) {
    const typed = await askText('另存为新稿', `${title.value} 副本`, '新稿名称')
    if (typed === null) return
    if (typed) {
      const checked = checkedDraftTitle(typed)
      if (!checked) return
      nextTitle = checked
    }
  }
  saving.value = true
  try {
    const detail = await draftsApi.savePattern(draftId, {
      cells_b64: encodeCellsB64(cells),
      ...(saveAs ? { save_as: true, title: nextTitle } : {}),
    })
    draftId = detail.id
    title.value = detail.title
    palette.value = detail.palette.map((item) => ({ ...item }))
    saved = new Uint8Array(cells)
    syncHist()
    showTip(saveAs ? '已另存为新稿' : '已覆盖保存')
  } catch (e) {
    showError(e, '保存失败')
  } finally {
    saving.value = false
  }
}

function clamp(n: number, min: number, max: number) {
  return Math.max(min, Math.min(max, n))
}

function wait(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
</script>

<template>
  <view v-if="ready" class="page" :style="{ height: pageHeight + 'px' }">
    <view class="toolbar">
      <view
        v-for="item in tools"
        :key="item.id"
        class="tool"
        :class="{ on: tool === item.id }"
        @click="tool = item.id"
      ><text>{{ item.label }}</text></view>
      <view class="tool" :class="{ off: !canUndo }" @click="undo"><text>撤销</text></view>
      <view class="tool" :class="{ off: !canRedo }" @click="redo"><text>重做</text></view>
    </view>
    <scroll-view
      class="stage"
      scroll-x
      scroll-y
      :show-scrollbar="true"
      :scroll-with-animation="false"
      :scroll-left="scrollLeft"
      :scroll-top="scrollTop"
      :style="{ height: stageHeight + 'px' }"
    >
      <canvas
        id="boardCanvas"
        canvas-id="boardCanvas"
        type="2d"
        class="board"
        disable-scroll
        :style="{ width: cssSize + 'px', height: cssSize + 'px' }"
        @touchstart="onStart"
        @touchmove.stop.prevent="onMove"
        @touchend="onEnd"
        @touchcancel="onEnd"
      />
    </scroll-view>
    <view class="dock">
      <view class="info">
        <view class="now">
          <view class="dot" :style="{ background: current?.hex || '#fff' }" />
          <text class="now-name">{{ current ? current.code + ' ' + current.name : '选色' }}</text>
        </view>
        <text class="meta">{{ boardSize }}×{{ boardSize }} · {{ usedColors }} 色 · {{ pegs }} 颗</text>
        <text v-if="unsaved" class="dirty">未保存</text>
      </view>
      <view v-if="tool === 'eraser' && base" class="erase-hint">
        <text>橡皮铺回底色 {{ base.code }} {{ base.name }}</text>
        <text class="link" @click="setBase">用当前色当底色</text>
      </view>
      <scroll-view class="palette" scroll-x :show-scrollbar="false">
        <view class="palette-row">
          <view
            v-for="(item, index) in palette"
            :key="item.code + index"
            class="chip"
            :class="{ on: index === colorIndex, empty: item.count === 0 }"
            @click="pickSwatch(index)"
          >
            <view class="chip-dot" :style="{ background: item.hex }" />
            <text class="chip-code">{{ item.code }}</text>
            <text class="chip-cnt">{{ item.count }}</text>
          </view>
        </view>
      </scroll-view>
      <view class="toggles">
        <text class="toggle" :class="{ on: showCodes }" @click="toggleCodes">色号</text>
        <text class="toggle" @click="zoom(-1)">缩小</text>
        <text class="toggle" @click="zoom(1)">放大</text>
        <text class="toggle" @click="toggleMat">用料 {{ showMat ? '收起' : '展开' }}</text>
      </view>
      <scroll-view v-if="showMat" scroll-y class="mat">
        <view v-for="item in ranked" :key="item.code + item.index" class="mat-row" @click="pickSwatch(item.index)">
          <view class="dot" :style="{ background: item.hex }" />
          <text class="mat-code">{{ item.code }}</text>
          <text class="mat-name">{{ item.name }}</text>
          <text class="mat-cnt">{{ item.count }}</text>
        </view>
      </scroll-view>
      <view class="saves">
        <view class="btn ghost" @click="save(true)"><text>{{ saving ? '保存中' : '另存为新稿' }}</text></view>
        <view class="btn primary" @click="save(false)"><text>{{ saving ? '保存中' : '覆盖保存' }}</text></view>
      </view>
      <text class="draft-name">{{ title }}</text>
    </view>
  </view>
  <view v-else class="page idle">
    <!-- 微信里 flex 子项上的裸文本高度为 0，只剩虚线框。文案放进 text，再包一层 view。 -->
    <view class="empty">
      <view class="line"><text class="empty-title">{{ statusTitle }}</text></view>
      <view v-if="statusHint" class="line"><text class="empty-hint">{{ statusHint }}</text></view>
      <view class="idle-actions">
        <view v-if="canRetry" class="btn primary" @click="retryLoad"><text>重试</text></view>
        <view class="btn ghost" @click="goLibrary"><text>去稿件库</text></view>
        <view class="btn ghost" @click="goPick"><text>返回选稿</text></view>
      </view>
    </view>
  </view>
</template>

<style scoped lang="scss">
.page { display: flex; flex-direction: column; background: $bg; }
.idle { display: block; box-sizing: border-box; padding: 24rpx; min-height: 100%; }
.line { width: 100%; }
.empty-title { display: block; color: $ink; font-size: 30rpx; font-weight: 700; line-height: 1.5; }
.empty-hint { display: block; margin-top: 8rpx; color: $ink-2; font-size: 26rpx; line-height: 1.5; }
.idle-actions { display: flex; flex-direction: column; gap: 12rpx; width: 100%; margin-top: 20rpx; }
.idle-actions .btn { flex: none; width: 100%; }
.toolbar {
  display: flex; gap: 8rpx; padding: 8rpx 12rpx; background: $card; border-bottom: 1rpx solid $line;
}
.tool {
  padding: 8rpx 12rpx; border-radius: 999rpx; background: $brand-lighter; color: $ink-2;
  font-size: 22rpx; font-weight: 700;
}
.tool text { font-size: 22rpx; font-weight: 700; color: $ink-2; }
.tool.on { background: $brand; }
.tool.on text { color: #fff; }
.tool.off { opacity: 0.4; }
.stage { width: 100%; background: #efe8e2; }
.board { display: block; }
.dock { background: $card; border-top: 1rpx solid $line; padding: 8rpx 16rpx 16rpx; }
.info { display: flex; align-items: center; gap: 12rpx; min-width: 0; }
.now { display: flex; align-items: center; gap: 8rpx; max-width: 42%; }
.now-name { font-size: 22rpx; font-weight: 700; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.meta { flex: 1; color: $ink-3; font-size: 20rpx; text-align: right; }
.dirty { color: $brand-dark; font-size: 20rpx; font-weight: 700; }
.dot, .chip-dot { width: 22rpx; height: 22rpx; border-radius: 50%; border: 1rpx solid rgba(0, 0, 0, 0.08); flex-shrink: 0; }
.erase-hint {
  display: flex; justify-content: space-between; margin-top: 8rpx; color: $ink-2; font-size: 22rpx;
}
.link { color: $brand-dark; font-weight: 700; }
.palette { width: 100%; margin-top: 8rpx; white-space: nowrap; }
.palette-row { display: inline-flex; gap: 8rpx; padding-bottom: 4rpx; }
.chip {
  display: inline-flex; flex-direction: column; align-items: center; width: 72rpx;
  padding: 6rpx 0; border-radius: 16rpx; border: 1rpx solid transparent; background: $bg;
}
.chip.on { border-color: $brand; background: $brand-lighter; }
.chip.empty { opacity: 0.45; }
.chip-code { font-size: 18rpx; font-weight: 700; line-height: 1.2; }
.chip-cnt { font-size: 16rpx; color: $ink-3; line-height: 1.2; }
.toggles { display: flex; gap: 8rpx; margin-top: 8rpx; }
.toggle {
  padding: 6rpx 16rpx; border-radius: 999rpx; background: $bg; color: $ink-2; font-size: 22rpx;
}
.toggle.on { background: $ink; color: #fff; }
.mat { max-height: 180rpx; margin-top: 8rpx; }
.mat-row { display: flex; align-items: center; gap: 10rpx; padding: 6rpx 0; }
.mat-code { width: 64rpx; font-weight: 700; font-size: 22rpx; }
.mat-name { flex: 1; color: $ink-2; font-size: 22rpx; }
.mat-cnt { font-weight: 700; font-size: 22rpx; }
.saves { display: flex; gap: 12rpx; margin-top: 10rpx; }
.btn { flex: 1; text-align: center; border-radius: 999rpx; padding: 16rpx 0; font-weight: 700; font-size: 26rpx; }
.btn text { font-size: 26rpx; font-weight: 700; }
.btn.primary { background: $brand; }
.btn.primary text { color: #fff; }
.btn.ghost { background: $brand-lighter; }
.btn.ghost text { color: $brand-dark; }
.draft-name {
  display: block; margin-top: 8rpx; color: $ink-3; font-size: 20rpx;
  overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
}
</style>
