import { describe, expect, it } from "vitest"

import { getPrefetchOrder } from "./prefetchOrder"

describe("getPrefetchOrder", () => {
  it("returns next-first order", () => {
    expect(getPrefetchOrder(10, 5, false, 2)).toEqual([6, 4, 7, 3])
  })

  it("clamps at boundaries without loop", () => {
    expect(getPrefetchOrder(5, 0, false, 2)).toEqual([1, 2])
    expect(getPrefetchOrder(5, 4, false, 2)).toEqual([3, 2])
  })

  it("wraps with loop navigation", () => {
    expect(getPrefetchOrder(5, 0, true, 2)).toEqual([1, 4, 2, 3])
    expect(getPrefetchOrder(5, 4, true, 1)).toEqual([0, 3])
  })

  it("returns empty for single image or zero distance", () => {
    expect(getPrefetchOrder(1, 0, true, 3)).toEqual([])
    expect(getPrefetchOrder(10, 5, false, 0)).toEqual([])
  })

  it("never includes the current index", () => {
    expect(getPrefetchOrder(3, 1, true, 5)).not.toContain(1)
  })
})
