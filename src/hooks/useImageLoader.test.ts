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

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), info: vi.fn(), success: vi.fn() }
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

import { toast } from "@/components/ui/toast"
import { getEffectiveViewMode } from "@/hooks/useEffectiveViewMode"
import { useImageLoader } from "@/hooks/useImageLoader"
import i18n from "@/i18n"
import { applyManualViewMode, closeImage, useAppStore } from "@/store/appStore"
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
  vi.mocked(toast.error).mockClear()
  vi.mocked(toast.info).mockClear()
  closeImage()
  useArchiveProgressStore.setState({ progress: {} })
  useSettingsStore.setState({
    viewMode: "single",
    loopNavigation: false,
    resumeReading: true,
    showCoverAlone: true,
    comicAutoDualView: false,
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

describe("useImageLoader 양쪽 이어보기", () => {
  it("표지 단독 양쪽 모드에서 쌍 중간 위치는 쌍 시작으로 연다", async () => {
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

describe("useImageLoader 만화 자동 양쪽 보기", () => {
  async function openArchive() {
    const { result } = renderHook(() => useImageLoader())
    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })
  }

  it("옵션이 켜져 있으면 ComicInfo 없이도 좌→우 양쪽 보기로 연다", async () => {
    useSettingsStore.setState({ comicAutoDualView: true })

    await openArchive()

    expect(useAppStore.getState().comicViewMode).toBe("left-to-right")
    expect(getEffectiveViewMode()).toBe("left-to-right")
  })

  it("ComicInfo Manga가 YesAndRightToLeft면 우→좌로 연다", async () => {
    useSettingsStore.setState({ comicAutoDualView: true })
    setupInvoke({
      get_comic_info: async () => ({ ...comicInfo("만화"), manga: "YesAndRightToLeft" })
    })

    await openArchive()

    expect(useAppStore.getState().comicViewMode).toBe("right-to-left")
  })

  it("옵션이 꺼져 있으면 설정된 보기 모드를 그대로 쓴다", async () => {
    await openArchive()

    expect(useAppStore.getState().comicViewMode).toBeNull()
    expect(getEffectiveViewMode()).toBe("single")
  })

  it("보기 모드를 직접 고르면 자동 결정이 해제된다", async () => {
    useSettingsStore.setState({ comicAutoDualView: true })
    await openArchive()

    applyManualViewMode("single")

    expect(useAppStore.getState().comicViewMode).toBeNull()
    expect(getEffectiveViewMode()).toBe("single")
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

describe("useImageLoader 손상 파일 건너뛰기", () => {
  const DIR = ["/pics/a.jpg", "/pics/b.jpg", "/pics/c.jpg"]

  function setupBrokenFile(broken: string, loaded: string[]) {
    setupInvoke({
      load_image: async (args) => {
        const path = String(args?.filePath)
        loaded.push(path)
        if (path === broken) throw { code: "not_found", message: "File not found" }
        return { ...imgInfo(path), source_path: path, file_name: path.split("/").pop() ?? path }
      },
      get_directory_images: async () => ({ images: [...DIR], current_index: 0, availability: [] })
    })
    useAppStore.setState({
      dirImages: { images: [...DIR], current_index: 0, availability: [] },
      failedPaths: []
    })
  }

  it("켜져 있으면 실패한 파일을 표시하지 않고 다음 파일로 이동한다", async () => {
    useSettingsStore.setState({ skipBrokenFiles: true, cacheMode: "off" })
    const loaded: string[] = []
    setupBrokenFile("/pics/b.jpg", loaded)
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage("/pics/b.jpg", { refreshDirectory: false })
    })

    const st = useAppStore.getState()
    expect(st.imageInfo?.source_path).toBe("/pics/c.jpg")
    expect(st.dirImages.current_index).toBe(2)
    expect(st.failedPaths).toContain("/pics/b.jpg")
    expect(st.loading).toBe(false)
    expect(vi.mocked(toast.info)).toHaveBeenCalledWith(i18n.t("toast.load.skipped"), {
      description: "/pics/b.jpg"
    })
  })

  it("꺼져 있으면 오류를 남기고 다음 파일을 읽지 않는다", async () => {
    useSettingsStore.setState({ skipBrokenFiles: false, cacheMode: "off" })
    const loaded: string[] = []
    setupBrokenFile("/pics/b.jpg", loaded)
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage("/pics/b.jpg", { refreshDirectory: false })
    })

    const st = useAppStore.getState()
    expect(st.imageInfo).toBeNull()
    expect(st.error).toBe("File not found")
    expect(st.errorCode).toBe("not_found")
    expect(st.loading).toBe(false)
    expect(loaded).not.toContain("/pics/c.jpg")
    expect(vi.mocked(toast.error)).toHaveBeenCalledWith(
      i18n.t("toast.load.imageFail"),
      expect.objectContaining({ description: "File not found" })
    )
  })

  it("아카이브 엔트리가 실패하면 다음 엔트리로 이동한다", async () => {
    useSettingsStore.setState({ skipBrokenFiles: true, cacheMode: "off" })
    setupInvoke({
      load_archive_image: async (args) => {
        const entry = String(args?.entryName)
        if (entry === "002.jpg") throw { code: "corrupt", message: "bad entry" }
        return imgInfo(entry)
      }
    })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })
    await act(async () => {
      await result.current.loadArchiveImageByIndex(ARCHIVE_PATH, "002.jpg")
    })

    const st = useAppStore.getState()
    expect(st.imageInfo?.file_name).toBe("003.jpg")
    expect(st.dirImages.current_index).toBe(2)
    expect(st.failedPaths).toContain("002.jpg")
    expect(st.loading).toBe(false)
  })
})

describe("useImageLoader 아카이브 열기 실패", () => {
  const failingArchive = () =>
    setupInvoke({
      get_archive_images: async () => {
        throw { code: "corrupt", message: "Failed to read ZIP" }
      }
    })

  it("보던 이미지가 있으면 화면을 되돌리고 알림만 띄운다", async () => {
    const viewing = imgInfo("/pics/a.jpg")
    useAppStore.setState({
      imageInfo: viewing,
      dirImages: { images: ["/pics/a.jpg", "/pics/b.jpg"], current_index: 0, availability: [] }
    })
    failingArchive()
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    const st = useAppStore.getState()
    expect(st.imageInfo).toEqual(viewing)
    expect(st.archivePath).toBeNull()
    expect(st.error).toBeNull()
    expect(st.dirImages.images).toEqual(["/pics/a.jpg", "/pics/b.jpg"])
    expect(st.loading).toBe(false)
    expect(vi.mocked(toast.error)).toHaveBeenCalledWith(
      i18n.t("toast.load.archiveFail"),
      expect.objectContaining({ description: "Failed to read ZIP" })
    )
  })

  it("보던 이미지가 없으면 오류 상태로 남긴다", async () => {
    failingArchive()
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })

    const st = useAppStore.getState()
    expect(st.imageInfo).toBeNull()
    expect(st.archivePath).toBeNull()
    expect(st.error).toBe("Failed to read ZIP")
    expect(st.errorCode).toBe("corrupt")
  })
})

describe("useImageLoader 주변 예열", () => {
  const FOLDER = [
    "/pics/0.jpg",
    "/pics/1.jpg",
    "/pics/2.jpg",
    "/pics/3.jpg",
    "/pics/4.jpg",
    "/pics/5.jpg"
  ]

  function setupFolder(loaded: string[]) {
    setupInvoke({
      load_image: async (args) => {
        const path = String(args?.filePath)
        loaded.push(path)
        return { ...imgInfo(path), source_path: path }
      }
    })
    useAppStore.setState({
      archivePath: null,
      dirImages: { images: [...FOLDER], current_index: 2, availability: [] }
    })
  }

  it("캐시 모드가 꺼져 있으면 미리 읽지 않는다", async () => {
    useSettingsStore.setState({ viewMode: "webtoon", cacheMode: "off" })
    const loaded: string[] = []
    setupFolder(loaded)
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      result.current.prefetchAround(2)
      await new Promise((r) => setTimeout(r, 20))
    })
    expect(loaded).toEqual([])
  })

  it("웹툰은 캐시 모드 거리의 두 배만큼 앞뒤를 읽는다", async () => {
    useSettingsStore.setState({ viewMode: "webtoon", cacheMode: "nearby" })
    const loaded: string[] = []
    setupFolder(loaded)
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      result.current.prefetchAround(2)
      await new Promise((r) => setTimeout(r, 20))
    })
    expect([...loaded].sort()).toEqual(["/pics/0.jpg", "/pics/1.jpg", "/pics/3.jpg", "/pics/4.jpg"])
  })
})

describe("useImageLoader 아카이브 첫 페이지 캐시", () => {
  it("만화로 연 첫 페이지는 메타 캐시에 남아 다시 추출하지 않는다", async () => {
    const extracted: string[] = []
    setupInvoke({
      load_archive_image: async (args) => {
        extracted.push(String(args?.entryName))
        return imgInfo(String(args?.entryName))
      }
    })
    useArchiveProgressStore.setState({ progress: {} })
    const { result } = renderHook(() => useImageLoader())

    await act(async () => {
      await result.current.loadImage(ARCHIVE_PATH, { archiveOpen: "full" })
    })
    const firstCount = extracted.filter((e) => e === ENTRIES[0]).length
    await act(async () => {
      await result.current.getOrLoadImage(ENTRIES[0])
    })

    expect(firstCount).toBe(1)
    expect(extracted.filter((e) => e === ENTRIES[0])).toHaveLength(1)
  })
})
