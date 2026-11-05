/** 画布比工作区小就用边距居中；比工作区大就滚动到正中。 */
export interface BoardFrame {
  offsetX: number
  offsetY: number
  scrollLeft: number
  scrollTop: number
}

export function boardViewport(input: {
  canvas: number
  viewWidth: number
  viewHeight: number
}): BoardFrame {
  const canvas = Math.max(0, input.canvas)
  const viewWidth = Math.max(0, input.viewWidth)
  const viewHeight = Math.max(0, input.viewHeight)
  return {
    offsetX: gap(viewWidth, canvas),
    offsetY: gap(viewHeight, canvas),
    scrollLeft: gap(canvas, viewWidth),
    scrollTop: gap(canvas, viewHeight),
  }
}

function gap(outer: number, inner: number): number {
  return Math.max(0, Math.round((outer - inner) / 2))
}
