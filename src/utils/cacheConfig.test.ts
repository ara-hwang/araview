import { describe, it, expect } from "vitest"
import { getCacheLimit, getPrefetchDistance } from "./cacheConfig"

describe("getCacheLimit", () => {
  it('returns 1 for "off" mode', () => {
    expect(getCacheLimit("off")).toBe(1)
  })

  it('returns 24 for "nearby" mode', () => {
    expect(getCacheLimit("nearby")).toBe(24)
  })

  it('returns 64 for "extended" mode', () => {
    expect(getCacheLimit("extended")).toBe(64)
  })
})

describe("getPrefetchDistance", () => {
  it('returns 0 for "off" mode', () => {
    expect(getPrefetchDistance("off")).toBe(0)
  })

  it('returns 1 for "nearby" mode', () => {
    expect(getPrefetchDistance("nearby")).toBe(1)
  })

  it('returns 3 for "extended" mode', () => {
    expect(getPrefetchDistance("extended")).toBe(3)
  })
})
