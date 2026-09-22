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

import { sanitizeProgress, useArchiveProgressStore } from "@/store/archiveProgressStore"

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
    expect(useArchiveProgressStore.getState().getProgress("/comics/b.cbz")).toBeNull()
  })

  it("다시 저장하면 최신 위치로 갱신된다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("/comics/a.cbz", "003.jpg")
    await s.save("/comics/a.cbz", "010.jpg")
    expect(useArchiveProgressStore.getState().get("/comics/a.cbz")).toBe("010.jpg")
  })

  it("index/total을 함께 저장하고 유지한다", async () => {
    const s = useArchiveProgressStore.getState()
    await s.save("/comics/a.cbz", "010.jpg", { index: 9, total: 340 })
    expect(useArchiveProgressStore.getState().getProgress("/comics/a.cbz")).toEqual({
      entry: "010.jpg",
      index: 9,
      total: 340
    })
    // meta 없이 갱신하면 이전 index/total을 유지한다
    await s.save("/comics/a.cbz", "011.jpg")
    expect(useArchiveProgressStore.getState().getProgress("/comics/a.cbz")).toEqual({
      entry: "011.jpg",
      index: 9,
      total: 340
    })
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

describe("sanitizeProgress", () => {
  it("현재 형식(object)을 복원한다", () => {
    const result = sanitizeProgress({
      "/comics/a.cbz": { entry: "010.jpg", index: 9, total: 340 }
    })
    expect(result["/comics/a.cbz"]).toEqual({ entry: "010.jpg", index: 9, total: 340 })
  })

  it("구버전(string) 저장값을 이어보기 가능한 기록으로 이전한다", () => {
    const result = sanitizeProgress({ "/comics/old.cbz": "005.jpg" })
    expect(result["/comics/old.cbz"]).toEqual({ entry: "005.jpg", index: -1, total: 0 })
  })

  it("손상된 항목은 버린다", () => {
    const result = sanitizeProgress({
      "": { entry: "001.jpg", index: 0, total: 10 },
      "/comics/no-entry.cbz": { index: 3, total: 10 },
      "/comics/empty-entry.cbz": { entry: "", index: 3, total: 10 },
      "/comics/negative.cbz": { entry: "001.jpg", index: -4, total: -1 }
    })
    expect(result).toEqual({
      "/comics/negative.cbz": { entry: "001.jpg", index: -1, total: 0 }
    })
  })

  it("객체가 아닌 입력은 빈 맵을 반환한다", () => {
    for (const bad of [null, undefined, 42, "oops", []]) {
      expect(sanitizeProgress(bad)).toEqual({})
    }
  })
})
