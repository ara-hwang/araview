import { describe, it, expect } from "vitest"

import { classifyError, errorMessage } from "./appError"

describe("classifyError", () => {
  it("classifies missing files", () => {
    expect(classifyError("File not found").kind).toBe("not-found")
    expect(classifyError("Path not found").kind).toBe("not-found")
    expect(classifyError("No images found in directory").kind).toBe("not-found")
    expect(classifyError("Archive not found").kind).toBe("not-found")
    expect(classifyError("No images found in archive").kind).toBe("not-found")
    expect(classifyError("Entry not found: nope.png").kind).toBe("not-found")
  })

  it("classifies permission errors", () => {
    expect(classifyError("permission denied (os error 5)").kind).toBe("permission")
  })

  it("classifies unsupported formats", () => {
    expect(classifyError("Unsupported image format").kind).toBe("unsupported")
    expect(classifyError("Not an archive file").kind).toBe("unsupported")
    expect(classifyError("Unsupported archive format").kind).toBe("unsupported")
    expect(classifyError("PSD files are read-only").kind).toBe("unsupported")
    expect(classifyError("SVG save is not supported").kind).toBe("unsupported")
    expect(classifyError("AVIF save is not supported").kind).toBe("unsupported")
    expect(classifyError("PSB is not supported").kind).toBe("unsupported")
    expect(classifyError("Archive entry too large").kind).toBe("unsupported")
  })

  it("classifies corrupt/unreadable files", () => {
    expect(classifyError("Failed to read directory: corrupt header").kind).toBe("corrupt")
    expect(classifyError("Failed to decode image: bad png").kind).toBe("corrupt")
    expect(classifyError("Failed to read RAR: boom").kind).toBe("corrupt")
    expect(classifyError("Failed to read entry data: boom").kind).toBe("corrupt")
  })

  it("falls back to unknown", () => {
    expect(classifyError("something totally new").kind).toBe("unknown")
  })

  it("prefers backend code over message text", () => {
    // 메시지가 오해를 살 수 있어도 코드가 우선한다
    expect(classifyError({ code: "not_found", message: "Failed to read X" }).kind).toBe("not-found")
    expect(classifyError({ code: "corrupt", message: "File not found" }).kind).toBe("corrupt")
    expect(classifyError({ code: "too_large", message: "anything" }).kind).toBe("unsupported")
    expect(classifyError({ code: "permission", message: "Failed to move" }).kind).toBe("permission")
  })

  it("falls back to message matching for generic codes", () => {
    expect(classifyError({ code: "unknown", message: "permission denied" }).kind).toBe("permission")
    expect(classifyError({ code: "invalid_input", message: "nope" }).kind).toBe("unknown")
  })
})

describe("errorMessage", () => {
  it("extracts message from structured errors", () => {
    expect(errorMessage({ code: "not_found", message: "File not found" })).toBe("File not found")
  })

  it("passes strings through", () => {
    expect(errorMessage("boom")).toBe("boom")
  })

  it("stringifies anything else", () => {
    expect(errorMessage(42)).toBe("42")
  })
})
