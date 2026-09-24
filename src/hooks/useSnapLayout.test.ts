import { act, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useSnapLayout } from "./useSnapLayout"

// tauri-plugin-snap-layout: 주입 스크립트가 정의하는 전역 훅을 모킹한다.
const mockGlobalAttach = vi.fn()
const mockGlobalDetach = vi.fn()

// @tauri-apps/api/event의 listen을 모킹한다.
type Listener = (event: { payload?: unknown }) => void
const listeners = new Map<string, Listener>()
const mockListen = vi.fn(
  (event: string, handler: Listener) =>
    new Promise<() => void>((resolve) => {
      listeners.set(event, handler)
      resolve(() => listeners.delete(event))
    })
)

vi.mock("tauri-plugin-snap-layout", () => ({
  attach: (...args: unknown[]) => mockGlobalAttach(...args),
  detach: () => mockGlobalDetach()
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: [string, Listener]) => mockListen(...args)
}))

const flushAsync = async () => {
  await Promise.resolve()
  await Promise.resolve()
}

// window에서 임의 전역을 설정/해제하기 위한 헬퍼 (Window 타입에 없는 키 접근)
const setWindowGlobal = (key: string, value: unknown) => {
  ;(window as unknown as Record<string, unknown>)[key] = value
}
const deleteWindowGlobal = (key: string) => {
  delete (window as unknown as Record<string, unknown>)[key]
}

describe("useSnapLayout", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listeners.clear()
    mockGlobalDetach.mockResolvedValue(undefined)
    setWindowGlobal("__SNAP_LAYOUT_ATTACH__", mockGlobalAttach)
  })

  afterEach(() => {
    deleteWindowGlobal("__SNAP_LAYOUT_ATTACH__")
  })

  it("활성화되면 전역 attach로 버튼 id를 등록한다", async () => {
    const { result } = renderHook(() => useSnapLayout(true))
    await flushAsync()

    expect(mockGlobalAttach).toHaveBeenCalledWith("caption-maximize")
    expect(result.current.snapHover).toBe(false)
  })

  it("mouseenter 이벤트에서 snapHover가 true가 된다", async () => {
    const { result } = renderHook(() => useSnapLayout(true))
    await flushAsync()

    act(() => {
      listeners.get("tauri-snap://snap/mouseenter")?.({})
    })
    await flushAsync()
    expect(result.current.snapHover).toBe(true)

    act(() => {
      listeners.get("tauri-snap://snap/mouseleave")?.({})
    })
    await flushAsync()
    expect(result.current.snapHover).toBe(false)
  })

  it("비활성화되면 detach로 네이티브 오버레이를 떼어내고 hover를 끈다", async () => {
    const { result, rerender } = renderHook(({ enabled }) => useSnapLayout(enabled), {
      initialProps: { enabled: true }
    })
    await flushAsync()

    act(() => {
      listeners.get("tauri-snap://snap/mouseenter")?.({})
    })
    await flushAsync()
    expect(result.current.snapHover).toBe(true)

    rerender({ enabled: false })
    await flushAsync()
    expect(mockGlobalDetach).toHaveBeenCalled()
    expect(result.current.snapHover).toBe(false)
  })

  it("언마운트 시 detach를 호출한다", async () => {
    const { unmount } = renderHook(() => useSnapLayout(true))
    await flushAsync()
    expect(mockGlobalDetach).not.toHaveBeenCalled()

    unmount()
    await flushAsync()
    expect(mockGlobalDetach).toHaveBeenCalled()
  })

  it("전역 attach가 아직 정의되지 않으면 재시도 후 등록한다", async () => {
    deleteWindowGlobal("__SNAP_LAYOUT_ATTACH__")
    vi.useFakeTimers()

    try {
      renderHook(() => useSnapLayout(true))
      // 50ms 간격 재시도: 정의되기 전까지는 호출 없음
      vi.advanceTimersByTime(120)
      expect(mockGlobalAttach).not.toHaveBeenCalled()

      setWindowGlobal("__SNAP_LAYOUT_ATTACH__", mockGlobalAttach)
      vi.advanceTimersByTime(100)
      expect(mockGlobalAttach).toHaveBeenCalledWith("caption-maximize")
    } finally {
      vi.useRealTimers()
    }
  })

  it("비활성 상태로 시작하면 attach를 호출하지 않는다", async () => {
    renderHook(() => useSnapLayout(false))
    await flushAsync()
    expect(mockGlobalAttach).not.toHaveBeenCalled()
    expect(mockListen).not.toHaveBeenCalled()
  })
})
