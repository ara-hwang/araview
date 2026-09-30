import { describe, expect, it } from "vitest"

import { computeListWindow, scrollTopToReveal } from "@/utils/listWindow"

const base = { viewportHeight: 200, itemCount: 1000, rowHeight: 40, overscan: 3 }

describe("computeListWindow", () => {
  it("renders only the visible rows plus overscan at the top", () => {
    expect(computeListWindow({ ...base, scrollTop: 0 })).toEqual({
      start: 0,
      end: 8,
      totalHeight: 40000
    })
  })

  it("moves the window with scrollTop", () => {
    const w = computeListWindow({ ...base, scrollTop: 4000 })
    expect(w.start).toBe(97)
    expect(w.end).toBe(108)
  })

  it("clamps at the end of the list", () => {
    const w = computeListWindow({ ...base, scrollTop: 40000 })
    expect(w.end).toBe(1000)
    expect(w.start).toBeLessThan(1000)
  })

  it("handles empty lists", () => {
    expect(computeListWindow({ ...base, itemCount: 0, scrollTop: 0 })).toEqual({
      start: 0,
      end: 0,
      totalHeight: 0
    })
  })
})

describe("scrollTopToReveal", () => {
  it("keeps scrollTop when the row is visible", () => {
    expect(scrollTopToReveal(2, 0, 200, 40)).toBe(0)
  })
  it("scrolls up to a row above the viewport", () => {
    expect(scrollTopToReveal(1, 400, 200, 40)).toBe(40)
  })
  it("scrolls down to a row below the viewport", () => {
    expect(scrollTopToReveal(10, 0, 200, 40)).toBe(240)
  })
})
