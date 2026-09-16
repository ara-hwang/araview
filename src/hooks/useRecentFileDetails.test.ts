import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (p: string) => `asset://${p}`
}))

import { useRecentFileDetails } from "@/hooks/useRecentFileDetails"
import type { ImageInfo } from "@/types"

afterEach(() => {
  cleanup()
})

const imgInfo = (overrides: Partial<ImageInfo> = {}): ImageInfo => ({
  file_path: "/pics/a.png",
  mime_type: "image/png",
  file_name: "a.png",
  file_size: 2048,
  width: 800,
  height: 600,
  ...overrides
})

describe("useRecentFileDetails", () => {
  it("이미지는 info와 src를 함께 반환한다", async () => {
    const getOrLoadImage = vi.fn(async () => imgInfo())
    const { result } = renderHook(() => useRecentFileDetails(["/pics/a.png"], getOrLoadImage))

    await waitFor(() => {
      expect(result.current.get("/pics/a.png")?.status).toBe("done")
    })
    const detail = result.current.get("/pics/a.png")
    expect(detail?.info?.file_name).toBe("a.png")
    expect(detail?.src).toBe("asset:///pics/a.png")
  })

  it("아카이브는 src 없이 done으로 반환한다", async () => {
    const getOrLoadImage = vi.fn(async () =>
      imgInfo({
        file_path: "/docs/comic.cbz",
        file_name: "comic.cbz",
        mime_type: "application/vnd.comicbook+zip",
        width: null,
        height: null
      })
    )
    const { result } = renderHook(() => useRecentFileDetails(["/docs/comic.cbz"], getOrLoadImage))

    await waitFor(() => {
      expect(result.current.get("/docs/comic.cbz")?.status).toBe("done")
    })
    expect(result.current.get("/docs/comic.cbz")?.src).toBeUndefined()
  })

  it("실패한 경로는 error 상태로 남긴다", async () => {
    const getOrLoadImage = vi.fn(async () => {
      throw new Error("missing")
    })
    const { result } = renderHook(() => useRecentFileDetails(["/pics/gone.png"], getOrLoadImage))

    await waitFor(() => {
      expect(result.current.get("/pics/gone.png")?.status).toBe("error")
    })
  })
})
