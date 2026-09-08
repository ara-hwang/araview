import { describe, expect, it } from "vitest"
import { SUPPORTED_IMAGE_EXTENSIONS } from "./imageExtensions"
import {
  EXTENSION_LABELS,
  extensionLabel,
  isSupportedExtension
} from "./extensionLabels"

describe("extensionLabels", () => {
  it("covers every supported extension", () => {
    for (const ext of SUPPORTED_IMAGE_EXTENSIONS) {
      expect(EXTENSION_LABELS[ext]).toBeTruthy()
      expect(isSupportedExtension(ext)).toBe(true)
    }
  })

  it("returns a fallback label for unknown extensions", () => {
    expect(extensionLabel("pdf")).toBe("PDF")
    expect(isSupportedExtension("pdf")).toBe(false)
  })

  it("returns the mapped label for known extensions", () => {
    expect(extensionLabel("png")).toBe("PNG 이미지")
    expect(extensionLabel("cbz")).toBe("만화 아카이브")
  })
})
