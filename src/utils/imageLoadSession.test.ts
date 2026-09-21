import { describe, expect, it } from "vitest"

import { beginImageLoad, isCurrentImageLoad, resetImageLoadSession } from "@/utils/imageLoadSession"

describe("imageLoadSession", () => {
  it("marks only the latest load as current", () => {
    resetImageLoadSession()
    const first = beginImageLoad()
    const second = beginImageLoad()
    expect(isCurrentImageLoad(first)).toBe(false)
    expect(isCurrentImageLoad(second)).toBe(true)
  })
})
