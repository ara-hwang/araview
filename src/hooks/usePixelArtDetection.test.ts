import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock("@tauri-apps/api/core", () => ({ invoke }))

import { clearPixelArtDetectionCache, usePixelArtDetection } from "@/hooks/usePixelArtDetection"

const pixelArt = {
  classification: "pixel_art" as const,
  confidence: 0.9,
  pixel_scale: 4,
  method: "hybrid" as const
}

beforeEach(() => {
  cleanup()
  clearPixelArtDetectionCache()
  invoke.mockReset()
})

describe("usePixelArtDetection", () => {
  it("같은 경로의 동시 요청을 IPC 한 번으로 합친다", async () => {
    let resolveDetection!: (value: typeof pixelArt) => void
    invoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveDetection = resolve
        })
    )

    const first = renderHook(() => usePixelArtDetection("/sprite.png", 100, true))
    const second = renderHook(() => usePixelArtDetection("/sprite.png", 100, true))

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    resolveDetection(pixelArt)
    await waitFor(() => {
      expect(first.result.current?.classification).toBe("pixel_art")
      expect(second.result.current?.classification).toBe("pixel_art")
    })
  })

  it("이미지 경로가 바뀌는 첫 렌더에서 이전 판정을 재사용하지 않는다", async () => {
    const resolvers: Array<(value: typeof pixelArt) => void> = []
    invoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvers.push(resolve)
        })
    )
    const view = renderHook(({ path }) => usePixelArtDetection(path, 100, true), {
      initialProps: { path: "/first.png" }
    })
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    view.rerender({ path: "/second.png" })
    expect(view.result.current).toBeNull()
    resolvers[0]?.(pixelArt)
    await waitFor(() => expect(resolvers).toHaveLength(2))
    resolvers[1]?.(pixelArt)
    await waitFor(() => expect(view.result.current).toBeNull())
    view.unmount()
  })

  it("분석 디코드는 동시에 두 개로 제한하고 화면에서 벗어난 큐를 취소한다", async () => {
    const resolvers: Array<(value: typeof pixelArt) => void> = []
    invoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvers.push(resolve)
        })
    )

    const first = renderHook(() => usePixelArtDetection("/first.png", 1, true))
    const second = renderHook(() => usePixelArtDetection("/second.png", 2, true))
    const third = renderHook(() => usePixelArtDetection("/third.png", 3, true))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))

    third.unmount()
    resolvers.forEach((resolve) => resolve(pixelArt))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
    first.unmount()
    second.unmount()
  })

  it("일시적인 분석 실패는 캐시하지 않아 다시 시도한다", async () => {
    invoke.mockRejectedValueOnce(new Error("decode failed"))
    const first = renderHook(() => usePixelArtDetection("/photo.jpg", 200, true))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    first.unmount()

    invoke.mockResolvedValueOnce({
      classification: "continuous",
      confidence: 0.8,
      pixel_scale: null,
      method: "hybrid"
    })
    const second = renderHook(() => usePixelArtDetection("/photo.jpg", 200, true))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
    second.unmount()
  })
})
