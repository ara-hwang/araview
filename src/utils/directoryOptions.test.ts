import { describe, expect, it } from "vitest"
import { buildDirListOptions } from "@/utils/directoryOptions"

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
