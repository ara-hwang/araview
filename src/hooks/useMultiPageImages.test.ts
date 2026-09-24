import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useMultiPageImages } from "@/hooks/useMultiPageImages"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo, ImageInfo } from "@/types"

const IMAGES = [
  "/pics/1.jpg",
  "/pics/2.jpg",
  "/pics/3.jpg",
  "/pics/4.jpg",
  "/pics/5.jpg",
  "/pics/6.jpg"
]

const imgInfo = (path: string): ImageInfo => ({
  file_path: path,
  source_path: path,
  mime_type: "image/jpeg",
  file_name: path.split("/").pop() ?? path,
  file_size: 10,
  width: 800,
  height: 600
})

function setup(index: number) {
  closeImage()
  useAppStore.setState({
    dirImages: { images: IMAGES, current_index: index, availability: [] }
  })
  useSettingsStore.setState({
    viewMode: "left-to-right",
    loopNavigation: false,
    showCoverAlone: true
  })
}

/** ComicInfo 표지 판정용 최소 메타데이터. */
function comicInfoWithCover(image: number): ComicInfo {
  return {
    title: null,
    series: null,
    number: null,
    count: null,
    volume: null,
    summary: null,
    writer: null,
    penciller: null,
    publisher: null,
    genre: null,
    tags: null,
    language_iso: null,
    page_count: null,
    age_rating: null,
    community_rating: null,
    pages: [{ image, page_type: "FrontCover" }]
  }
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
})

afterEach(() => {
  cleanup()
})

describe("useMultiPageImages 양면 오프셋", () => {
  it("표지 단독이 켜지면 첫 장만 로드한다", async () => {
    setup(0)
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(1)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[0]])
    expect(getOrLoadImage).toHaveBeenCalledTimes(1)
  })

  it("표지 단독이 켜지면 두 번째 화면부터 두 장을 로드한다", async () => {
    setup(1)
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(2)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[1], IMAGES[2]])
  })

  it("표지 단독이 꺼지면 첫 화면부터 두 장을 로드한다", async () => {
    setup(0)
    useSettingsStore.setState({ showCoverAlone: false })
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(2)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[0], IMAGES[1]])
  })

  it("단일 모드에서는 페이지를 로드하지 않는다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "single" })
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(0)
    })
    expect(getOrLoadImage).not.toHaveBeenCalled()
  })

  it("ComicInfo FrontCover가 0번이 아니면 표지를 기준으로 짝을 맞춘다", async () => {
    setup(3)
    useAppStore.setState({ comicInfo: comicInfoWithCover(3) })
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(1)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[3]])
    expect(getOrLoadImage).toHaveBeenCalledTimes(1)
  })

  it("ComicInfo 표지 뒤 페이지는 표지 기준 쌍으로 로드한다", async () => {
    setup(4)
    useAppStore.setState({ comicInfo: comicInfoWithCover(3) })
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(2)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[4], IMAGES[5]])
  })

  it("표지 바로 앞에 남는 페이지도 단독으로 로드한다", async () => {
    setup(2)
    useAppStore.setState({ comicInfo: comicInfoWithCover(3) })
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(1)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[2]])
  })

  it("마지막에 남은 한 장은 단독 렌더용으로 하나만 반환한다", async () => {
    // 6장 + 표지 단독: 마지막 화면(5번)은 짝이 없어 한 장만 남는다.
    setup(5)
    const getOrLoadImage = vi.fn(async (path: string) => imgInfo(path))
    const { result } = renderHook(() => useMultiPageImages(getOrLoadImage))

    await waitFor(() => {
      expect(result.current.pages).toHaveLength(1)
    })
    expect(result.current.pages.map((p) => p.path)).toEqual([IMAGES[5]])
  })
})
