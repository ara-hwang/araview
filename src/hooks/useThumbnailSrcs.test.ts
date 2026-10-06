import { renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  batches: [] as string[][],
  /** 설정하면 배치 응답이 이 약속을 기다린다 (요청 중 상태 재현). */
  gate: null as Promise<void> | null
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "generate_thumbnails_batch") {
      const filePaths = args?.filePaths as string[]
      h.batches.push(filePaths)
      return (h.gate ?? Promise.resolve()).then(() =>
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
  h.gate = null
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

  it("요청 중에 목록이 바뀌어도 받은 결과를 쓰고 같은 경로를 다시 요청하지 않는다", async () => {
    let open = () => {}
    h.gate = new Promise<void>((resolve) => {
      open = resolve
    })
    const { result, rerender } = renderHook(
      ({ paths }) => useThumbnailSrcs(paths, getOrLoad, OPTIONS),
      { initialProps: { paths: ["/a.png", "/b.png"] } }
    )
    await waitFor(() => expect(h.batches).toHaveLength(1))

    rerender({ paths: ["/b.png", "/c.png"] })
    await waitFor(() => expect(h.batches).toHaveLength(2))
    open()
    await waitFor(() => expect(result.current.urls.size).toBe(2))

    expect(h.batches).toEqual([["/a.png", "/b.png"], ["/c.png"]])
    expect(result.current.urls.has("/a.png")).toBe(false)
    expect(result.current.urls.get("/b.png")).toBe("asset:///thumbs/b.png")
  })

  it("보이는 창을 청크로 나눠 먼저 요청하고 나머지를 잇는다", async () => {
    const paths = ["/1.png", "/2.png", "/3.png", "/4.png", "/5.png"]
    const options = { chunkDelayMs: 0, chunkSize: 2, priorityPaths: ["/3.png", "/4.png", "/5.png"] }
    const { result } = renderHook(() => useThumbnailSrcs(paths, getOrLoad, options))
    await waitFor(() => expect(result.current.urls.size).toBe(5))
    expect(h.batches).toEqual([["/3.png", "/4.png"], ["/5.png"], ["/1.png", "/2.png"]])
  })
})
