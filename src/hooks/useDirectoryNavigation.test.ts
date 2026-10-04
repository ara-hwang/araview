import { act, cleanup, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useDirectoryNavigation } from "@/hooks/useDirectoryNavigation"
import {
  closeImage,
  setImageInfoAndResetView,
  syncDirImagesIndexTo,
  useAppStore
} from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo } from "@/types"

const IMAGES = ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg", "/pics/d.jpg"]

function imageInfoFor(path: string) {
  const file_name = path.split("/").pop() ?? path
  return {
    file_path: path,
    source_path: path,
    file_name,
    file_size: 10,
    mime_type: "image/jpeg",
    width: 800,
    height: 600
  }
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
    inker: null,
    colorist: null,
    letterer: null,
    cover_artist: null,
    editor: null,
    year: null,
    month: null,
    day: null,
    publisher: null,
    genre: null,
    tags: null,
    language_iso: null,
    page_count: null,
    age_rating: null,
    community_rating: null,
    manga: null,
    pages: [{ image, page_type: "FrontCover" }]
  }
}

/** 실제 로더처럼 항목을 그리고 그 목록 위치를 current_index로 맞춘다. */
function paint(path: string) {
  setImageInfoAndResetView(imageInfoFor(path))
  syncDirImagesIndexTo(path)
}

function paintingLoader() {
  return vi.fn(async (path: string, _options?: { refreshDirectory?: boolean }) => paint(path))
}

function setup(index = 0, total: string[] = IMAGES) {
  closeImage()
  useAppStore.setState({
    containerSize: { width: 1000, height: 700 },
    dirImages: { images: total, current_index: index, availability: [] },
    archivePath: null
  })
  useSettingsStore.setState({ loopNavigation: false, viewMode: "single", showCoverAlone: true })
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe("useDirectoryNavigation 로드-표시-이동 플로우", () => {
  it("다음 이동이 이미지를 로드해 표시하고 인덱스를 올린다", async () => {
    setup(0)
    const loadImage = paintingLoader()
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
    const loadImage = paintingLoader()
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
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[0], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(0)
  })

  it("양쪽 모드는 표지 단독이 꺼지면 2장씩 넘긴다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: false })
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[2], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("양쪽 모드 표지 단독에서는 표지 다음이 2페이지다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })
    expect(loadImage).toHaveBeenCalledWith(IMAGES[1], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(1)

    await act(async () => {
      await result.current.navigateImage("next")
    })
    expect(loadImage).toHaveBeenCalledWith(IMAGES[3], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(3)
  })

  it("양쪽 모드 표지 단독에서 2페이지의 이전은 표지다", async () => {
    setup(1)
    useSettingsStore.setState({ viewMode: "right-to-left", showCoverAlone: true })
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("prev")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[0], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(0)
  })

  it("양쪽 모드 점프는 쌍 시작으로 스냅한다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    const loadImage = paintingLoader()
    const { result, rerender } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateToIndex(2)
    })
    expect(loadImage).toHaveBeenCalledWith(IMAGES[1], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(1)

    act(() => {
      useSettingsStore.setState({ showCoverAlone: false })
    })
    rerender()
    await act(async () => {
      await result.current.navigateToIndex(3)
    })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("인덱스 점프가 범위를 clamp한다", async () => {
    setup(0)
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateToIndex(99)
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[3], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(3)
  })

  it("오프셋 점프가 루프 설정에 따라 wrap/clamp된다", async () => {
    setup(0, ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg"])
    const loadImage = paintingLoader()
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

  it("ComicInfo FrontCover가 0번이 아니면 표지 다음이 1페이지다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    useAppStore.setState({ archivePath: "/docs/m.cbz", comicInfo: comicInfoWithCover(1) })
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    // 표지가 1번이면 0번은 혼자 보는 화면이 된다.
    await act(async () => {
      await result.current.navigateImage("next")
    })
    expect(loadImage).toHaveBeenCalledWith(IMAGES[1], { refreshDirectory: false })

    await act(async () => {
      await result.current.navigateImage("next")
    })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("ComicInfo 표지 기준으로 점프 스냅한다", async () => {
    setup(0)
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    useAppStore.setState({ archivePath: "/docs/m.cbz", comicInfo: comicInfoWithCover(1) })
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    // 표지 1 뒤 쌍은 (2,3), (4,5)다.
    await act(async () => {
      await result.current.navigateToIndex(3)
    })
    expect(loadImage).toHaveBeenCalledWith(IMAGES[2], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("아카이브 모드에서는 엔트리 로더를 쓰고 인덱스를 올린다", async () => {
    setup(0)
    useAppStore.setState({ archivePath: "/docs/m.cbz" })
    const loadImage = paintingLoader()
    const loadArchive = vi.fn(async (_archivePath: string, entry: string) => paint(entry))
    const { result } = renderHook(() => useDirectoryNavigation(loadImage, loadArchive))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadArchive).toHaveBeenCalledWith("/docs/m.cbz", IMAGES[1])
    expect(loadImage).not.toHaveBeenCalled()
    expect(useAppStore.getState().dirImages.current_index).toBe(1)
  })

  it("로더가 그리지 않으면(더 새 로드에 밀리면) 인덱스를 쓰지 않는다", async () => {
    setup(0)
    // 밀린 로드는 그리지 않고 정상 종료한다. 이동이 목표 인덱스를 따로 쓰면
    // 그 사이 그려진 다른 이미지와 current_index가 어긋난다.
    const loadImage = vi.fn(async () => {})
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))

    await act(async () => {
      await result.current.navigateImage("next")
    })

    expect(loadImage).toHaveBeenCalledWith(IMAGES[1], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(0)
  })

  it("같은 콜백을 다시 불러도 직전 이동이 그린 위치에서 출발한다", async () => {
    setup(0)
    const loadImage = paintingLoader()
    const { result } = renderHook(() => useDirectoryNavigation(loadImage))
    // 리렌더 전 콜백(키 반복 입력이 잡고 있는 핸들러)을 그대로 쓴다.
    const { navigateImage, navigateByOffset } = result.current

    await act(async () => {
      await navigateImage("next")
    })
    await act(async () => {
      await navigateImage("next")
    })
    expect(loadImage).toHaveBeenLastCalledWith(IMAGES[2], { refreshDirectory: false })
    expect(useAppStore.getState().dirImages.current_index).toBe(2)

    await act(async () => {
      await navigateByOffset(-2)
    })
    expect(useAppStore.getState().dirImages.current_index).toBe(0)
  })
})
