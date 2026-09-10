import { describe, expect, it } from "vitest"
import {
  buildDirListOptions,
  resolveRefreshedIndex
} from "@/utils/directoryOptions"

describe("buildDirListOptions", () => {
  it("설정을 백엔드 페이로드(camelCase)로 변환", () => {
    expect(
      buildDirListOptions({
        sortKey: "size",
        sortDescending: true,
        shuffle: false,
        includeSubfolders: true
      })
    ).toEqual({
      sortKey: "size",
      descending: true,
      shuffle: false,
      recursive: true
    })
  })

  it("기본값은 이름 오름차순", () => {
    expect(
      buildDirListOptions({
        sortKey: "name",
        sortDescending: false,
        shuffle: false,
        includeSubfolders: false
      })
    ).toEqual({
      sortKey: "name",
      descending: false,
      shuffle: false,
      recursive: false
    })
  })
})

describe("resolveRefreshedIndex", () => {
  it("같은 파일을 새 목록에서 찾아 복원", () => {
    expect(
      resolveRefreshedIndex(["a.png", "b.png", "c.png"], 1, [
        "c.png",
        "b.png",
        "a.png"
      ])
    ).toBe(1)
  })

  it("사라진 파일은 범위 안으로 clamp", () => {
    expect(resolveRefreshedIndex(["a.png", "b.png"], 1, ["a.png"])).toBe(0)
    expect(resolveRefreshedIndex(["a.png"], 0, [])).toBe(0)
  })
})
