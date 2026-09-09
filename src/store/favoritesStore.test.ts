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

import { useFavoritesStore } from "@/store/favoritesStore"

beforeEach(() => {
  useFavoritesStore.setState({ files: [] })
})

describe("favoritesStore", () => {
  it("toggle 추가/제거를 반복", async () => {
    expect(await useFavoritesStore.getState().toggle("/a.png")).toBe(true)
    expect(useFavoritesStore.getState().files).toEqual(["/a.png"])
    expect(await useFavoritesStore.getState().toggle("/a.png")).toBe(false)
    expect(useFavoritesStore.getState().files).toEqual([])
  })

  it("중복 추가 시 맨 앞으로 이동", async () => {
    const s = useFavoritesStore.getState()
    await s.add("/a.png")
    await s.add("/b.png")
    await s.add("/a.png")
    expect(useFavoritesStore.getState().files).toEqual(["/a.png", "/b.png"])
  })

  it("replace는 순서 유지", async () => {
    const s = useFavoritesStore.getState()
    await s.add("/a.png")
    await s.add("/b.png")
    await s.replace("/b.png", "/c.png")
    expect(useFavoritesStore.getState().files).toEqual(["/c.png", "/a.png"])
  })

  it("isFavorite 확인", async () => {
    await useFavoritesStore.getState().add("/a.png")
    expect(useFavoritesStore.getState().isFavorite("/a.png")).toBe(true)
    expect(useFavoritesStore.getState().isFavorite("/x.png")).toBe(false)
  })
})
