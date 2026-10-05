import { describe, it, expect } from "vitest"

import { SUPPORTED_IMAGE_EXTENSIONS } from "./imageExtensions"

describe("SUPPORTED_IMAGE_EXTENSIONS", () => {
  it("contains all expected image extensions", () => {
    const expected = [
      "png",
      "apng",
      "jpg",
      "jpeg",
      "gif",
      "bmp",
      "webp",
      "svg",
      "ico",
      "avif",
      "heic",
      "heif",
      "psd",
      "tga",
      "dds",
      "exr",
      "qoi",
      "cbz",
      "zip"
    ]
    expect([...SUPPORTED_IMAGE_EXTENSIONS]).toEqual(expected)
  })

  it("has 19 supported extensions", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toHaveLength(19)
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

  it("includes game and HDR texture formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("tga")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("dds")
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("exr")
  })

  it("includes APNG", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("apng")
  })

  it("includes QOI", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).toContain("qoi")
  })

  it("does not include unsupported formats", () => {
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("psb")
    expect(SUPPORTED_IMAGE_EXTENSIONS).not.toContain("raw")
  })
})
