import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
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

/** 로드 완료 상태의 원본 폭과 렌더 폭을 흉내 낸다. 반환 함수로 되돌린다. */
function mockImageMetrics(naturalWidth: number, renderedWidth: number) {
  const proto = HTMLImageElement.prototype as unknown as Record<string, unknown>
  Object.defineProperty(proto, "complete", { configurable: true, get: () => true })
  Object.defineProperty(proto, "naturalWidth", { configurable: true, get: () => naturalWidth })
  const rectSpy = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: renderedWidth,
    height: renderedWidth,
    top: 0,
    left: 0,
    right: renderedWidth,
    bottom: renderedWidth,
    x: 0,
    y: 0,
    toJSON: () => ({})
  } as DOMRect)
  return () => {
    rectSpy.mockRestore()
    delete proto.complete
    delete proto.naturalWidth
  }
}

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
    // 16px 원본을 24px로 표시 = 1.5x 확대: 2x 미만이라 감지 결과가 필요하다.
    const restore = mockImageMetrics(16, 24)

    try {
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
      fireEvent.load(image)
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
    } finally {
      restore()
    }
  })

  it("자동 모드는 2x 이상 확대에서 분석 없이 바로 픽셀 보존을 적용한다", async () => {
    invoke.mockResolvedValue(null)
    // 16px 원본을 64px로 표시 = 4x 확대.
    const restore = mockImageMetrics(16, 64)

    try {
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
      fireEvent.load(image)
      expect(image.className).toContain("image-rendering-pixelated")
      await new Promise((resolve) => setTimeout(resolve, 20))
      expect(invoke).not.toHaveBeenCalledWith("detect_pixel_art", expect.anything())
    } finally {
      restore()
    }
  })

  it("자동 모드는 배율을 실측하기 전에는 판정을 요청하지 않는다", async () => {
    invoke.mockResolvedValue(null)
    render(
      <PixelArtImage
        filePath="/sprite.png"
        scalingMode="auto"
        autoDetectPixelArt
        src="/sprite.png"
        alt="sprite"
      />
    )
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(invoke).not.toHaveBeenCalled()
  })

  it("자동 모드라도 축소 표시면 원본을 다시 디코드하는 판정을 요청하지 않는다", async () => {
    invoke.mockResolvedValue(null)
    // 800px 원본을 200px로 표시 = 0.25x 축소: 판정 결과를 쓰지 않는다.
    const restore = mockImageMetrics(800, 200)
    try {
      render(
        <PixelArtImage
          filePath="/manga.jpg"
          scalingMode="auto"
          autoDetectPixelArt
          src="/manga.jpg"
          alt="manga"
        />
      )
      fireEvent.load(screen.getByAltText("manga"))
      await new Promise((resolve) => setTimeout(resolve, 20))
      expect(invoke).not.toHaveBeenCalled()
    } finally {
      restore()
    }
  })

  it("실측 배율이 축소면 픽셀 보존 모드도 부드럽게 표시하고, 확대면 복원한다", () => {
    const proto = HTMLImageElement.prototype as unknown as Record<string, unknown>
    Object.defineProperty(proto, "complete", { configurable: true, get: () => true })
    Object.defineProperty(proto, "naturalWidth", { configurable: true, get: () => 800 })
    const rectSpy = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 200,
      height: 300,
      top: 0,
      left: 0,
      right: 200,
      bottom: 300,
      x: 0,
      y: 0,
      toJSON: () => ({})
    } as DOMRect)

    try {
      render(
        <PixelArtImage
          filePath="/manga.jpg"
          scalingMode="pixelated"
          autoDetectPixelArt={false}
          src="/manga.jpg"
          alt="manga"
        />
      )
      const image = screen.getByAltText("manga")
      // 200px 표시 / 800px 원본 = 0.25x 축소: nearest 보간은 스크린톤을 깨뜨린다.
      fireEvent.load(image)
      expect(image.className).toContain("image-rendering-smooth")

      // 1600px 표시 = 2x 확대: 픽셀 보존 판정이 다시 적용된다.
      rectSpy.mockReturnValue({
        width: 1600,
        height: 2400,
        top: 0,
        left: 0,
        right: 1600,
        bottom: 2400,
        x: 0,
        y: 0,
        toJSON: () => ({})
      } as DOMRect)
      fireEvent(window, new Event("resize"))
      expect(image.className).toContain("image-rendering-pixelated")
    } finally {
      rectSpy.mockRestore()
      delete proto.complete
      delete proto.naturalWidth
    }
  })
})
