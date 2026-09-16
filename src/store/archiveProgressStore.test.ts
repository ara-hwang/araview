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

import { useArchiveProgressStore } from "@/store/archiveProgressStore"

beforeEach(() => {
  useArchiveProgressStore.setState({ progress: {} })
})

describe("archiveProgressStore", () => {
  it("저장한 엔트리를 그대로 돌려준다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("/comics/a.cbz", "003.jpg")
    expect(useArchiveProgressStore.getState().get("/comics/a.cbz")).toBe("003.jpg")
  })

  it("모르는 아카이브는 null을 반환한다", () => {
    expect(useArchiveProgressStore.getState().get("/comics/b.cbz")).toBeNull()
  })

  it("다시 저장하면 최신 위치로 갱신된다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("/comics/a.cbz", "003.jpg")
    await s.save("/comics/a.cbz", "010.jpg")
    expect(useArchiveProgressStore.getState().get("/comics/a.cbz")).toBe("010.jpg")
  })

  it("빈 경로나 빈 엔트리는 무시한다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("", "003.jpg")
    await s.save("/comics/a.cbz", "")
    expect(useArchiveProgressStore.getState().progress).toEqual({})
  })

  it("remove로 기록을 지운다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("/comics/a.cbz", "003.jpg")
    await s.remove("/comics/a.cbz")
    expect(useArchiveProgressStore.getState().get("/comics/a.cbz")).toBeNull()
  })
})
