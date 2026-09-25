import { describe, expect, it } from "vitest"

import { getPixelArtDetectionPath, resolveImageRenderingMode } from "@/utils/imageRendering"

const pixelArt = {
  classification: "pixel_art" as const,
  confidence: 0.9,
  pixel_scale: 4,
  method: "hybrid" as const
}

const uncertain = {
  classification: "uncertain" as const,
  confidence: 0.2,
  pixel_scale: null,
  method: "unsupported" as const
}

describe("resolveImageRenderingMode", () => {
  it("수동 설정은 자동 감지보다 우선한다", () => {
    expect(resolveImageRenderingMode("pixelated", false, null)).toBe("pixelated")
    expect(resolveImageRenderingMode("smooth", true, pixelArt)).toBe("smooth")
  })

  it("자동 모드는 확신 있는 픽셀 아트만 픽셀 보존으로 표시한다", () => {
    expect(resolveImageRenderingMode("auto", true, pixelArt)).toBe("pixelated")
    expect(resolveImageRenderingMode("auto", true, uncertain)).toBe("smooth")
    expect(resolveImageRenderingMode("auto", false, pixelArt)).toBe("smooth")
    expect(resolveImageRenderingMode("auto", true, { ...pixelArt, confidence: 0.5 })).toBe("smooth")
  })

  it("변환 또는 축소 sidecar가 있으면 표시 바이트를 분석한다", () => {
    const base = {
      file_path: "cache/converted.jpg",
      source_path: "source.heic",
      file_name: "source.heic",
      file_size: 10,
      mime_type: "image/heic",
      width: 10,
      height: 10
    }
    expect(getPixelArtDetectionPath(base)).toBe("cache/converted.jpg")
    expect(
      getPixelArtDetectionPath({ ...base, file_path: "source.heic", mime_type: "image/png" })
    ).toBe("source.heic")
  })
})
