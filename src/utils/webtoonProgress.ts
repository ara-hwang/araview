export type WebtoonPageRect = {
  index: number
  top: number
  height: number
}

export type WebtoonScrollMetrics = {
  currentIndex: number
  percent: number
}

type CalculateWebtoonScrollMetricsInput = {
  pages: readonly WebtoonPageRect[]
  viewportTop: number
  viewportHeight: number
  scrollTop: number
  scrollHeight: number
  fallbackIndex: number
}

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max)

export function calculateWebtoonScrollMetrics({
  pages,
  viewportTop,
  viewportHeight,
  scrollTop,
  scrollHeight,
  fallbackIndex
}: CalculateWebtoonScrollMetricsInput): WebtoonScrollMetrics {
  const safeViewportHeight = Number.isFinite(viewportHeight) ? Math.max(0, viewportHeight) : 0
  const safeViewportTop = Number.isFinite(viewportTop) ? viewportTop : 0
  const validPages = pages.filter(
    (page) =>
      Number.isFinite(page.index) &&
      Number.isFinite(page.top) &&
      Number.isFinite(page.height) &&
      page.height >= 0
  )

  const safeScrollHeight = Number.isFinite(scrollHeight) ? Math.max(0, scrollHeight) : 0
  const maxScroll = Math.max(0, safeScrollHeight - safeViewportHeight)
  const safeScrollTop = Number.isFinite(scrollTop) ? clamp(scrollTop, 0, maxScroll) : 0
  const percent = maxScroll === 0 ? 100 : Math.round((safeScrollTop / maxScroll) * 100)

  if (
    validPages.length === 0 ||
    safeViewportHeight === 0 ||
    validPages.every((page) => page.height === 0)
  ) {
    return { currentIndex: fallbackIndex, percent }
  }

  const viewportBottom = safeViewportTop + safeViewportHeight
  const visiblePages = validPages.filter(
    (page) => page.top + page.height >= safeViewportTop && page.top <= viewportBottom
  )
  const candidates = visiblePages.length > 0 ? visiblePages : validPages
  const viewportCenter = safeViewportTop + safeViewportHeight / 2

  let currentIndex = candidates[0].index
  let closestDistance = Number.POSITIVE_INFINITY
  for (const page of candidates) {
    const pageCenter = page.top + page.height / 2
    const distance = Math.abs(pageCenter - viewportCenter)
    if (distance < closestDistance) {
      currentIndex = page.index
      closestDistance = distance
    }
  }

  return { currentIndex, percent }
}
