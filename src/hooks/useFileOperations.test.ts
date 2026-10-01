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

import {
  getEffectivePath,
  getNextPathAfterTrash,
  isAvifImage,
  isPsdImage,
  isSvgImage,
  replacePathInList,
  saveBlockedReason,
  useFileOperations
} from "@/hooks/useFileOperations"
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

describe("isPsdImage", () => {
  it("PSD MIME이면 sidecar 경로와 무관하게 true", () => {
    expect(
      isPsdImage({
        mime_type: "image/vnd.adobe.photoshop",
        file_name: "design.psd"
      })
    ).toBe(true)
  })

  it("파일명 대소문자와 무관하게 .psd를 판정", () => {
    expect(isPsdImage({ mime_type: "image/png", file_name: "DESIGN.PSD" })).toBe(true)
  })

  it("일반 이미지와 null은 false", () => {
    expect(isPsdImage({ mime_type: "image/png", file_name: "a.png" })).toBe(false)
    expect(isPsdImage(null)).toBe(false)
  })
})

describe("isSvgImage", () => {
  it("SVG MIME이면 true", () => {
    expect(isSvgImage({ mime_type: "image/svg+xml", file_name: "icon.svg" })).toBe(true)
  })

  it("파일명으로 .svg를 판정", () => {
    expect(isSvgImage({ mime_type: "image/png", file_name: "ICON.SVG" })).toBe(true)
  })

  it("일반 이미지와 null은 false", () => {
    expect(isSvgImage({ mime_type: "image/png", file_name: "a.png" })).toBe(false)
    expect(isSvgImage(null)).toBe(false)
  })
})

describe("saveBlockedReason", () => {
  it("PSD는 noPsd", () => {
    expect(
      saveBlockedReason({
        mime_type: "image/vnd.adobe.photoshop",
        file_name: "a.psd"
      })
    ).toBe("noPsd")
  })

  it("TGA/DDS/EXR은 noReadOnly", () => {
    expect(saveBlockedReason({ mime_type: "image/x-tga", file_name: "a.tga" })).toBe("noReadOnly")
    expect(saveBlockedReason({ mime_type: "image/jpeg", file_name: "B.DDS" })).toBe("noReadOnly")
    expect(saveBlockedReason({ mime_type: "image/x-exr", file_name: "c.exr" })).toBe("noReadOnly")
  })

  it("SVG는 noSvg", () => {
    expect(saveBlockedReason({ mime_type: "image/svg+xml", file_name: "a.svg" })).toBe("noSvg")
  })

  it("AVIF는 noAvif", () => {
    expect(saveBlockedReason({ mime_type: "image/avif", file_name: "a.avif" })).toBe("noAvif")
    expect(isAvifImage({ mime_type: "image/png", file_name: "A.AVIF" })).toBe(true)
    expect(isAvifImage(null)).toBe(false)
  })

  it("저장 가능 포맷과 null은 null", () => {
    expect(saveBlockedReason({ mime_type: "image/png", file_name: "a.png" })).toBeNull()
    expect(saveBlockedReason({ mime_type: "image/jpeg", file_name: "a.jpg" })).toBeNull()
    expect(saveBlockedReason(null)).toBeNull()
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
  h.invokes = []
  h.confirmCalls = 0
})

afterEach(() => {
  cleanup()
})

describe("useFileOperations 아카이브 컨텍스트", () => {
  it("미리보기 모드에서는 휴지통/이름 변경/저장을 차단한다", async () => {
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
      await result.current.saveEdits({
        rotationCw: 90,
        flipH: false,
        flipV: false,
        format: null,
        overwrite: false,
        newFileName: null
      })
    })

    // 확인 다이얼로그도, 백엔드 파일 작업도 호출되지 않는다.
    expect(h.confirmCalls).toBe(0)
    const commands = h.invokes.map((call) => call.cmd)
    expect(commands).not.toContain("trash_file")
    expect(commands).not.toContain("rename_file")
    expect(commands).not.toContain("save_image_edits")
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
