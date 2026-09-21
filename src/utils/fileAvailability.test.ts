import { describe, expect, it } from "vitest"

import type { DirectoryImages } from "@/types"

import { availabilityAt, cloudOnlyPathSet, isCloudOnlyPath } from "./fileAvailability"

const dir = (images: string[], availability: DirectoryImages["availability"]): DirectoryImages => ({
  images,
  current_index: 0,
  availability
})

describe("fileAvailability", () => {
  it("marks cloud_only paths", () => {
    const listing = dir(["/a.jpg", "/b.jpg"], ["local", "cloud_only"])
    expect(isCloudOnlyPath(listing, "/b.jpg")).toBe(true)
    expect(isCloudOnlyPath(listing, "/a.jpg")).toBe(false)
    expect(cloudOnlyPathSet(listing)).toEqual(new Set(["/b.jpg"]))
  })

  it("returns unknown when availability is missing", () => {
    const listing = dir(["/a.jpg"], [])
    expect(availabilityAt(listing, 0)).toBe("unknown")
    expect(isCloudOnlyPath(listing, "/a.jpg")).toBe(false)
  })
})
