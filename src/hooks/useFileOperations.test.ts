import { describe, expect, it } from "vitest"
import {
  getEffectivePath,
  getNextPathAfterTrash,
  replacePathInList
} from "@/hooks/useFileOperations"

describe("getEffectivePath", () => {
  it("아카이브 모드에서는 아카이브 경로를 반환", () => {
    expect(getEffectivePath("/tmp/a.jpg", "/docs/m.cbz")).toBe("/docs/m.cbz")
  })

  it("일반 모드에서는 이미지 경로를 반환", () => {
    expect(getEffectivePath("/pics/a.jpg", null)).toBe("/pics/a.jpg")
  })

  it("둘 다 없으면 null", () => {
    expect(getEffectivePath(null, null)).toBeNull()
  })
})

describe("getNextPathAfterTrash", () => {
  const images = ["/a.jpg", "/b.jpg", "/c.jpg"]

  it("중간 삭제 시 같은 인덱스의 다음 이미지를 반환", () => {
    expect(getNextPathAfterTrash(images, 1, "/b.jpg")).toBe("/c.jpg")
  })

  it("마지막 삭제 시 이전 이미지를 반환", () => {
    expect(getNextPathAfterTrash(images, 2, "/c.jpg")).toBe("/b.jpg")
  })

  it("마지막 1장 삭제 시 null (홈 귀환)", () => {
    expect(getNextPathAfterTrash(["/a.jpg"], 0, "/a.jpg")).toBeNull()
  })
})

describe("replacePathInList", () => {
  it("구 경로만 신 경로로 교체", () => {
    expect(replacePathInList(["/a.jpg", "/b.jpg"], "/a.jpg", "/z.jpg")).toEqual(
      ["/z.jpg", "/b.jpg"]
    )
  })

  it("없는 경로는 목록 그대로", () => {
    expect(replacePathInList(["/a.jpg"], "/x.jpg", "/z.jpg")).toEqual([
      "/a.jpg"
    ])
  })
})
