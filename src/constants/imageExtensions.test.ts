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
      "psd",
      "cbz",
      "cb7",
      "cbr",
      "rar",
      "zip",
      "7z",
      "cbt"
    ]
    expect([...SUPPORTED_IMAGE_EXTENSIONS]).toEqual(expected)
  })

  it("has 21 supported extensions", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toHaveLength(21)
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
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("psd")
  })

  it("does not include unsupported formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("psb")
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("raw")
  })
})
