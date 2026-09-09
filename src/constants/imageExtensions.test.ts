import { describe, it, expect } from "vitest"
import { SUPPORTED_IMAGE_EXTENSIONS } from "./imageExtensions"

describe("SUPPORTED_IMAGE_EXTENSIONS", () => {
  it("contains all expected image extensions", () => {
    const expected = [
      "png",
      "jpg",
      "jpeg",
      "gif",
      "bmp",
      "webp",
      "svg",
      "ico",
      "tiff",
      "tif",
      "avif",
      "heic",
      "heif",
      "cbz",
      "cb7"
    ]
    expect([...SUPPORTED_IMAGE_EXTENSIONS]).toEqual(expected)
  })

  it("has 15 supported extensions", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toHaveLength(15)
  })

  it("includes common web image formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("png")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("jpg")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("webp")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("gif")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("svg")
  })

  it("includes modern image formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("avif")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("heic")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("heif")
  })

  it("does not include unsupported formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("psd")
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("raw")
  })
})
