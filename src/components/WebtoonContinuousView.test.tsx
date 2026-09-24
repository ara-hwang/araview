import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => path
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
})
