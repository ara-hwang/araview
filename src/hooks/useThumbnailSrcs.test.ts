import { renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  batches: [] as string[][]
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "generate_thumbnails_batch") {
      const filePaths = args?.filePaths as string[]
      h.batches.push(filePaths)
      return Promise.resolve(
        filePaths.map((source) => ({
          source,
          thumb: { file_path: `/thumbs${source}`, width: 1, height: 1 },
          error: null
        }))
      )
    }
    return Promise.resolve(null)
  }
}))

import { useThumbnailSrcs } from "@/hooks/useThumbnailSrcs"
import type { ImageInfo } from "@/types"

const getOrLoad = (path: string): Promise<ImageInfo> => Promise.reject(new Error(path))
const OPTIONS = { chunkDelayMs: 0 }

beforeEach(() => {
  h.batches = []
})

describe("useThumbnailSrcs 목록 변경", () => {
  it("내용이 같은 새 배열로 다시 렌더해도 다시 로드하지 않는다", async () => {
    const { result, rerender } = renderHook(
      ({ paths }) => useThumbnailSrcs(paths, getOrLoad, OPTIONS),
      { initialProps: { paths: ["/a.png", "/b.png"] } }
    )
    await waitFor(() => expect(result.current.urls.size).toBe(2))
    expect(h.batches).toEqual([["/a.png", "/b.png"]])

    rerender({ paths: ["/a.png", "/b.png"] })
    await new Promise((r) => setTimeout(r, 20))
    expect(h.batches).toHaveLength(1)
    expect(result.current.urls.get("/a.png")).toBe("asset:///thumbs/a.png")
  })

  it("경로가 추가되면 새 경로만 이어서 로드한다", async () => {
    const { result, rerender } = renderHook(
      ({ paths }) => useThumbnailSrcs(paths, getOrLoad, OPTIONS),
      { initialProps: { paths: ["/a.png"] } }
    )
    await waitFor(() => expect(result.current.urls.size).toBe(1))

    rerender({ paths: ["/a.png", "/c.png"] })
    await waitFor(() => expect(result.current.urls.size).toBe(2))
    expect(h.batches).toEqual([["/a.png"], ["/c.png"]])
  })
})
