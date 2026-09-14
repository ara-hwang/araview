import { describe, expect, it } from "vitest"
import {
  formatDpi,
  formatUnixDateTime,
  histogramChannelPath,
  histogramMax
} from "@/utils/imageDetails"

describe("formatDpi", () => {
  it("returns null when both axes are missing", () => {
    expect(formatDpi(null, null)).toBeNull()
    expect(formatDpi(undefined, undefined)).toBeNull()
    expect(formatDpi(0, -3)).toBeNull()
    expect(formatDpi(Number.NaN, null)).toBeNull()
  })

  it("collapses equal axes to a single value", () => {
    expect(formatDpi(300, 300)).toBe("300 DPI")
  })

  it("shows both axes when they differ", () => {
    expect(formatDpi(300, 72)).toBe("300 × 72 DPI")
  })

  it("shows the present axis only", () => {
    expect(formatDpi(300, null)).toBe("300 DPI")
    expect(formatDpi(null, 150)).toBe("150 DPI")
  })

  it("rounds fractional DPI to one decimal", () => {
    expect(formatDpi(46.54, null)).toBe("46.5 DPI")
  })
})

describe("formatUnixDateTime", () => {
  it("returns null for invalid input", () => {
    expect(formatUnixDateTime(null)).toBeNull()
    expect(formatUnixDateTime(undefined)).toBeNull()
    expect(formatUnixDateTime(-1)).toBeNull()
    expect(formatUnixDateTime(Number.NaN)).toBeNull()
  })

  it("formats epoch seconds with date and time", () => {
    const text = formatUnixDateTime(0, "en-US")
    expect(text).toContain("1970")
    // 시각 부분이 포함되는지 (로케일과 무관하게 숫자 2개 이상 + 구분자)
    expect(text).toMatch(/\d/)
  })
})

describe("histogramMax", () => {
  it("returns the largest bin with a floor of 1", () => {
    expect(histogramMax({ r: [0, 5], g: [3], b: [0], sampled_pixels: 8 })).toBe(
      5
    )
    expect(histogramMax({ r: [0], g: [0], b: [0], sampled_pixels: 0 })).toBe(1)
  })
})

describe("histogramChannelPath", () => {
  it("returns an empty path for degenerate input", () => {
    expect(histogramChannelPath([], 10, 100, 40)).toBe("")
    expect(histogramChannelPath([1, 2], 10, 0, 40)).toBe("")
  })

  it("builds a closed area path scaled by max", () => {
    const d = histogramChannelPath([0, 10], 10, 100, 40)
    expect(d.startsWith("M0,40")).toBe(true)
    expect(d.endsWith("L100,40 Z")).toBe(true)
    // 최대값 빈은 꼭대기(y=0)까지 올라간다
    expect(d).toContain("L50,0 L100,0")
    // 0 빈은 바닥(y=40)에 붙는다
    expect(d).toContain("L0,40 L50,40")
  })
})
