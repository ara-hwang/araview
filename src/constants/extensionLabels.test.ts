import { describe, expect, it, beforeAll } from "vitest"

import { initI18n } from "@/i18n"

import { EXTENSION_LABELS, extensionLabel, isSupportedExtension } from "./extensionLabels"
import { SUPPORTED_IMAGE_EXTENSIONS } from "./imageExtensions"

beforeAll(async () => {
  await initI18n("ko")
})

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

  it("returns the mapped label for known extensions", async () => {
    await initI18n("ko")
    expect(extensionLabel("png")).toBe("PNG 이미지")
    expect(extensionLabel("apng")).toBe("APNG 이미지")
    expect(extensionLabel("psd")).toBe("PSD 이미지")
    expect(extensionLabel("cbz")).toBe("만화 아카이브")
    await initI18n("en")
    expect(extensionLabel("png")).toBe("PNG image")
    expect(extensionLabel("apng")).toBe("APNG image")
    expect(extensionLabel("psd")).toBe("PSD image")
    expect(extensionLabel("cbz")).toBe("Comic archive")
    await initI18n("ko")
  })
})
