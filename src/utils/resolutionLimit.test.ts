import { describe, expect, it } from "vitest"

import { maxSideForResolution } from "@/utils/resolutionLimit"

describe("maxSideForResolution", () => {
  it("원본은 제한 없음(null)", () => {
    expect(maxSideForResolution("original")).toBeNull()
  })

  it("4k는 긴 변 3840px", () => {
    expect(maxSideForResolution("4k")).toBe(3840)
  })

  it("1080p는 긴 변 1920px", () => {
    expect(maxSideForResolution("1080p")).toBe(1920)
  })
})
