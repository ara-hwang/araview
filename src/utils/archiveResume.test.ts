import { describe, expect, it } from "vitest"

import { resolveArchiveStartIndex } from "@/utils/archiveResume"

const images = ["001.jpg", "002.jpg", "003.jpg", "004.jpg"]

describe("resolveArchiveStartIndex", () => {
  it("저장된 엔트리가 중간에 있으면 그 위치에서 시작", () => {
    expect(resolveArchiveStartIndex(images, "003.jpg")).toBe(2)
  })

  it("저장된 엔트리가 첫 페이지면 0에서 시작", () => {
    expect(resolveArchiveStartIndex(images, "001.jpg")).toBe(0)
  })

  it("기록이 없으면 0에서 시작", () => {
    expect(resolveArchiveStartIndex(images, null)).toBe(0)
  })

  it("목록에 없는 엔트리는 0으로 되돌린다", () => {
    expect(resolveArchiveStartIndex(images, "999.jpg")).toBe(0)
  })

  it("빈 목록은 0을 반환한다", () => {
    expect(resolveArchiveStartIndex([], "001.jpg")).toBe(0)
  })
})
