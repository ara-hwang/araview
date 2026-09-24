import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const mockRetry = vi.fn()
const mockWheelNavigate = vi.fn()

vi.mock("@/hooks/useThumbnailSrcs", () => ({
  useThumbnailSrcs: (paths: string[]) => ({
    urls: new Map(paths.map((p) => [p, `thumb://${p}`])),
    failed: new Set<string>(),
    retry: mockRetry
  })
}))

vi.mock("@/hooks/useWheelNavigation", () => ({
  useWheelNavigation: () => mockWheelNavigate
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (p: string) => p,
  invoke: async () => null
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

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && (opts.index !== undefined || opts.name !== undefined)
        ? `${key} ${String(opts.index ?? "")} ${String(opts.name ?? "")}`
        : key
  })
}))

import { ImageNavBar } from "@/components/ImageNavBar"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

const IMAGES = ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg"]

function setup(dirImages?: { images: string[]; current_index: number; availability?: [] }) {
  closeImage()
  useAppStore.setState({
    dirImages: dirImages
      ? { availability: [], ...dirImages }
      : { images: IMAGES, current_index: 0, availability: [] },
    failedPaths: [],
    archivePath: null
  })
  useSettingsStore.setState({ loopNavigation: false })
}

const baseProps = {
  onNavigate: vi.fn(),
  onNavigateToIndex: vi.fn(),
  getOrLoadImage: async () => ({
    file_path: "/pics/a.jpg",
    source_path: "/pics/a.jpg",
    file_name: "a.jpg",
    file_size: 1,
    mime_type: "image/jpeg",
    width: 10,
    height: 10
  }),
  position: "bottom" as const,
  thumbSize: "s" as const,
  showName: false,
  showIndex: false
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe("ImageNavBar", () => {
  it("첫 장에서는 이전 버튼이 비활성화되고 다음은 이동을 호출한다", () => {
    setup()
    const onNavigate = vi.fn()
    render(<ImageNavBar {...baseProps} onNavigate={onNavigate} />)

    const prev = screen.getByRole("button", { name: "viewer.nav.prev" })
    const next = screen.getByRole("button", { name: "viewer.nav.next" })
    expect((prev as HTMLButtonElement).disabled).toBe(true)
    expect((next as HTMLButtonElement).disabled).toBe(false)

    fireEvent.click(next)
    expect(onNavigate).toHaveBeenCalledWith("next")
  })

  it("루프가 켜지면 첫 장에서도 이전 버튼이 활성화된다", () => {
    setup()
    useSettingsStore.setState({ loopNavigation: true })
    render(<ImageNavBar {...baseProps} />)

    expect(
      (screen.getByRole("button", { name: "viewer.nav.prev" }) as HTMLButtonElement).disabled
    ).toBe(false)
  })

  it("마지막 장에서는 다음 버튼이 비활성화된다", () => {
    setup({ images: IMAGES, current_index: 2 })
    render(<ImageNavBar {...baseProps} />)

    expect(
      (screen.getByRole("button", { name: "viewer.nav.next" }) as HTMLButtonElement).disabled
    ).toBe(true)
  })

  it("썸네일 클릭이 해당 인덱스 이동을 호출한다", () => {
    setup({ images: IMAGES, current_index: 0 })
    const onNavigateToIndex = vi.fn()
    render(<ImageNavBar {...baseProps} onNavigateToIndex={onNavigateToIndex} />)

    const thumb = screen.getByRole("button", { name: /viewer\.nav\.thumb 2 / })
    fireEvent.click(thumb)
    expect(onNavigateToIndex).toHaveBeenCalledWith(1)
  })

  it("그리드 토글과 도크 숨김이 호출된다", () => {
    setup()
    const onToggleGrid = vi.fn()
    const onToggleDock = vi.fn()
    render(<ImageNavBar {...baseProps} onToggleGrid={onToggleGrid} onToggleDock={onToggleDock} />)

    fireEvent.click(screen.getByRole("button", { name: "viewer.nav.grid" }))
    expect(onToggleGrid).toHaveBeenCalledWith()
    fireEvent.click(screen.getByRole("button", { name: "viewer.nav.dockHide" }))
    expect(onToggleDock).toHaveBeenCalledTimes(1)
  })

  it("이미지가 1장이면 스트립 썸네일을 렌더하지 않는다", () => {
    setup({ images: ["/pics/only.jpg"], current_index: 0 })
    render(<ImageNavBar {...baseProps} />)

    expect(screen.queryByRole("button", { name: /viewer\.nav\.thumb/ })).toBeNull()
  })

  it("hidden이면 컨테이너가 숨김 처리된다", () => {
    setup()
    const { container } = render(<ImageNavBar {...baseProps} hidden />)
    expect(container.firstChild).toHaveProperty("className")
    expect((container.firstChild as HTMLElement).className).toContain("hidden")
  })
})
