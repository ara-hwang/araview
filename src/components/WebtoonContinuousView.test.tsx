import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => path,
  invoke: async () => ({
    classification: "continuous",
    confidence: 0,
    pixel_scale: null,
    method: "unsupported"
  })
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string | number>) => {
      if (key === "viewer.webtoon.position") {
        return `${values?.current} / ${values?.total} · ${values?.percent}%`
      }
      if (key === "viewer.webtoon.progressLabel") {
        return `${values?.current} of ${values?.total}, ${values?.percent} percent`
      }
      return key
    }
  })
}))

import { WebtoonContinuousView } from "@/components/WebtoonContinuousView"
import type { ImageInfo } from "@/types"

const getOrLoadImage = async (path: string): Promise<ImageInfo> => ({
  file_path: path,
  source_path: path,
  file_name: path.split("/").pop() ?? path,
  file_size: 100,
  mime_type: "image/jpeg",
  width: 800,
  height: 1200
})

beforeEach(() => {
  cleanup()
  vi.stubGlobal("IntersectionObserver", undefined)
  vi.stubGlobal("ResizeObserver", undefined)
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0)
    return 1
  })
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn()
  })
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get: () => 2000
  })
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe("WebtoonContinuousView", () => {
  it("이미지 간격, 페이지 경계, Fit width, 진행 표시, 썸네인 점프를 반영한다", async () => {
    const onOpenThumbnailGrid = vi.fn()
    const { container } = render(
      <WebtoonContinuousView
        images={["/comic/page-1.jpg", "/comic/page-2.jpg"]}
        currentIndex={0}
        getOrLoadImage={getOrLoadImage}
        onCenterChange={vi.fn()}
        scrollTarget={null}
        imageGap={24}
        showPageBoundaries
        fitWidth
        showProgress
        thumbnailJump
        onOpenThumbnailGrid={onOpenThumbnailGrid}
      />
    )

    const firstImage = await screen.findByAltText("page-1.jpg")
    expect(firstImage.className).toContain("w-full")
    expect(
      container.querySelector('[data-webtoon-index="1"]')?.classList.contains("border-t")
    ).toBe(true)
    expect(
      container.querySelector('[style*="--webtoon-image-gap"]')?.getAttribute("style")
    ).toContain("--webtoon-image-gap: 24px")
    expect(screen.getByTestId("webtoon-progress").textContent).toContain("1 / 2 · 0%")
    expect(screen.getByRole("progressbar").getAttribute("aria-label")).toBe("1 of 2, 0 percent")

    fireEvent.click(screen.getByRole("button", { name: "viewer.webtoon.openThumbnails" }))
    expect(onOpenThumbnailGrid).toHaveBeenCalledTimes(1)
  })

  it("현재 위치 표시를 꺼도 썸네인 점프 버튼은 유지한다", () => {
    const onOpenThumbnailGrid = vi.fn()
    render(
      <WebtoonContinuousView
        images={["/comic/page-1.jpg"]}
        currentIndex={0}
        getOrLoadImage={getOrLoadImage}
        onCenterChange={vi.fn()}
        scrollTarget={null}
        imageGap={0}
        showPageBoundaries={false}
        fitWidth={false}
        showProgress={false}
        thumbnailJump
        onOpenThumbnailGrid={onOpenThumbnailGrid}
      />
    )

    expect(screen.queryByTestId("webtoon-progress")).toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "viewer.webtoon.openThumbnails" }))
    expect(onOpenThumbnailGrid).toHaveBeenCalledTimes(1)
  })

  it("폴더 안 아카이브는 이미지 대신 만화 열기 카드로 그린다", async () => {
    const load = vi.fn(getOrLoadImage)
    const onOpenArchive = vi.fn()
    render(
      <WebtoonContinuousView
        images={["/pics/a.jpg", "/pics/comic.cbz"]}
        currentIndex={0}
        getOrLoadImage={load}
        onCenterChange={vi.fn()}
        scrollTarget={null}
        imageGap={0}
        showPageBoundaries={false}
        fitWidth={false}
        showProgress={false}
        thumbnailJump={false}
        onOpenArchive={onOpenArchive}
      />
    )

    fireEvent.click(await screen.findByRole("button", { name: "viewer.archivePreview.open" }))
    expect(onOpenArchive).toHaveBeenCalledWith("/pics/comic.cbz")
    expect(load).not.toHaveBeenCalledWith("/pics/comic.cbz")
  })

  it("옵저버를 페이지끼리 공유하고, 멀리 지나간 페이지는 자리만 남기고 이미지를 내려놓는다", async () => {
    type Callback = (entries: { target: Element; isIntersecting: boolean }[]) => void
    const observers: {
      root?: Element | null
      rootMargin?: string
      emit: (target: Element, inside: boolean) => void
    }[] = []
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(callback: Callback, options?: { root?: Element | null; rootMargin?: string }) {
          observers.push({
            root: options?.root,
            rootMargin: options?.rootMargin,
            emit: (target, inside) => callback([{ target, isIntersecting: inside }])
          })
        }
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    )

    const { container } = render(
      <WebtoonContinuousView
        images={["/comic/page-1.jpg", "/comic/page-2.jpg", "/comic/page-3.jpg"]}
        currentIndex={0}
        getOrLoadImage={getOrLoadImage}
        onCenterChange={vi.fn()}
        scrollTarget={null}
        imageGap={0}
        showPageBoundaries={false}
        fitWidth={false}
        showProgress={false}
        thumbnailJump={false}
      />
    )

    // 페이지 수와 무관하게 로드용과 유지용 옵저버 하나씩만 만든다.
    expect(observers.map((o) => o.rootMargin)).toEqual(["200% 0px", "400% 0px"])
    // root가 스크롤 컨테이너여야 여백이 컨테이너 밖 페이지까지 닿는다.
    const scroller = container.querySelector("[data-webtoon-scroll-region]")
    expect(observers.map((o) => o.root)).toEqual([scroller, scroller])
    const [near, keep] = observers
    const page = container.querySelector('[data-webtoon-index="1"]') as HTMLElement

    expect(screen.queryByAltText("page-2.jpg")).toBeNull()
    act(() => near.emit(page, true))
    expect(await screen.findByAltText("page-2.jpg")).toBeTruthy()

    // 로드 범위만 벗어나면 그대로 두고, 유지 범위까지 벗어나면 내려놓는다.
    act(() => near.emit(page, false))
    expect(screen.queryByAltText("page-2.jpg")).not.toBeNull()
    act(() => keep.emit(page, false))
    expect(screen.queryByAltText("page-2.jpg")).toBeNull()
    const placeholder = page.firstElementChild as HTMLElement
    expect(placeholder.style.getPropertyValue("--webtoon-page-width")).toBe("800px")
    expect(placeholder.style.getPropertyValue("--webtoon-page-ratio")).toBe("800 / 1200")

    // 유지 범위 보고가 로드 범위 보고보다 늦게 와도 다시 로드한다.
    act(() => near.emit(page, true))
    expect(await screen.findByAltText("page-2.jpg")).toBeTruthy()
    act(() => keep.emit(page, true))
    expect(screen.queryByAltText("page-2.jpg")).not.toBeNull()
  })
})
