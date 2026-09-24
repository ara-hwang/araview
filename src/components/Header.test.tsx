import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const mockHandleOpenFile = vi.fn()
const mockGoHome = vi.fn()
const mockToggleExif = vi.fn()
const mockPaletteSetOpen = vi.fn()

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    isMaximized: async () => false,
    onResized: async () => () => {},
    close: vi.fn(),
    setAlwaysOnTop: async () => {}
  })
}))

vi.mock("@tauri-apps/api/event", () => ({
  listen: async () => () => {}
}))

vi.mock("tauri-plugin-snap-layout", () => ({
  detach: async () => {}
}))

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {}
    })
  }
}))

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() }
}))

vi.mock("@/components/AppTooltip", () => ({
  AppTooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>
}))

vi.mock("@/components/GifControls", () => ({
  GifControls: () => null
}))

vi.mock("@/hooks/useAlwaysOnTop", () => ({
  useAlwaysOnTop: () => ({ alwaysOnTop: false, toggle: vi.fn() })
}))

vi.mock("@/hooks/useCloseImage", () => ({
  useCloseImage: () => mockGoHome
}))

vi.mock("@/hooks/useCommandPalette", () => ({
  requestOpenSettings: vi.fn(),
  usePaletteStore: { getState: () => ({ setOpen: mockPaletteSetOpen }) }
}))

vi.mock("@/hooks/useExifLoader", () => ({
  useExifLoader: () => ({ toggleExifPanel: mockToggleExif })
}))

vi.mock("@/hooks/useImageLoader", () => ({
  useImageLoader: () => ({ handleOpenFile: mockHandleOpenFile })
}))

vi.mock("@/hooks/useSnapLayout", () => ({
  useSnapLayout: () => ({ snapHover: false })
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import Header from "@/components/Header"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

const demoImage = {
  file_path: "/pics/a.jpg",
  source_path: "/pics/a.jpg",
  file_name: "a.jpg",
  file_size: 123,
  mime_type: "image/jpeg",
  width: 2000,
  height: 1000
}

function buttonByName(pattern: RegExp) {
  return screen.getByRole("button", { name: pattern })
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
  closeImage()
  useAppStore.setState({ zoom: 1 })
  useSettingsStore.setState({ viewMode: "single", fitMode: "auto" })
})

describe("Header", () => {
  it("이미지가 없으면 이미지 의존 버튼이 비활성화된다", () => {
    render(<Header />)

    expect(buttonByName(/header\.home/).hasAttribute("disabled")).toBe(true)
    expect(buttonByName(/header\.zoomOut/).hasAttribute("disabled")).toBe(true)
    expect(buttonByName(/header\.zoomIn/).hasAttribute("disabled")).toBe(true)
    expect(buttonByName(/menu\.rotateCw/).hasAttribute("disabled")).toBe(true)
    expect(buttonByName(/menu\.flipH/).hasAttribute("disabled")).toBe(true)
    // 줌 표시 100%
    expect(screen.getByText("100%")).toBeTruthy()
  })

  it("이미지가 있으면 홈 버튼이 활성화되고 줌 퍼센트가 반영된다", () => {
    useAppStore.setState({ imageInfo: demoImage, zoom: 2 })

    render(<Header />)

    expect(buttonByName(/header\.home/).hasAttribute("disabled")).toBe(false)
    expect(screen.getByText("200%")).toBeTruthy()
  })

  it("열기 버튼 클릭이 파일 열기를 호출한다", () => {
    render(<Header />)

    fireEvent.click(buttonByName(/header\.open/))
    expect(mockHandleOpenFile).toHaveBeenCalledTimes(1)
  })

  it("홈 버튼 클릭이 홈 이동을 호출한다", () => {
    useAppStore.setState({ imageInfo: demoImage })
    render(<Header />)

    fireEvent.click(buttonByName(/header\.home/))
    expect(mockGoHome).toHaveBeenCalledTimes(1)
  })

  it("보기 모드 전환이 설정을 업데이트한다", async () => {
    render(<Header />)

    fireEvent.click(buttonByName(/settings\.view\.ltr/))
    await waitFor(() => {
      expect(useSettingsStore.getState().viewMode).toBe("left-to-right")
    })
  })

  it("회전 버튼 클릭이 rotation 상태를 바꾼다", () => {
    useAppStore.setState({ imageInfo: demoImage, rotation: 0 })
    render(<Header />)

    fireEvent.click(buttonByName(/menu\.rotateCw/))
    expect(useAppStore.getState().rotation).toBe(90)
  })

  it("캡션 버튼(최소화/최대화/닫기)이 렌더된다", () => {
    render(<Header />)

    expect(buttonByName(/header\.minimize/)).toBeTruthy()
    expect(buttonByName(/header\.maximize/)).toBeTruthy()
    expect(buttonByName(/header\.close/)).toBeTruthy()
  })
})
