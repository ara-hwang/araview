import { act, cleanup, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useDirectoryNavigation } from "@/hooks/useDirectoryNavigation"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

const IMAGES = ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg", "/pics/d.jpg"]

function imageInfoFor(path: string) {
  const file_name = path.split("/").pop() ?? path
  return {
    file_path: path,
    file_name,
    file_size: 10,
    mime_type: "image/jpeg",
    width: 800,
    height: 600
  }
}

function setup(index = 0, total: string[] = IMAGES) {
  closeImage()
  useAppStore.setState({
    containerSize: { width: 1000, height: 700 },
    dirImages: { images: total, current_index: index, availability: [] },
    archivePath: null
  })
  useSettingsStore.setState({ loopNavigation: false, viewMode: "single" })
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe("useDirectoryNavigation 로드-표시-이동 플로우", () => {
  it("다음 이동이 이미지를 로드해 표시하고 인덱스를 올린다", async () => {
    setup(0)
    const loadImage = vi.fn(async (path: string) => {
      const { setImageInfoAndResetView } = await import("@/store/appStore")
      setImageInfoAndResetView(imageInfoFor(path))
    })
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[1], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(1)
    expect(useAppStore.getState().imageInfo?.file_path).toBe(IMAGES[1])
  })

  it("비루프 경계에서는 이동하지 않고 로드도 호출하지 않는다", async () => {
    setup(3)
    const loadImage = vi.fn()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).not.toHaveBeenCalled()
    expect(useAppStore.getState().dirImages.current_index).toBe(3)
  })

  it("루프가 켜지면 마지막 다음이 처음으로 감긴다", async () => {
    setup(3)
    useSettingsStore.setState({ loopNavigation: true })
    const loadImage = vi.fn()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[0], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(0)
  })

  it("양면 모드는 2장씩 넘긴다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right" })
    const loadImage = vi.fn()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[2], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("인덱스 점프가 범위를 clamp한다", async () => {
    setup(0)
    const loadImage = vi.fn()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateToIndex(99)
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[3], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(3)
  })

  it("오프셋 점프가 루프 설정에 따라 wrap/clamp된다", async () => {
    setup(0, ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg"])
    const loadImage = vi.fn()
    const { result, rerender } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateByOffset(-1)
    })
    // 비루프면 0에 clamp
    expect(useAppStore.getState().dirImages.current_index).toBe(0)

    act(() => {
      useSettingsStore.setState({ loopNavigation: true })
    })
    rerender()
    await act(async () => {
      await result.current.navigateByOffset(-1)
    })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("아카이브 모드에서는 엔트리 로더를 쓰고 인덱스를 올린다", async () => {
    setup(0)
    useAppStore.setState({ archivePath: "/docs/m.cbz" })
    const loadImage = vi.fn()
    const loadArchive = vi.fn()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage, loadArchive))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadArchive).toHaveBeenCalledWith("/docs/m.cbz", IMAGES[1])
    expect(loadImage).not.toHaveBeenCalled()
    expect(useAppStore.getState().dirImages.current_index).toBe(1)
  })
})
