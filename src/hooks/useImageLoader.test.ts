import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  calls: [] as string[],
  impl: null as null | ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>)
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (cmd: string, args?: Record<string, unknown>) => {
    h.calls.push(cmd)
    if (!h.impl) return Promise.reject(new Error(`unmocked invoke: ${cmd}`))
    return h.impl(cmd, args)
  }
}))

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => null)
}))

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => ({
      get: vi.fn(async () => null),
      set: vi.fn(async () => {}),
      save: vi.fn(async () => {})
    }))
  }
}))

import { useImageLoader } from "@/hooks/useImageLoader"
import { closeImage, useAppStore } from "@/store/appStore"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo, DirectoryImages, ImageInfo } from "@/types"

const ARCHIVE_PATH = "/docs/comic.cbz"
const ENTRIES = ["001.jpg", "002.jpg", "003.jpg", "004.jpg", "005.jpg"]

const imgInfo = (entry: string): ImageInfo => ({
  file_path: `/temp/${entry}`,
  source_path: `/temp/${entry}`,
  mime_type: "image/jpeg",
  file_name: entry,
  file_size: 100,
  width: 800,
  height: 1200
})

const comicInfo = (series: string, pages: ComicInfo["pages"] = null): ComicInfo => ({
  title: null,
  series,
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
  pages
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function setupInvoke(
  overrides: Record<string, (args?: Record<string, unknown>) => Promise<unknown>> = {}
) {
  h.calls = []
  h.impl = (cmd, args) => {
    const override = overrides[cmd]
    if (override) return override(args)
    switch (cmd) {
      case "get_archive_images":
        return Promise.resolve<DirectoryImages>({
          images: [...ENTRIES],
          current_index: 0,
          availability: []
        })
      case "load_archive_image":
        return Promise.resolve(imgInfo(String(args?.entryName)))
      case "get_comic_info":
        return Promise.resolve(null)
      case "archive_prefetch":
        return Promise.resolve(0)
      default:
        return Promise.resolve(null)
    }
  }
}

beforeEach(() => {
  cleanup()
  closeImage()
  useArchiveProgressStore.setState({ progress: {} })
  useSettingsStore.setState({
    viewMode: "single",
    loopNavigation: false,
    resumeReading: true,
    showCoverAlone: true,
    recordRecentFiles: false,
    imageScalingMode: "auto",
    autoDetectPixelArt: true
  })
  setupInvoke()
})

afterEach(() => {
  cleanup()
})

describe("useImageLoader ComicInfo", () => {
  it("아카이브를 열면 ComicInfo를 커밋한다", async () => {
    setupInvoke({ get_comic_info: async () => comicInfo("테스트 시리즈") })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    expect(useAppStore.getState().comicInfo?.series).toBe("테스트 시리즈")
    expect(useAppStore.getState().comicInfoError).toBeNull()
  })

  it("깨진 ComicInfo는 에러로 남기고 이미지는 그대로 표시한다", async () => {
    setupInvoke({
      get_comic_info: async () => {
        throw new Error("Failed to parse ComicInfo.xml")
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    const state = useAppStore.getState()
    expect(state.comicInfo).toBeNull()
    expect(state.comicInfoError).toContain("Failed to parse ComicInfo.xml")
    expect(state.imageInfo?.file_name).toBe(ENTRIES[0])
    expect(state.archivePath).toBe(ARCHIVE_PATH)
  })

  it("이전 로드의 ComicInfo는 최신 로드를 덮지 않는다", async () => {
    const first = deferred<ComicInfo | null>()
    let comicCalls = 0
    setupInvoke({
      get_comic_info: () => {
        comicCalls += 1
        return comicCalls === 1 ? first.promise : Promise.resolve(comicInfo("두번째"))
      }
    })
    const { result } = renderHook(() => useImageLoader())

    let firstLoad: Promise<void> | null = null
    await act(async () => {
      firstLoad = result.current.loadImage("/docs/one.cbz", { archiveOpen: "full" })
    })
    await act(async () => {
      await result.current.loadImage("/docs/two.cbz", { archiveOpen: "full" })
    })
    expect(useAppStore.getState().comicInfo?.series).toBe("두번째")

    first.resolve(comicInfo("첫번째"))
    await act(async () => {
      await firstLoad
    })

    expect(useAppStore.getState().comicInfo?.series).toBe("두번째")
  })

  it("폴더 미리보기에서는 ComicInfo를 읽지 않는다", async () => {
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      // archiveOpen 기본값이 folderPreview다.
      await result.current.loadImage(ARCHIVE_PATH)
    })

    expect(h.calls).not.toContain("get_comic_info")
    expect(useAppStore.getState().archivePreviewPath).toBe(ARCHIVE_PATH)
  })

  it("일반 이미지를 열면 이전 ComicInfo를 비운다", async () => {
    useAppStore.setState({ comicInfo: comicInfo("남은 값"), comicInfoError: "남은 에러" })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage("/pics/a.jpg")
    })

    expect(useAppStore.getState().comicInfo).toBeNull()
    expect(useAppStore.getState().comicInfoError).toBeNull()
  })
})

describe("useImageLoader 양면 이어보기", () => {
  it("표지 단독 양면 모드에서 쌍 중간 위치는 쌍 시작으로 연다", async () => {
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    useArchiveProgressStore.setState({
      progress: {
        [ARCHIVE_PATH]: { entry: ENTRIES[2], index: 2, total: ENTRIES.length }
      }
    })
    const loaded: string[] = []
    setupInvoke({
      load_archive_image: async (args) => {
        const entry = String(args?.entryName)
        loaded.push(entry)
        return imgInfo(entry)
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    // 3번째 페이지에서 이어보면 2-3 쌍이 아니라 2번째 페이지(쌍 시작)로 연다.
    expect(loaded[0]).toBe(ENTRIES[1])
    expect(useAppStore.getState().dirImages.current_index).toBe(1)
  })

  it("표지 단독이 꺼지면 짝수 쌍 시작으로 스냅한다", async () => {
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: false })
    useArchiveProgressStore.setState({
      progress: {
        [ARCHIVE_PATH]: { entry: ENTRIES[3], index: 3, total: ENTRIES.length }
      }
    })
    const loaded: string[] = []
    setupInvoke({
      load_archive_image: async (args) => {
        const entry = String(args?.entryName)
        loaded.push(entry)
        return imgInfo(entry)
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    // 쌍 스냅은 표지 단독 설정과 무관하게 적용한다 (4번째 → [3,4] 쌍 시작).
    expect(loaded[0]).toBe(ENTRIES[2])
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })

  it("ComicInfo 표지가 0번이 아니면 표지 기준 쌍으로 이어본다", async () => {
    useSettingsStore.setState({ viewMode: "left-to-right", showCoverAlone: true })
    useArchiveProgressStore.setState({
      progress: {
        [ARCHIVE_PATH]: { entry: ENTRIES[3], index: 3, total: ENTRIES.length }
      }
    })
    const loaded: string[] = []
    setupInvoke({
      get_comic_info: async () => comicInfo("표지 테스트", [{ image: 1, page_type: "FrontCover" }]),
      load_archive_image: async (args) => {
        const entry = String(args?.entryName)
        loaded.push(entry)
        return imgInfo(entry)
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    // 표지 1 뒤 쌍은 (2,3)이므로 4번째 페이지에서 이어보면 3번째 페이지로 연다.
    expect(loaded[0]).toBe(ENTRIES[2])
    expect(useAppStore.getState().dirImages.current_index).toBe(2)
    expect(useAppStore.getState().comicInfo?.series).toBe("표지 테스트")
  })

  it("단일 모드에서는 저장 위치를 스냅하지 않는다", async () => {
    useSettingsStore.setState({ viewMode: "single" })
    useArchiveProgressStore.setState({
      progress: {
        [ARCHIVE_PATH]: { entry: ENTRIES[2], index: 2, total: ENTRIES.length }
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    expect(useAppStore.getState().dirImages.current_index).toBe(2)
  })
})

describe("useImageLoader 표시 해상도 상한", () => {
  it("표시 정책을 바꾸면 sidecar에 새 보간 정책을 전달하고 현재 이미지를 다시 로드한다", async () => {
    useSettingsStore.setState({
      maxResolution: "original",
      imageScalingMode: "auto",
      autoDetectPixelArt: true
    })
    const policies: Array<[unknown, unknown]> = []
    setupInvoke({
      load_image: async (args) => {
        policies.push([args?.imageScalingMode, args?.autoDetectPixelArt])
        return { ...imgInfo(String(args?.filePath)), file_name: "policy.jpg" }
      },
      get_directory_images: async () => ({
        images: ["/pics/policy.jpg"],
        current_index: 0,
        availability: []
      })
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage("/pics/policy.jpg")
    })
    expect(policies).toEqual([["auto", true]])

    await act(async () => {
      useSettingsStore.setState({ imageScalingMode: "smooth", autoDetectPixelArt: false })
    })
    await waitFor(() => {
      expect(policies).toEqual([
        ["auto", true],
        ["smooth", false]
      ])
    })
  })

  it("상한이 바뀌면 새 maxSide로 현재 이미지를 다시 로드한다", async () => {
    useSettingsStore.setState({ maxResolution: "original" })
    const maxSides: Array<number | null> = []
    setupInvoke({
      load_image: async (args) => {
        maxSides.push((args?.maxSide as number | null | undefined) ?? null)
        return { ...imgInfo(String(args?.filePath)), file_name: "a.jpg" }
      },
      get_directory_images: async () => ({
        images: ["/pics/a.jpg"],
        current_index: 0,
        availability: []
      })
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage("/pics/a.jpg")
    })
    expect(maxSides).toEqual([null])

    await act(async () => {
      useSettingsStore.setState({ maxResolution: "4k" })
    })

    await waitFor(() => {
      expect(maxSides).toEqual([null, 3840])
    })
    expect(useAppStore.getState().imageInfo?.file_name).toBe("a.jpg")
  })
})
