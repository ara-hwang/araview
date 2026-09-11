/** 고정 셀 크기 썸네일 그리드의 가상화 계산 (순수 함수). */

export type GridLayoutInput = {
  viewportWidth: number
  itemCount: number
  minCellWidth: number
  cellHeight: number
  gap: number
  padding: number
}

export type GridLayout = {
  columns: number
  cellWidth: number
  rowStride: number
  totalRows: number
  totalHeight: number
}

/** 뷰포트 폭에 맞는 열 수와 셀 폭, 전체 높이를 계산한다. */
export function computeGridLayout(input: GridLayoutInput): GridLayout {
  const { viewportWidth, itemCount, minCellWidth, cellHeight, gap, padding } =
    input
  const available = Math.max(0, viewportWidth - padding * 2)
  const columns = Math.max(
    1,
    Math.floor((available + gap) / (Math.max(1, minCellWidth) + gap))
  )
  const cellWidth = Math.max(0, (available - gap * (columns - 1)) / columns)
  const totalRows = Math.ceil(itemCount / columns)
  const totalHeight =
    totalRows > 0
      ? padding * 2 + totalRows * cellHeight + (totalRows - 1) * gap
      : 0
  return {
    columns,
    cellWidth,
    rowStride: cellHeight + gap,
    totalRows,
    totalHeight
  }
}

export type GridWindowInput = {
  scrollTop: number
  viewportHeight: number
  itemCount: number
  columns: number
  cellHeight: number
  gap: number
  padding: number
  overscanRows: number
}

export type GridWindow = {
  startRow: number
  endRow: number
  startIndex: number
  endIndex: number
  totalHeight: number
}

/** 보이는 행 범위(±overscan)만 잘라 렌더할 인덱스 창을 돌려준다. */
export function computeGridWindow(input: GridWindowInput): GridWindow {
  const columns = Math.max(1, Math.floor(input.columns))
  const totalRows = Math.ceil(input.itemCount / columns)
  const rowStride = input.cellHeight + input.gap
  const totalHeight =
    totalRows > 0
      ? input.padding * 2 +
        totalRows * input.cellHeight +
        (totalRows - 1) * input.gap
      : 0

  if (totalRows === 0 || rowStride <= 0) {
    return {
      startRow: 0,
      endRow: 0,
      startIndex: 0,
      endIndex: 0,
      totalHeight: 0
    }
  }

  const firstVisible = Math.floor(
    Math.max(0, input.scrollTop - input.padding) / rowStride
  )
  const lastVisible = Math.ceil(
    Math.max(0, input.scrollTop + input.viewportHeight - input.padding) /
      rowStride
  )
  const overscan = Math.max(0, Math.floor(input.overscanRows))
  const startRow = Math.min(totalRows, Math.max(0, firstVisible - overscan))
  const endRow = Math.min(totalRows, Math.max(startRow, lastVisible + overscan))
  const startIndex = Math.min(input.itemCount, startRow * columns)
  const endIndex = Math.min(input.itemCount, endRow * columns)
  return { startRow, endRow, startIndex, endIndex, totalHeight }
}

export function gridRowForIndex(index: number, columns: number): number {
  return Math.floor(index / Math.max(1, columns))
}

/** 셀의 상단 오프셋(px). */
export function gridOffsetForIndex(
  index: number,
  columns: number,
  cellHeight: number,
  gap: number,
  padding: number
): number {
  return padding + gridRowForIndex(index, columns) * (cellHeight + gap)
}

/** 선택 셀이 항상 보이도록 필요한 scrollTop을 반환한다. 이미 보이면 그대로. */
export function gridScrollTopToReveal(params: {
  index: number
  scrollTop: number
  viewportHeight: number
  columns: number
  cellHeight: number
  gap: number
  padding: number
}): number {
  const top = gridOffsetForIndex(
    params.index,
    params.columns,
    params.cellHeight,
    params.gap,
    params.padding
  )
  const bottom = top + params.cellHeight
  const viewBottom = params.scrollTop + params.viewportHeight
  if (top < params.scrollTop) {
    return Math.max(0, top - params.padding)
  }
  if (bottom > viewBottom) {
    return Math.max(0, bottom - params.viewportHeight + params.padding)
  }
  return params.scrollTop
}

/** 그리드를 열 때 선택 셀을 화면 중앙에 두는 scrollTop. */
export function gridScrollTopToCenter(params: {
  index: number
  viewportHeight: number
  columns: number
  cellHeight: number
  gap: number
  padding: number
}): number {
  const top = gridOffsetForIndex(
    params.index,
    params.columns,
    params.cellHeight,
    params.gap,
    params.padding
  )
  const centered = top - (params.viewportHeight - params.cellHeight) / 2
  return Math.max(0, centered)
}
