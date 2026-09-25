import { cleanup, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  convertFileSrc: (path: string) => path
}))

import { PixelArtImage } from "@/components/PixelArtImage"
import { clearPixelArtDetectionCache } from "@/hooks/usePixelArtDetection"

beforeEach(() => {
  cleanup()
  clearPixelArtDetectionCache()
  invoke.mockReset()
})

describe("PixelArtImage", () => {
  it("수동 픽셀 보존 모드는 자동 감지 없이 픽셀 클래스를 적용한다", () => {
    invoke.mockClear()
    render(
      <PixelArtImage
        filePath="/sprite.png"
        scalingMode="pixelated"
        autoDetectPixelArt
        src="/sprite.png"
        alt="sprite"
      />
    )
    expect(screen.getByAltText("sprite").className).toContain("image-rendering-pixelated")
    expect(invoke).not.toHaveBeenCalled()
  })

  it("자동 모드는 분석 전까지 부드러운 표시를 사용하고, 감지 후 픽셀 표시로 전환한다", async () => {
    let resolveDetection!: (value: {
      classification: "pixel_art"
      confidence: number
      pixel_scale: number
      method: "hybrid"
    }) => void
    invoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveDetection = resolve
        })
    )

    render(
      <PixelArtImage
        filePath="/sprite.png"
        scalingMode="auto"
        autoDetectPixelArt
        src="/sprite.png"
        alt="sprite"
      />
    )
    const image = screen.getByAltText("sprite")
    expect(image.className).toContain("image-rendering-smooth")
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("detect_pixel_art", { filePath: "/sprite.png" })
    )

    resolveDetection({
      classification: "pixel_art",
      confidence: 0.9,
      pixel_scale: 4,
      method: "hybrid"
    })
    await waitFor(() => expect(image.className).toContain("image-rendering-pixelated"))
  })
})
