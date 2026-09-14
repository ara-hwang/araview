import type { ImageHistogram } from "@/types"

/** DPI 표시문. 한 축만 있으면 단일 값, 둘 다 있으면 "300 × 72 DPI". */
export function formatDpi(
  dpiX: number | null | undefined,
  dpiY: number | null | undefined
): string | null {
  const valid = (n: unknown): n is number =>
    typeof n === "number" && Number.isFinite(n) && n > 0
  const x = valid(dpiX) ? dpiX : null
  const y = valid(dpiY) ? dpiY : null
  if (x === null && y === null) return null
  const one = (n: number) =>
    Number.isInteger(n) ? String(n) : String(Math.round(n * 10) / 10)
  if (x !== null && y !== null) {
    return x === y ? `${one(x)} DPI` : `${one(x)} × ${one(y)} DPI`
  }
  return `${one((x ?? y) as number)} DPI`
}

/** 유닉스 초 → 로케일 날짜+시각. 파기 불가면 null. */
export function formatUnixDateTime(
  unix: number | null | undefined,
  locale?: string
): string | null {
  if (typeof unix !== "number" || !Number.isFinite(unix) || unix < 0) {
    return null
  }
  const date = new Date(unix * 1000)
  if (Number.isNaN(date.getTime())) return null
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(date)
}

/** 히스토그램 전체 채널의 최대 빈도 (차트 정규화용, 최소 1). */
export function histogramMax(hist: ImageHistogram): number {
  let max = 1
  for (const channel of [hist.r, hist.g, hist.b]) {
    for (const count of channel) {
      if (count > max) max = count
    }
  }
  return max
}

/**
 * 단일 채널 빈도 → SVG area path. viewBox는 `0 0 width height` 기준이며
 * 막대 사이 틈 없이 계단형 영역으로 그린다.
 */
export function histogramChannelPath(
  bins: number[],
  max: number,
  width: number,
  height: number
): string {
  const n = bins.length
  if (n === 0 || width <= 0 || height <= 0) return ""
  const safeMax = max > 0 ? max : 1
  const step = width / n
  let d = `M0,${height}`
  for (let i = 0; i < n; i += 1) {
    const v = Math.min(bins[i] ?? 0, safeMax) / safeMax
    const y = height - v * height
    const x = i * step
    d += ` L${round2(x)},${round2(y)} L${round2(x + step)},${round2(y)}`
  }
  d += ` L${width},${height} Z`
  return d
}

function round2(n: number): number {
  return Math.round(n * 100) / 100
}
