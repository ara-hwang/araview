import { describe, expect, it } from "vitest"
import {
  computeGridLayout,
  computeGridWindow,
  gridOffsetForIndex,
  gridScrollTopToCenter,
  gridScrollTopToReveal
} from "./gridWindow"

const base = {
  cellHeight: 160,
  gap: 8,
  padding: 16
}

describe("computeGridLayout", () => {
  it("fills the viewport width with gap-aware columns", () => {
    const layout = computeGridLayout({
      viewportWidth: 1000,
      itemCount: 100,
      minCellWidth: 140,
      ...base
    })
    expect(layout.columns).toBe(6)
    expect(layout.cellWidth).toBeCloseTo(154.67, 1)
    expect(layout.rowStride).toBe(168)
    expect(layout.totalRows).toBe(17)
    expect(layout.totalHeight).toBe(32 + 17 * 160 + 16 * 8)
  })

  it("keeps at least one column in narrow viewports", () => {
    const layout = computeGridLayout({
      viewportWidth: 200,
      itemCount: 1,
      minCellWidth: 140,
      ...base
    })
    expect(layout.columns).toBe(1)
    expect(layout.cellWidth).toBe(168)
    expect(layout.totalHeight).toBe(192)
  })

  it("returns an empty layout without items", () => {
    const layout = computeGridLayout({
      viewportWidth: 1000,
      itemCount: 0,
      minCellWidth: 140,
      ...base
    })
    expect(layout.totalRows).toBe(0)
    expect(layout.totalHeight).toBe(0)
  })
})

describe("computeGridWindow", () => {
  const window = (scrollTop: number, viewportHeight = 500) =>
    computeGridWindow({
      scrollTop,
      viewportHeight,
      itemCount: 100,
      columns: 6,
      overscanRows: 1,
      ...base
    })

  it("starts at the first row with overscan", () => {
    expect(window(0)).toEqual({
      startRow: 0,
      endRow: 4,
      startIndex: 0,
      endIndex: 24,
      totalHeight: 2880
    })
  })

  it("follows the scroll position", () => {
    const result = window(1000)
    expect(result.startRow).toBe(4)
    expect(result.endRow).toBe(10)
    expect(result.startIndex).toBe(24)
    expect(result.endIndex).toBe(60)
  })

  it("clamps the window at the end", () => {
    const result = window(2800)
    expect(result.startRow).toBe(15)
    expect(result.endRow).toBe(17)
    expect(result.startIndex).toBe(90)
    expect(result.endIndex).toBe(100)
  })

  it("handles an empty list", () => {
    const result = computeGridWindow({
      scrollTop: 0,
      viewportHeight: 500,
      itemCount: 0,
      columns: 6,
      overscanRows: 1,
      ...base
    })
    expect(result).toEqual({
      startRow: 0,
      endRow: 0,
      startIndex: 0,
      endIndex: 0,
      totalHeight: 0
    })
  })
})

describe("gridOffsetForIndex", () => {
  it("maps index to row offset", () => {
    expect(gridOffsetForIndex(0, 6, 160, 8, 16)).toBe(16)
    expect(gridOffsetForIndex(5, 6, 160, 8, 16)).toBe(16)
    expect(gridOffsetForIndex(6, 6, 160, 8, 16)).toBe(184)
  })
})

describe("gridScrollTopToReveal", () => {
  const params = {
    scrollTop: 0,
    viewportHeight: 500,
    columns: 6,
    cellHeight: 160,
    gap: 8,
    padding: 16
  }

  it("keeps an already visible cell untouched", () => {
    expect(gridScrollTopToReveal({ ...params, index: 0 })).toBe(0)
  })

  it("scrolls down to a cell below the viewport", () => {
    expect(gridScrollTopToReveal({ ...params, index: 30 })).toBe(532)
  })

  it("scrolls up to a cell above the viewport", () => {
    expect(gridScrollTopToReveal({ ...params, index: 0, scrollTop: 600 })).toBe(
      0
    )
  })
})

describe("gridScrollTopToCenter", () => {
  it("centers the current cell and clamps at the top", () => {
    expect(
      gridScrollTopToCenter({
        index: 30,
        viewportHeight: 500,
        columns: 6,
        cellHeight: 160,
        gap: 8,
        padding: 16
      })
    ).toBe(686)
    expect(
      gridScrollTopToCenter({
        index: 0,
        viewportHeight: 500,
        columns: 6,
        cellHeight: 160,
        gap: 8,
        padding: 16
      })
    ).toBe(0)
  })
})
