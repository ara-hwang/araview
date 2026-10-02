import { act, cleanup, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  invokes: [] as Array<{ cmd: string; args?: Record<string, unknown> }>,
  confirmCalls: 0
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (cmd: string, args?: Record<string, unknown>) => {
    h.invokes.push({ cmd, args })
    return Promise.resolve(null)
  }
}))

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: async () => {
    h.confirmCalls += 1
    return true
  },
  open: vi.fn(async () => null)
}))

import { revealItemInDir } from "@tauri-apps/plugin-opener"

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(async () => {}),
  revealItemInDir: vi.fn(async () => {})
}))

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn()
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

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), info: vi.fn(), success: vi.fn() }
}))

import { toast } from "@/components/ui/toast"
import {
  getEffectivePath,
  getNextPathAfterTrash,
  replacePathInList,
  useFileOperations
} from "@/hooks/useFileOperations"
import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import type { ImageInfo } from "@/types"

describe("getEffectivePath", () => {
  it("아카이브 모드에서는 아카이브 경로를 반환", () => {
    expect(getEffectivePath("/tmp/a.jpg", "/docs/m.cbz")).toBe("/docs/m.cbz")
  })

  it("일반 모드에서는 이미지 경로를 반환", () => {
    expect(getEffectivePath("/pics/a.jpg", null)).toBe("/pics/a.jpg")
  })

  it("둘 다 없으면 null", () => {
    expect(getEffectivePath(null, null)).toBeNull()
  })
})

describe("getNextPathAfterTrash", () => {
  const images = ["/a.jpg", "/b.jpg", "/c.jpg"]

  it("중간 삭제 시 같은 인덱스의 다음 이미지를 반환", () => {
    expect(getNextPathAfterTrash(images, 1, "/b.jpg")).toBe("/c.jpg")
  })

  it("마지막 삭제 시 이전 이미지를 반환", () => {
    expect(getNextPathAfterTrash(images, 2, "/c.jpg")).toBe("/b.jpg")
  })

  it("마지막 1장 삭제 시 null (홈 귀환)", () => {
    expect(getNextPathAfterTrash(["/a.jpg"], 0, "/a.jpg")).toBeNull()
  })
})

describe("replacePathInList", () => {
  it("구 경로만 신 경로로 교체", () => {
    expect(replacePathInList(["/a.jpg", "/b.jpg"], "/a.jpg", "/z.jpg")).toEqual([
      "/z.jpg",
      "/b.jpg"
    ])
  })

  it("없는 경로는 목록 그대로", () => {
    expect(replacePathInList(["/a.jpg"], "/x.jpg", "/z.jpg")).toEqual(["/a.jpg"])
  })
})

const imageInfo = (overrides: Partial<ImageInfo> = {}): ImageInfo => ({
  file_path: "/pics/photo.jpg",
  source_path: "/pics/photo.jpg",
  mime_type: "image/jpeg",
  file_name: "photo.jpg",
  file_size: 10,
  width: 100,
  height: 100,
  ...overrides
})

const noopLoad = async () => {}

beforeEach(() => {
  vi.mocked(toast.error).mockClear()
  h.invokes = []
  h.confirmCalls = 0
})

afterEach(() => {
  cleanup()
})

describe("useFileOperations 아카이브 컨텍스트", () => {
  it("미리보기 모드에서는 휴지통/이름 변경을 차단한다", async () => {
    useAppStore.setState({
      imageInfo: imageInfo({ file_path: "/tmp/extract/page.jpg" }),
      dirImages: { images: ["/docs/comic.cbz"], current_index: 0, availability: [] },
      archivePath: null,
      archivePreviewPath: "/docs/comic.cbz"
    })
    const { result } = renderHook(() => useFileOperations({ loadImage: noopLoad }))

    await act(async () => {
      await result.current.trashCurrent()
      await result.current.renameCurrent("new.jpg")
    })

    // 확인 다이얼로그도, 백엔드 파일 작업도 호출되지 않는다.
    expect(h.confirmCalls).toBe(0)
    const commands = h.invokes.map((call) => call.cmd)
    expect(commands).not.toContain("trash_file")
    expect(commands).not.toContain("rename_file")
  })

  it("미리보기 모드의 경로 복사는 아카이브 경로를 쓴다", async () => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true
    })
    useAppStore.setState({
      imageInfo: imageInfo({ file_path: "/tmp/extract/page.jpg" }),
      dirImages: { images: ["/docs/comic.cbz"], current_index: 0, availability: [] },
      archivePath: null,
      archivePreviewPath: "/docs/comic.cbz"
    })
    const { result } = renderHook(() => useFileOperations({ loadImage: noopLoad }))

    await act(async () => {
      await result.current.copyPathCurrent()
    })

    expect(writeText).toHaveBeenCalledWith("/docs/comic.cbz")
  })

  it("일반 모드 휴지통은 렌더 경로가 아니라 원본(source_path)을 쓴다", async () => {
    useAppStore.setState({
      imageInfo: imageInfo({ file_path: "/tmp/scaled/abc.jpg", source_path: "/pics/photo.jpg" }),
      dirImages: { images: ["/pics/photo.jpg"], current_index: 0, availability: [] },
      archivePath: null,
      archivePreviewPath: null
    })
    const { result } = renderHook(() => useFileOperations({ loadImage: noopLoad }))

    await act(async () => {
      await result.current.trashCurrent()
    })

    const trash = h.invokes.find((call) => call.cmd === "trash_file")
    expect(trash?.args?.filePath).toBe("/pics/photo.jpg")
  })
})

describe("useFileOperations 대상 경로 작업", () => {
  it("대상이 없으면 작업별 empty 토스트를 띄운다", async () => {
    useAppStore.setState({ imageInfo: null, archivePath: null, archivePreviewPath: null })
    const { result } = renderHook(() => useFileOperations({ loadImage: noopLoad }))

    await act(async () => {
      await result.current.revealCurrent()
      await result.current.openExternal()
      await result.current.copyPathCurrent()
    })

    const titles = vi.mocked(toast.error).mock.calls.map((call) => call[0])
    expect(titles).toEqual([
      i18n.t("toast.reveal.empty"),
      i18n.t("toast.external.empty"),
      i18n.t("toast.path.empty")
    ])
    expect(titles).not.toContain("toast.reveal.empty")
  })

  it("작업이 실패하면 fail 토스트에 대상 경로를 담는다", async () => {
    vi.mocked(revealItemInDir).mockRejectedValueOnce(new Error("denied"))
    useAppStore.setState({
      imageInfo: imageInfo(),
      archivePath: null,
      archivePreviewPath: null
    })
    const { result } = renderHook(() => useFileOperations({ loadImage: noopLoad }))

    await act(async () => {
      await result.current.revealCurrent()
    })

    expect(toast.error).toHaveBeenCalledWith(
      i18n.t("toast.reveal.fail"),
      expect.objectContaining({ description: "denied" })
    )
  })
})
