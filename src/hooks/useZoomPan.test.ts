import { act, cleanup, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => ({
      get: vi.fn(async () => null),
      set: vi.fn(async () => {}),
      save: vi.fn(async () => {})
    }))
  }
}))

import { useZoomPan } from "@/hooks/useZoomPan"
import { closeImage, setZoomToFit, useAppStore, zoomInBy } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

function openTestImage() {
  useAppStore.setState({
    imageInfo: {
      file_path: "/pics/a.jpg",
      file_name: "a.jpg",
      file_size: 123,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1000
    },
    containerSize: { width: 1000, height: 700 },
    imageSize: { width: 2000, height: 1000 },
    rotation: 0,
    zoom: 1,
    position: { x: 0, y: 0 },
    isDragging: false
  })
}

beforeEach(() => {
  closeImage()
  useSettingsStore.setState({ fitMode: "width", viewMode: "single" })
})

afterEach(() => {
  cleanup()
})

describe("useZoomPan 리사이즈 핏 재적용", () => {
  it("핏 잠금 상태에서는 컨테이너가 바뀌면 기억된 맞춤 모드로 줌을 다시 맞춘다", () => {
    openTestImage()
    setZoomToFit("width")
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)

    renderHook(() => useZoomPan())

    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 } })
    })

    const state = useAppStore.getState()
    expect(state.zoom).toBeCloseTo(0.25)
    expect(state.position).toEqual({ x: 0, y: 0 })
  })

  it("수동 줌으로 잠금이 풀리면 리사이즈해도 줌을 유지한다", () => {
    openTestImage()
    setZoomToFit("width")
    zoomInBy(2)
    const manualZoom = useAppStore.getState().zoom
    expect(useAppStore.getState().isFitLocked).toBe(false)

    renderHook(() => useZoomPan())

    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 } })
    })

    expect(useAppStore.getState().zoom).toBeCloseTo(manualZoom)
  })

  it("single 모드가 아니면 리사이즈해도 줌을 바꾸지 않는다", () => {
    openTestImage()
    setZoomToFit("width")
    useSettingsStore.setState({ viewMode: "webtoon" })

    renderHook(() => useZoomPan())

    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 } })
    })

    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })
})
