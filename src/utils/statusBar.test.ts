import { describe, it, expect } from "vitest"

import { basenameOf, buildStatusModel, parentFolderNameOf } from "./statusBar"

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

describe("buildStatusModel", () => {
  it("builds a file model from the directory entry", () => {
    expect(
      buildStatusModel({
        archivePath: null,
        currentEntry: "D:\\Photos\\vacation\\IMG_1234.jpg",
        fallbackFileName: "IMG_1234.jpg"
      })
    ).toEqual({
      kind: "file",
      folderName: "vacation",
      fileName: "IMG_1234.jpg",
      tooltip: "D:\\Photos\\vacation\\IMG_1234.jpg"
    })
  })

  it("reflects the current entry when browsing recursively", () => {
    const first = buildStatusModel({
      archivePath: null,
      currentEntry: "D:\\Photos\\a.jpg",
      fallbackFileName: null
    })
    const second = buildStatusModel({
      archivePath: null,
      currentEntry: "D:\\Photos\\sub\\b.jpg",
      fallbackFileName: null
    })
    expect(first).toMatchObject({ folderName: "Photos", fileName: "a.jpg" })
    expect(second).toMatchObject({ folderName: "sub", fileName: "b.jpg" })
  })

  it("keeps the archive entry subpath intact", () => {
    expect(
      buildStatusModel({
        archivePath: "D:\\Comics\\Series\\bleach_c01.cbz",
        currentEntry: "ch01/003.jpg",
        fallbackFileName: null
      })
    ).toEqual({
      kind: "archive",
      folderName: "Series",
      archiveName: "bleach_c01.cbz",
      entryName: "ch01/003.jpg",
      tooltip: "D:\\Comics\\Series\\bleach_c01.cbz › ch01/003.jpg"
    })
  })

  it("builds an archive model without an entry", () => {
    expect(
      buildStatusModel({
        archivePath: "D:\\Comics\\bleach_c01.cbz",
        currentEntry: undefined,
        fallbackFileName: null
      })
    ).toEqual({
      kind: "archive",
      folderName: "Comics",
      archiveName: "bleach_c01.cbz",
      entryName: null,
      tooltip: "D:\\Comics\\bleach_c01.cbz"
    })
  })

  it("falls back to the file name without a folder", () => {
    expect(
      buildStatusModel({
        archivePath: null,
        currentEntry: undefined,
        fallbackFileName: "IMG_1234.jpg"
      })
    ).toEqual({
      kind: "file",
      folderName: null,
      fileName: "IMG_1234.jpg",
      tooltip: "IMG_1234.jpg"
    })
  })

  it("returns empty when nothing is known", () => {
    expect(
      buildStatusModel({
        archivePath: null,
        currentEntry: undefined,
        fallbackFileName: null
      })
    ).toEqual({ kind: "empty" })
  })
})
