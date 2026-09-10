import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => ({
      get: vi.fn(async () => null),
      set: vi.fn(async () => {}),
      save: vi.fn(async () => {})
    }))
  }
}))

import { MAX_PALETTE_MRU, usePaletteMruStore } from "@/store/paletteMruStore"

beforeEach(() => {
  usePaletteMruStore.setState({ ids: [] })
})

describe("paletteMruStore", () => {
  it("최근 실행을 맨 앞에 쌓는다", async () => {
    const s = usePaletteMruStore.getState()
    await s.push("zoomIn")
    await s.push("copyImage")
    expect(usePaletteMruStore.getState().ids).toEqual(["copyImage", "zoomIn"])
  })

  it("중복 실행 시 맨 앞으로 이동한다", async () => {
    const s = usePaletteMruStore.getState()
    await s.push("zoomIn")
    await s.push("copyImage")
    await s.push("zoomIn")
    expect(usePaletteMruStore.getState().ids).toEqual(["zoomIn", "copyImage"])
  })

  it(`최대 ${MAX_PALETTE_MRU}개를 유지한다`, async () => {
    const s = usePaletteMruStore.getState()
    const ids = [
      "zoomIn",
      "zoomOut",
      "copyImage",
      "navigatePrev",
      "navigateNext",
      "rotateCW",
      "flipH"
    ] as const
    for (const id of ids) await s.push(id)
    const stored = usePaletteMruStore.getState().ids
    expect(stored.length).toBe(MAX_PALETTE_MRU)
    expect(stored[0]).toBe("flipH")
  })

  it("유효하지 않은 id는 무시한다", async () => {
    const s = usePaletteMruStore.getState()
    // @ts-expect-error 유효하지 않은 id 입력
    await s.push("no-such-command")
    expect(usePaletteMruStore.getState().ids).toEqual([])
  })
})
