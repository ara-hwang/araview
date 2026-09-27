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

  it("축소 배율에서는 설정과 감지와 무관하게 항상 부드럽게 표시한다", () => {
    // nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다.
    expect(resolveImageRenderingMode("pixelated", false, null, 0.5)).toBe("smooth")
    expect(resolveImageRenderingMode("pixelated", false, null, 0.99)).toBe("smooth")
    expect(resolveImageRenderingMode("auto", true, pixelArt, 0.25)).toBe("smooth")
  })

  it("확대 배율에서는 기존 판정을 유지한다", () => {
    expect(resolveImageRenderingMode("pixelated", false, null, 1)).toBe("pixelated")
    expect(resolveImageRenderingMode("pixelated", false, null, 3.5)).toBe("pixelated")
    expect(resolveImageRenderingMode("auto", true, pixelArt, 2)).toBe("pixelated")
  })

  it("배율을 알 수 없으면 기존 판정을 유지한다", () => {
    expect(resolveImageRenderingMode("pixelated", false, null, null)).toBe("pixelated")
    expect(resolveImageRenderingMode("auto", true, pixelArt, null)).toBe("pixelated")
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
