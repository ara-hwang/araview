import { describe, it, expect } from "vitest"
import { getPositionBounds, clampPosition, isFullyContained } from "./zoomPanUtils"

describe("getPositionBounds", () => {
  it("returns 0 bounds when image fits inside container", () => {
    const result = getPositionBounds(800, 600, 400, 300, 1)
    expect(result).toEqual({ maxX: 0, maxY: 0 })
  })

  it("returns positive bounds when image is larger than container", () => {
    const result = getPositionBounds(800, 600, 1200, 900, 1)
    expect(result).toEqual({ maxX: 200, maxY: 150 })
  })

  it("accounts for zoom factor", () => {
    const result = getPositionBounds(800, 600, 400, 300, 3)
    expect(result).toEqual({ maxX: 200, maxY: 150 })
  })

  it("returns 0 for axis where image fits even with zoom", () => {
    const result = getPositionBounds(1000, 600, 400, 300, 2)
    expect(result).toEqual({ maxX: 0, maxY: 0 })
  })

  it("handles exact size match", () => {
    const result = getPositionBounds(800, 600, 800, 600, 1)
    expect(result).toEqual({ maxX: 0, maxY: 0 })
  })

  it("handles zoom making exact-fit image overflow", () => {
    const result = getPositionBounds(800, 600, 800, 600, 2)
    expect(result).toEqual({ maxX: 400, maxY: 300 })
  })
})

describe("clampPosition", () => {
  it("returns 0,0 when maxX and maxY are 0", () => {
    expect(clampPosition(100, 200, 0, 0)).toEqual({ x: 0, y: 0 })
  })

  it("clamps positive values to max bounds", () => {
    expect(clampPosition(300, 200, 100, 50)).toEqual({ x: 100, y: 50 })
  })

  it("clamps negative values to negative max bounds", () => {
    expect(clampPosition(-300, -200, 100, 50)).toEqual({ x: -100, y: -50 })
  })

  it("passes through values within bounds", () => {
    expect(clampPosition(50, 30, 100, 50)).toEqual({ x: 50, y: 30 })
  })

  it("passes through negative values within bounds", () => {
    expect(clampPosition(-50, -30, 100, 50)).toEqual({ x: -50, y: -30 })
  })

  it("handles mixed clamping per axis", () => {
    expect(clampPosition(200, 10, 100, 50)).toEqual({ x: 100, y: 10 })
  })
})

describe("isFullyContained", () => {
  it("returns true when image fits inside container", () => {
    expect(isFullyContained(800, 600, 400, 300, 1)).toBe(true)
  })

  it("returns true when image exactly matches container", () => {
    expect(isFullyContained(800, 600, 800, 600, 1)).toBe(true)
  })

  it("returns false when image is wider than container", () => {
    expect(isFullyContained(800, 600, 1200, 300, 1)).toBe(false)
  })

  it("returns false when image is taller than container", () => {
    expect(isFullyContained(800, 600, 400, 900, 1)).toBe(false)
  })

  it("returns false when zoom makes image overflow", () => {
    expect(isFullyContained(800, 600, 400, 300, 3)).toBe(false)
  })

  it("returns true when zoom keeps image within container", () => {
    expect(isFullyContained(800, 600, 200, 150, 2)).toBe(true)
  })
})
