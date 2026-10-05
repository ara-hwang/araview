import { beforeEach, describe, expect, it } from "vitest"

import { recordWidePage, useWidePageStore } from "@/store/widePageStore"

describe("recordWidePage", () => {
  beforeEach(() => {
    useWidePageStore.setState({ scope: null, paths: new Set() })
  })

  it("가로가 더 긴 페이지만 기록한다", () => {
    recordWidePage(null, "/p/wide.jpg", { width: 3000, height: 2000 })
    recordWidePage(null, "/p/tall.jpg", { width: 1500, height: 2000 })
    recordWidePage(null, "/p/square.jpg", { width: 2000, height: 2000 })
    recordWidePage(null, "/p/unknown.jpg", { width: null, height: null })
    expect([...useWidePageStore.getState().paths]).toEqual(["/p/wide.jpg"])
  })

  it("같은 항목을 다시 기록해도 집합을 바꾸지 않는다", () => {
    recordWidePage("a.cbz", "003.jpg", { width: 3000, height: 2000 })
    const before = useWidePageStore.getState().paths
    recordWidePage("a.cbz", "003.jpg", { width: 3000, height: 2000 })
    expect(useWidePageStore.getState().paths).toBe(before)
  })

  it("목록 범위가 바뀌면 이전 기록을 버린다", () => {
    recordWidePage("a.cbz", "003.jpg", { width: 3000, height: 2000 })
    recordWidePage("b.cbz", "010.jpg", { width: 3000, height: 2000 })
    const state = useWidePageStore.getState()
    expect(state.scope).toBe("b.cbz")
    expect([...state.paths]).toEqual(["010.jpg"])
  })
})
