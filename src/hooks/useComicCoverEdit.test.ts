import { act, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  invoke: vi.fn()
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: h.invoke
}))

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), info: vi.fn(), success: vi.fn() }
}))

import { toast } from "@/components/ui/toast"
import { isCoverPage, useComicCoverEdit } from "@/hooks/useComicCoverEdit"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo, ComicPage } from "@/types"

const ARCHIVE = "/docs/m.cbz"
const IMAGES = ["1.jpg", "2.jpg", "3.jpg", "4.jpg"]

/** 백엔드 응답을 흉내 낸다. 테스트는 `pages`만 본다. */
const comicWith = (pages: ComicPage[]) => ({ pages }) as ComicInfo

function setup(archivePath: string | null = ARCHIVE) {
  closeImage()
  useAppStore.setState({
    archivePath,
    dirImages: { images: IMAGES, current_index: 0, availability: [] }
  })
  useSettingsStore.setState({ showCoverAlone: true })
}

async function toggle(index: number, onChanged = vi.fn()) {
  const { result } = renderHook(() => useComicCoverEdit(onChanged))
  await act(async () => {
    await result.current.toggleCover(index)
  })
  return onChanged
}

beforeEach(() => {
  vi.clearAllMocks()
  setup()
})

describe("useComicCoverEdit", () => {
  it("기본 표지(0번)를 해제하면 빈 표지 목록을 저장하고 그 페이지로 다시 맞춘다", async () => {
    h.invoke.mockResolvedValue(comicWith([{ image: 0, page_type: "Story" }]))
    expect(isCoverPage(0)).toBe(true)

    const onChanged = await toggle(0)

    expect(h.invoke).toHaveBeenCalledWith("set_comic_cover_pages", {
      filePath: ARCHIVE,
      coverPages: []
    })
    expect(isCoverPage(0)).toBe(false)
    expect(toast.success).toHaveBeenCalledTimes(1)
    expect(onChanged).toHaveBeenCalledWith(0)
  })

  it("표지에 붙은 페이지를 지정하면 연속 표지로 저장한다", async () => {
    h.invoke.mockResolvedValue(
      comicWith([
        { image: 0, page_type: "FrontCover" },
        { image: 1, page_type: "FrontCover" }
      ])
    )

    await toggle(1)

    expect(h.invoke).toHaveBeenCalledWith("set_comic_cover_pages", {
      filePath: ARCHIVE,
      coverPages: [0, 1]
    })
    expect(isCoverPage(1)).toBe(true)
  })

  it("저장에 실패하면 메타데이터를 바꾸지 않고 오류를 알린다", async () => {
    h.invoke.mockRejectedValue({ code: "permission", message: "Access is denied" })

    const onChanged = await toggle(2)

    expect(useAppStore.getState().comicInfo).toBeNull()
    expect(toast.error).toHaveBeenCalledTimes(1)
    expect(onChanged).not.toHaveBeenCalled()
  })

  it("아카이브가 아니면 아무것도 하지 않는다", async () => {
    setup(null)
    expect(isCoverPage(0)).toBe(false)

    await toggle(0)

    expect(h.invoke).not.toHaveBeenCalled()
  })
})
