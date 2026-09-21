import { describe, expect, it } from "vitest"

import { imageCacheKey } from "@/utils/imageCacheKey"

describe("imageCacheKey", () => {
  it("uses the path alone in disk mode", () => {
    expect(imageCacheKey(null, "D:\\pics\\a.png")).toBe("D:\\pics\\a.png")
  })

  it("scopes archive entries by archive path", () => {
    const a = imageCacheKey("D:\\a.cbz", "001.jpg")
    const b = imageCacheKey("D:\\b.cbz", "001.jpg")
    expect(a).not.toBe(b)
    expect(imageCacheKey("D:\\a.cbz", "001.jpg")).toBe(a)
  })
})
