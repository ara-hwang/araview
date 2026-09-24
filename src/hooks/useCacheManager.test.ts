import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useCacheInvalidationStore } from "@/store/cacheInvalidationStore"
import type { CacheClearResult, CacheStats } from "@/types"

const h = vi.hoisted(() => ({
  invoke: vi.fn()
}))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: h.invoke
}))

import { useCacheManager } from "@/hooks/useCacheManager"

const stats: CacheStats = {
  storage_mode: "persistent",
  persistent_available: true,
  total_bytes: 1024,
  file_count: 2,
  protected_bytes: 0,
  protected_file_count: 0,
  total_limit_bytes: 2 * 1024 * 1024 * 1024,
  categories: [
    {
      key: "thumbnails",
      bytes: 1024,
      file_count: 2,
      protected_bytes: 0,
      protected_file_count: 0,
      limit_bytes: 500 * 1024 * 1024
    }
  ]
}

beforeEach(() => {
  cleanup()
  h.invoke.mockReset()
  useCacheInvalidationStore.setState({ epoch: 0 })
  h.invoke.mockImplementation(async (command: string) => {
    if (command === "get_cache_stats") return stats
    if (command === "clear_cache") {
      return {
        removed_bytes: 1024,
        removed_file_count: 2,
        failed_file_count: 0,
        stats: { ...stats, total_bytes: 0, file_count: 0, categories: [] }
      } satisfies CacheClearResult
    }
    throw new Error(`unexpected command: ${command}`)
  })
})

afterEach(() => {
  cleanup()
})

describe("useCacheManager", () => {
  it("초기 마운트 시 통계를 불러온다", async () => {
    const { result } = renderHook(() => useCacheManager())

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.stats?.total_bytes).toBe(1024)
    expect(h.invoke).toHaveBeenCalledWith("get_cache_stats")
  })

  it("삭제 성공 후 통계를 갱신하고 캐시 무효화 epoch를 증가시킨다", async () => {
    const { result } = renderHook(() => useCacheManager())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(async () => {
      await result.current.clear("thumbnails")
    })

    expect(h.invoke).toHaveBeenCalledWith("clear_cache", { scope: "thumbnails" })
    expect(result.current.stats?.total_bytes).toBe(0)
    expect(useCacheInvalidationStore.getState().epoch).toBe(1)
  })
})
