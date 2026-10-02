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

import { useZoomPan, useZoomPanSync } from "@/hooks/useZoomPan"
import { closeImage, setZoomToFit, useAppStore, zoomInBy } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

function openTestImage() {
  useAppStore.setState({
    imageInfo: {
      file_path: "/pics/a.jpg",
      source_path: "/pics/a.jpg",
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

describe("useZoomPanSync 리사이즈 핏 재적용", () => {
  it("핏 잠금 상태에서는 컨테이너가 바뀌면 기억된 맞춤 모드로 줌을 다시 맞춘다", () => {
    openTestImage()
    setZoomToFit("width")
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)

    renderHook(() => useZoomPanSync())

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

    renderHook(() => useZoomPanSync())

    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 } })
    })

    expect(useAppStore.getState().zoom).toBeCloseTo(manualZoom)
  })

  it("single 모드가 아니면 리사이즈해도 줌을 바꾸지 않는다", () => {
    openTestImage()
    setZoomToFit("width")
    useSettingsStore.setState({ viewMode: "webtoon" })

    renderHook(() => useZoomPanSync())

    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 } })
    })

    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })
})

describe("useZoomPan 입력 핸들러", () => {
  it("팬/줌 상태가 바뀌어도 다시 렌더되지 않고 핸들러도 그대로다", () => {
    openTestImage()
    let renders = 0
    const { result } = renderHook(() => {
      renders += 1
      return useZoomPan()
    })
    const first = result.current
    const rendersBefore = renders

    act(() => {
      useAppStore.setState({ position: { x: 40, y: -20 }, zoom: 2, isDragging: true })
    })
    act(() => {
      useAppStore.setState({ containerSize: { width: 500, height: 700 }, isDragging: false })
    })

    expect(renders).toBe(rendersBefore)
    expect(result.current.handleMouseDown).toBe(first.handleMouseDown)
    expect(result.current.handleMouseMove).toBe(first.handleMouseMove)
    expect(result.current.handleMouseUp).toBe(first.handleMouseUp)
  })
})

describe("useZoomPanSync 줌 변경 위치 보정", () => {
  it("줌이 바뀌면 위치를 같은 비율로 옮기고 경계 안으로 맞춘다", () => {
    openTestImage()
    // 2000x1000을 2배로 보면 4000x2000. 컨테이너 1000x700에서 x 경계는 ±1500이다.
    useAppStore.setState({ zoom: 1, position: { x: 100, y: 50 }, isFitLocked: false })
    renderHook(() => useZoomPanSync())

    act(() => {
      useAppStore.setState({ zoom: 2 })
    })

    expect(useAppStore.getState().position).toEqual({ x: 200, y: 100 })
  })

  it("위치가 이미 (0,0)이면 줌이 바뀌어도 위치 객체를 바꾸지 않는다", () => {
    openTestImage()
    useAppStore.setState({ isFitLocked: false })
    renderHook(() => useZoomPanSync())
    const before = useAppStore.getState().position

    act(() => {
      useAppStore.setState({ zoom: 2 })
    })

    expect(useAppStore.getState().position).toBe(before)
  })
})
