import { describe, it, expect } from "vitest"

import {
  getPositionBounds,
  clampPosition,
  isFullyContained,
  getFitZoomFromSizes,
  getMinZoom,
  getOrientedImageSize,
  getZoomForFitMode
} from "./zoomPanUtils"

describe("getOrientedImageSize", () => {
  it("keeps size for 0/180 rotation", () => {
    expect(getOrientedImageSize(400, 300, 0)).toEqual({
      width: 400,
      height: 300
    })
    expect(getOrientedImageSize(400, 300, 180)).toEqual({
      width: 400,
      height: 300
    })
  })

  it("swaps size for 90/270 rotation", () => {
    expect(getOrientedImageSize(400, 300, 90)).toEqual({
      width: 300,
      height: 400
    })
    expect(getOrientedImageSize(400, 300, 270)).toEqual({
      width: 300,
      height: 400
    })
  })
})

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

describe("getFitZoomFromSizes", () => {
  it("returns 1 when sizes are not ready", () => {
    expect(getFitZoomFromSizes(0, 600, 400, 300)).toBe(1)
    expect(getFitZoomFromSizes(800, 600, 0, 300)).toBe(1)
  })

  it("fits a large image to the container", () => {
    expect(getFitZoomFromSizes(800, 600, 1600, 1200)).toBe(0.5)
  })

  it("is greater than 1 when the image is smaller than the container", () => {
    expect(getFitZoomFromSizes(800, 600, 400, 300)).toBe(2)
  })
})

describe("getMinZoom", () => {
  it("does not force small images above 100%", () => {
    expect(getMinZoom(800, 600, 400, 300)).toBe(1)
  })

  it("allows large images to shrink to fit", () => {
    expect(getMinZoom(800, 600, 1600, 1200)).toBe(0.5)
  })
})

describe("getZoomForFitMode", () => {
  it("width는 컨테이너 너비에 맞춘다", () => {
    expect(getZoomForFitMode("width", 1000, 700, 2000, 1000)).toBe(0.5)
  })

  it("height는 컨테이너 높이에 맞춘다", () => {
    expect(getZoomForFitMode("height", 1000, 700, 2000, 1400)).toBe(0.5)
  })

  it("screen은 양쪽을 containment한다", () => {
    expect(getZoomForFitMode("screen", 1000, 700, 2000, 1000)).toBe(0.5)
  })

  it("width/height/screen은 작은 이미지도 확대한다", () => {
    expect(getZoomForFitMode("width", 1000, 700, 100, 100)).toBe(10)
    expect(getZoomForFitMode("height", 1000, 700, 100, 100)).toBe(7)
    expect(getZoomForFitMode("screen", 1000, 700, 100, 100)).toBe(7)
  })

  it("auto는 작은 이미지를 100%로 둔다", () => {
    expect(getZoomForFitMode("auto", 1000, 700, 100, 100)).toBe(1)
  })

  it("auto는 큰 이미지를 화면에 맞춘다", () => {
    expect(getZoomForFitMode("auto", 1000, 700, 2000, 1000)).toBe(0.5)
  })

  it("크기를 모르면 1을 반환한다", () => {
    expect(getZoomForFitMode("width", 0, 700, 2000, 1000)).toBe(1)
    expect(getZoomForFitMode("screen", 1000, 700, 0, 1000)).toBe(1)
  })
})
