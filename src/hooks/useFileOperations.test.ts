import { describe, expect, it } from "vitest"

import {
  getEffectivePath,
  getNextPathAfterTrash,
  isAvifImage,
  isPsdImage,
  isSvgImage,
  replacePathInList,
  saveBlockedReason
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

describe("isPsdImage", () => {
  it("PSD MIME이면 sidecar 경로와 무관하게 true", () => {
    expect(
      isPsdImage({
        mime_type: "image/vnd.adobe.photoshop",
        file_name: "design.psd"
      })
    ).toBe(true)
  })

  it("파일명 대소문자와 무관하게 .psd를 판정", () => {
    expect(isPsdImage({ mime_type: "image/png", file_name: "DESIGN.PSD" })).toBe(true)
  })

  it("일반 이미지와 null은 false", () => {
    expect(isPsdImage({ mime_type: "image/png", file_name: "a.png" })).toBe(false)
    expect(isPsdImage(null)).toBe(false)
  })
})

describe("isSvgImage", () => {
  it("SVG MIME이면 true", () => {
    expect(isSvgImage({ mime_type: "image/svg+xml", file_name: "icon.svg" })).toBe(true)
  })

  it("파일명으로 .svg를 판정", () => {
    expect(isSvgImage({ mime_type: "image/png", file_name: "ICON.SVG" })).toBe(true)
  })

  it("일반 이미지와 null은 false", () => {
    expect(isSvgImage({ mime_type: "image/png", file_name: "a.png" })).toBe(false)
    expect(isSvgImage(null)).toBe(false)
  })
})

describe("saveBlockedReason", () => {
  it("PSD는 noPsd", () => {
    expect(
      saveBlockedReason({
        mime_type: "image/vnd.adobe.photoshop",
        file_name: "a.psd"
      })
    ).toBe("noPsd")
  })

  it("SVG는 noSvg", () => {
    expect(saveBlockedReason({ mime_type: "image/svg+xml", file_name: "a.svg" })).toBe("noSvg")
  })

  it("AVIF는 noAvif", () => {
    expect(saveBlockedReason({ mime_type: "image/avif", file_name: "a.avif" })).toBe("noAvif")
    expect(isAvifImage({ mime_type: "image/png", file_name: "A.AVIF" })).toBe(true)
    expect(isAvifImage(null)).toBe(false)
  })

  it("저장 가능 포맷과 null은 null", () => {
    expect(saveBlockedReason({ mime_type: "image/png", file_name: "a.png" })).toBeNull()
    expect(saveBlockedReason({ mime_type: "image/jpeg", file_name: "a.jpg" })).toBeNull()
    expect(saveBlockedReason(null)).toBeNull()
  })
})

describe("replacePathInList", () => {
  it("구 경로만 신 경로로 교체", () => {
    expect(replacePathInList(["/a.jpg", "/b.jpg"], "/a.jpg", "/z.jpg")).toEqual([
      "/z.jpg",
      "/b.jpg"
    ])
  })

  it("없는 경로는 목록 그대로", () => {
    expect(replacePathInList(["/a.jpg"], "/x.jpg", "/z.jpg")).toEqual(["/a.jpg"])
  })
})
