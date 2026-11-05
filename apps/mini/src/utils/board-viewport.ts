/** 默认缩放：画布宽度等于工作区宽度。不居中，也不留左右空档。 */
export function fitCellSize(side: number, availWidth: number, bufferCap = 2048): number {
  const cells = Math.max(1, Math.floor(side))
  const width = Math.max(1, Math.floor(availWidth))
  const maxCell = Math.max(1, Math.floor(bufferCap / cells))
  return Math.min(maxCell, width / cells)
}

/** 画布 CSS 边长。按格子反算时用四舍五入，避免二进制小数少 1 像素。 */
export function boardCssSize(side: number, cell: number): number {
  const cells = Math.max(1, Math.floor(side))
  return Math.max(1, Math.round(cells * cell))
}
