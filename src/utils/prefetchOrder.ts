/** 이웃 프리패치 대상 인덱스를 다음 우선 순서로 반환한다. */
export function getPrefetchOrder(
  total: number,
  index: number,
  loopNavigation: boolean,
  prefetchDistance: number
): number[] {
  if (total <= 1 || prefetchDistance <= 0) return []
  if (index < 0 || index >= total) return []

  const normalize = (value: number) => ((value % total) + total) % total
  const order: number[] = []
  const seen = new Set<number>([index])

  for (let offset = 1; offset <= prefetchDistance; offset += 1) {
    // 다음 페이지를 먼저: 체감 지연은 정방향 탐색에서 가장 크다.
    const next = index + offset
    if (next < total) {
      if (!seen.has(next)) {
        seen.add(next)
        order.push(next)
      }
    } else if (loopNavigation && total > 1) {
      const wrapped = normalize(next)
      if (!seen.has(wrapped)) {
        seen.add(wrapped)
        order.push(wrapped)
      }
    }

    const prev = index - offset
    if (prev >= 0) {
      if (!seen.has(prev)) {
        seen.add(prev)
        order.push(prev)
      }
    } else if (loopNavigation && total > 1) {
      const wrapped = normalize(prev)
      if (!seen.has(wrapped)) {
        seen.add(wrapped)
        order.push(wrapped)
      }
    }
  }

  return order
}
