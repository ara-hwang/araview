export type StripWindowInput = {
  /** 스크롤 오프셋 (가로 도크는 scrollLeft, 세로 도크는 scrollTop) */
  scrollOffset: number
  /** 보이는 영역 크기 (가로 도크는 clientWidth, 세로 도크는 clientHeight) */
  viewportSize: number
  itemCount: number
  itemSize: number
  gap: number
  padding: number
  /** 화면 밖 여유분으로 더 그릴 항목 수 */
  overscan?: number
}

export type StripWindow = {
  startIndex: number
  /** 배타적 끝 인덱스 */
  endIndex: number
  /** 스크롤 영역 전체 크기 */
  totalSize: number
}

/** 항목 시작 오프셋. 스트립 가상화와 스크롤 정렬에서 같은 식을 쓴다. */
export function stripOffsetForIndex(
  index: number,
  itemSize: number,
  gap: number,
  padding: number
): number {
  return padding + index * (itemSize + gap)
}

/**
 * 썸네일 스트립의 가시 창을 계산한다. 항목 크기가 고정이라
 * 스크롤 오프셋만으로 그릴 구간을 정할 수 있다.
 */
export function computeStripWindow({
  scrollOffset,
  viewportSize,
  itemCount,
  itemSize,
  gap,
  padding,
  overscan = 4
}: StripWindowInput): StripWindow {
  if (itemCount <= 0 || itemSize <= 0) {
    return { startIndex: 0, endIndex: 0, totalSize: 0 }
  }
  const stride = itemSize + gap
  const totalSize = padding * 2 + itemCount * itemSize + (itemCount - 1) * gap
  const visibleCount = Math.ceil((Math.max(0, viewportSize) + gap) / stride) + 1
  const first = Math.floor(Math.max(0, scrollOffset - padding) / stride)
  const startIndex = Math.max(0, Math.min(first - overscan, Math.max(0, itemCount - 1)))
  const endIndex = Math.min(itemCount, startIndex + visibleCount + overscan * 2)
  return { startIndex, endIndex, totalSize }
}

export type StripScrollTargetInput = {
  index: number
  scrollOffset: number
  viewportSize: number
  itemSize: number
  gap: number
  padding: number
}

/**
 * 현재 항목이 보이도록 맞출 스크롤 오프셋을 계산한다.
 * 이미 보이면 현재 값을 그대로 돌려준다.
 */
export function stripScrollToReveal({
  index,
  scrollOffset,
  viewportSize,
  itemSize,
  gap,
  padding
}: StripScrollTargetInput): number {
  if (index < 0 || viewportSize <= 0) return scrollOffset
  const itemStart = stripOffsetForIndex(index, itemSize, gap, padding)
  const itemEnd = itemStart + itemSize
  if (itemStart < scrollOffset) {
    return Math.max(0, itemStart - padding)
  }
  if (itemEnd > scrollOffset + viewportSize) {
    return Math.max(0, itemEnd - viewportSize + padding)
  }
  return scrollOffset
}
