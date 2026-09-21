import { describe, expect, it, beforeEach } from "vitest"

import type { ImageInfo } from "@/types"
import {
  clearImageMetaCache,
  getCachedImageInfo,
  getImageMetaCacheSize,
  setCachedImage
} from "@/utils/imageMetaCache"

const sample = (filePath: string): ImageInfo => ({
  file_path: filePath,
  mime_type: "image/png",
  file_name: "a.png",
  file_size: 100,
  width: 1,
  height: 1
})

describe("imageMetaCache", () => {
  beforeEach(() => {
    clearImageMetaCache()
  })

  it("keeps archive entries separate from disk paths with the same name", () => {
    setCachedImage(
      "D:\\book.cbz",
      "page.png",
      sample("/tmp/page-a.png"),
      100,
      () => {},
      50,
      1_000_000
    )
    setCachedImage(null, "page.png", sample("/tmp/disk-page.png"), 100, () => {}, 50, 1_000_000)
    expect(getImageMetaCacheSize()).toBe(2)
    expect(getCachedImageInfo("D:\\book.cbz", "page.png")?.file_path).toBe("/tmp/page-a.png")
    expect(getCachedImageInfo(null, "page.png")?.file_path).toBe("/tmp/disk-page.png")
  })

  it("clearImageMetaCache drops all entries", () => {
    setCachedImage(null, "a.png", sample("/tmp/a.png"), 10, () => {}, 10, 1_000)
    clearImageMetaCache()
    expect(getImageMetaCacheSize()).toBe(0)
  })
})
