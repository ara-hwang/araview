import { describe, it, expect } from "vitest"

import { basenameOf, parentFolderNameOf } from "./path"

describe("basenameOf", () => {
  it("handles backslash paths", () => {
    expect(basenameOf("D:\\Photos\\vacation\\IMG_1234.jpg")).toBe("IMG_1234.jpg")
  })

  it("handles forward slash paths", () => {
    expect(basenameOf("D:/Photos/vacation/IMG.jpg")).toBe("IMG.jpg")
  })

  it("handles UNC paths", () => {
    expect(basenameOf("\\\\NAS\\share\\pics\\a.png")).toBe("a.png")
  })

  it("returns empty for empty input", () => {
    expect(basenameOf("")).toBe("")
  })
})

describe("parentFolderNameOf", () => {
  it("returns the parent folder name", () => {
    expect(parentFolderNameOf("D:\\Photos\\vacation\\IMG_1234.jpg")).toBe("vacation")
  })

  it("returns null when there is no parent", () => {
    expect(parentFolderNameOf("IMG.jpg")).toBeNull()
    expect(parentFolderNameOf("")).toBeNull()
  })
})
