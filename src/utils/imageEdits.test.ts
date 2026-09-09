import { describe, expect, it } from "vitest"
import { buildSaveEditsPayload, describeTransform } from "@/utils/imageEdits"

describe("describeTransform", () => {
  it("변경 없으면 표시", () => {
    expect(describeTransform(0, false, false)).toBe("변경 없음")
  })

  it("회전+반전 조합 요약", () => {
    expect(describeTransform(90, true, false)).toBe("회전 90° · 좌우 반전")
    expect(describeTransform(0, false, true)).toBe("상하 반전")
  })
})

describe("buildSaveEditsPayload", () => {
  it("원본 유지는 format null", () => {
    expect(buildSaveEditsPayload(90, true, false, "keep", true, "")).toEqual({
      rotationCw: 90,
      flipH: true,
      flipV: false,
      format: null,
      overwrite: true,
      newFileName: null
    })
  })

  it("덮어쓰기면 파일명 무시, 새 파일이면 trim", () => {
    expect(
      buildSaveEditsPayload(0, false, false, "jpg", false, "  b.jpg ")
    ).toEqual({
      rotationCw: 0,
      flipH: false,
      flipV: false,
      format: "jpg",
      overwrite: false,
      newFileName: "b.jpg"
    })
  })
})
