import { describe, it, expect } from "vitest"
import { formatFileSize } from "./format"

describe("formatFileSize", () => {
  it("returns bytes for values under 1 KB", () => {
    expect(formatFileSize(0)).toBe("0 B")
    expect(formatFileSize(1)).toBe("1 B")
    expect(formatFileSize(512)).toBe("512 B")
    expect(formatFileSize(1023)).toBe("1023 B")
  })

  it("returns KB for values from 1 KB to under 1 MB", () => {
    expect(formatFileSize(1024)).toBe("1.0 KB")
    expect(formatFileSize(1536)).toBe("1.5 KB")
    expect(formatFileSize(10240)).toBe("10.0 KB")
    expect(formatFileSize(1024 * 1024 - 1)).toBe("1024.0 KB")
  })

  it("returns MB for values 1 MB and above", () => {
    expect(formatFileSize(1024 * 1024)).toBe("1.0 MB")
    expect(formatFileSize(1024 * 1024 * 2.5)).toBe("2.5 MB")
    expect(formatFileSize(1024 * 1024 * 100)).toBe("100.0 MB")
    expect(formatFileSize(1024 * 1024 * 1024)).toBe("1024.0 MB")
  })
})
