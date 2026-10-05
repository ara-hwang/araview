import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useGifPlayer } from "@/hooks/useGifPlayer"
import { useGifStore } from "@/store/gifStore"

const context = {
  imageSmoothingEnabled: true,
  clearRect: vi.fn(),
  drawImage: vi.fn()
}

const decoderTypes: string[] = []

class MockImageDecoder {
  tracks = {
    ready: Promise.resolve(),
    selectedTrack: { frameCount: 2, repetitionCount: 0 }
  }
  completed = Promise.resolve()

  constructor(init: { data: ArrayBuffer; type: string; preferAnimation: boolean }) {
    decoderTypes.push(init.type)
  }

  async decode() {
    return {
      image: {
        displayWidth: 16,
        displayHeight: 16,
        duration: 100,
        close: vi.fn()
      }
    }
  }

  close() {}
}

beforeEach(() => {
  cleanup()
  useGifStore.getState().reset()
  decoderTypes.length = 0
  context.imageSmoothingEnabled = true
  context.clearRect.mockClear()
  context.drawImage.mockClear()
  vi.stubGlobal("ImageDecoder", MockImageDecoder)
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      arrayBuffer: async () => new ArrayBuffer(8)
    })
  )
  vi.stubGlobal("matchMedia", vi.fn().mockReturnValue({ matches: false }))
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D
  )
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe("useGifPlayer", () => {
  it("픽셀 보존 모드에서 Canvas 스무딩을 끈다", async () => {
    const canvas = document.createElement("canvas")
    const canvasRef = { current: canvas }
    const view = renderHook(() => useGifPlayer(canvasRef, "asset://test.gif", true, false))

    await waitFor(() => expect(context.imageSmoothingEnabled).toBe(false))
    expect(context.drawImage).toHaveBeenCalled()
    expect(decoderTypes).toEqual(["image/gif"])
    view.unmount()
  })

  it("APNG는 image/apng 디코더로 프레임을 읽는다", async () => {
    const canvasRef = { current: document.createElement("canvas") }
    const view = renderHook(() =>
      useGifPlayer(canvasRef, "asset://test.apng", true, true, "image/apng")
    )

    await waitFor(() => expect(useGifStore.getState().active).toBe(true))
    expect(decoderTypes).toEqual(["image/apng"])
    expect(useGifStore.getState().frameCount).toBe(2)
    view.unmount()
  })
})
