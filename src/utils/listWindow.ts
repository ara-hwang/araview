/** 고정 행 높이 목록의 가상화 계산 (순수 함수). */

export type ListWindowInput = {
  scrollTop: number
  viewportHeight: number
  itemCount: number
  rowHeight: number
  /** 보이는 범위 위아래로 추가로 그릴 행 수 */
  overscan: number
}

export type ListWindow = {
  /** 그릴 첫 행 (포함) */
  start: number
  /** 그릴 마지막 행 (미포함) */
  end: number
  totalHeight: number
}

export function computeListWindow(input: ListWindowInput): ListWindow {
  const { scrollTop, viewportHeight, itemCount, rowHeight, overscan } = input
  const totalHeight = itemCount * rowHeight
  if (itemCount === 0 || rowHeight <= 0) return { start: 0, end: 0, totalHeight }

  const first = Math.floor(Math.max(0, scrollTop) / rowHeight)
  const last = Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewportHeight)) / rowHeight)
  return {
    start: Math.min(itemCount - 1, Math.max(0, first - overscan)),
    end: Math.min(itemCount, Math.max(first + 1, last) + overscan),
    totalHeight
  }
}

/** `index` 행이 보이도록 필요한 scrollTop. 이미 보이면 현재 값을 그대로 돌려준다. */
export function scrollTopToReveal(
  index: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number
): number {
  const top = index * rowHeight
  const bottom = top + rowHeight
  if (top < scrollTop) return top
  if (bottom > scrollTop + viewportHeight) return bottom - viewportHeight
  return scrollTop
}
