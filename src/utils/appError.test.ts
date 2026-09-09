import { describe, it, expect } from "vitest"
import { classifyError } from "./appError"

describe("classifyError", () => {
  it("classifies missing files", () => {
    expect(classifyError("File not found").kind).toBe("not-found")
    expect(classifyError("Path not found").kind).toBe("not-found")
    expect(classifyError("No images found in directory").kind).toBe("not-found")
  })

  it("classifies permission errors", () => {
    expect(classifyError("permission denied (os error 5)").kind).toBe(
      "permission"
    )
  })

  it("classifies unsupported formats", () => {
    expect(classifyError("Unsupported image format").kind).toBe("unsupported")
    expect(classifyError("Not an archive file").kind).toBe("unsupported")
  })

  it("classifies corrupt/unreadable files", () => {
    expect(classifyError("Failed to read directory: corrupt header").kind).toBe(
      "corrupt"
    )
  })

  it("falls back to unknown", () => {
    expect(classifyError("something totally new").kind).toBe("unknown")
  })
})
