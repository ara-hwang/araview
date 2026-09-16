import { describe, it, expect } from "vitest"

import { cn } from "./utils"

describe("cn", () => {
  it("merges class names", () => {
    expect(cn("foo", "bar")).toBe("foo bar")
  })

  it("handles conditional classes via clsx", () => {
    expect(cn("base", false && "hidden", "visible")).toBe("base visible")
  })

  it("merges conflicting tailwind classes (last wins)", () => {
    expect(cn("px-2 py-1", "px-4")).toBe("py-1 px-4")
  })

  it("handles undefined and null inputs", () => {
    expect(cn("foo", undefined, null, "bar")).toBe("foo bar")
  })

  it("returns empty string for no arguments", () => {
    expect(cn()).toBe("")
  })

  it("deduplicates identical classes", () => {
    expect(cn("text-red-500", "text-red-500")).toBe("text-red-500")
  })

  it("handles arrays of classes", () => {
    expect(cn(["foo", "bar"], "baz")).toBe("foo bar baz")
  })
})
