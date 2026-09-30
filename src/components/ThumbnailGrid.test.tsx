import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@/hooks/useThumbnailSrcs", () => ({
  useThumbnailSrcs: (paths: string[]) => ({
    urls: new Map(paths.map((p) => [p, `thumb://${p}`])),
    failed: new Set<string>(),
    retry: vi.fn()
  })
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

import { ThumbnailGrid } from "@/components/ThumbnailGrid"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore, type ViewMode } from "@/store/settingsStore"

const IMAGES = ["/pics/1.jpg", "/pics/2.jpg", "/pics/3.jpg", "/pics/4.jpg", "/pics/5.jpg"]

function setup(index: number, viewMode: ViewMode, showCoverAlone = true) {
  useAppStore.setState({
    dirImages: { images: IMAGES, current_index: index, availability: [] },
    failedPaths: [],
    archivePath: null,
    comicInfo: null
  })
  useSettingsStore.setState({ viewMode, showCoverAlone, loopNavigation: false })
}

const baseProps = {
  archivePath: null,
  getOrLoadImage: async () => ({
    file_path: "/pics/1.jpg",
    source_path: "/pics/1.jpg",
    file_name: "1.jpg",
    file_size: 1,
    mime_type: "image/jpeg",
    width: 10,
    height: 10
  }),
  failedPaths: [],
  onNavigateToIndex: vi.fn(),
  onClose: vi.fn()
}

// jsdom은 레이아웃 크기가 0이라 가상화 창이 거의 그려지지 않는다. 열/행 수를
// 넉넉히 잡아 모든 셀이 렌더되게 한다.
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get: () => 600
  })
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 400
  })
})

afterAll(() => {
  Reflect.deleteProperty(HTMLElement.prototype, "clientWidth")
  Reflect.deleteProperty(HTMLElement.prototype, "clientHeight")
})

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
})

const currentLabels = (container: HTMLElement) =>
  [...container.querySelectorAll('[role="option"][aria-current="true"]')].map((el) =>
    el.getAttribute("aria-label")
  )

describe("ThumbnailGrid 현재 페이지 표시", () => {
  it("단일 보기에서는 현재 장 하나만 표시한다", () => {
    setup(2, "single")
    const { container } = render(
      <ThumbnailGrid {...baseProps} dirImages={useAppStore.getState().dirImages} />
    )

    expect(currentLabels(container)).toEqual(["viewer.nav.thumb 3 3.jpg"])
  })

  it("양쪽 보기에서는 화면에 떠 있는 쌍 두 개를 표시한다", () => {
    setup(1, "left-to-right", false)
    const { container } = render(
      <ThumbnailGrid {...baseProps} dirImages={useAppStore.getState().dirImages} />
    )

    expect(currentLabels(container)).toEqual([
      "viewer.nav.thumb 2 2.jpg",
      "viewer.nav.thumb 3 3.jpg"
    ])
  })

  it("표지 단독 화면에서는 표지만 표시한다", () => {
    setup(0, "left-to-right", true)
    const { container } = render(
      <ThumbnailGrid {...baseProps} dirImages={useAppStore.getState().dirImages} />
    )

    expect(currentLabels(container)).toEqual(["viewer.nav.thumb 1 1.jpg"])
  })

  it("마지막 홀수 장은 혼자 보므로 한 개만 표시한다", () => {
    setup(4, "left-to-right", false)
    const { container } = render(
      <ThumbnailGrid {...baseProps} dirImages={useAppStore.getState().dirImages} />
    )

    expect(currentLabels(container)).toEqual(["viewer.nav.thumb 5 5.jpg"])
  })

  it("현재 페이지는 테두리로, 키보드 선택은 링으로 따로 표시한다", () => {
    setup(1, "left-to-right", false)
    const { container } = render(
      <ThumbnailGrid {...baseProps} dirImages={useAppStore.getState().dirImages} />
    )

    const option = (label: string) =>
      [...container.querySelectorAll('[role="option"]')].find(
        (el) => el.getAttribute("aria-label") === label
      )
    // 쌍 두 장은 테두리로 강조된다.
    expect(option("viewer.nav.thumb 2 2.jpg")?.className).toContain("border-foreground")
    expect(option("viewer.nav.thumb 3 3.jpg")?.className).toContain("border-foreground")
    // 쌍의 기준 장은 처음에 선택 위치라 링도 함께 붙는다.
    expect(option("viewer.nav.thumb 2 2.jpg")?.className).toContain("ring-2")
    expect(option("viewer.nav.thumb 3 3.jpg")?.className).not.toContain("ring-2")

    // 선택은 열 단위로 한 칸씩 움직인다. 현재 화면 밖으로 두 번 옮겨 본다.
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "ArrowRight" })
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "ArrowRight" })
    expect(container.querySelector('[aria-selected="true"]')?.getAttribute("aria-label")).toBe(
      "viewer.nav.thumb 4 4.jpg"
    )
    // 선택은 이동해도 현재 페이지 표시는 그대로다.
    expect(currentLabels(container)).toEqual([
      "viewer.nav.thumb 2 2.jpg",
      "viewer.nav.thumb 3 3.jpg"
    ])
    expect(option("viewer.nav.thumb 4 4.jpg")?.className).toContain("ring-2")
    expect(option("viewer.nav.thumb 4 4.jpg")?.className).not.toContain("border-foreground")
  })
})
