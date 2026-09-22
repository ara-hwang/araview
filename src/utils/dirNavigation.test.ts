import { describe, expect, it } from "vitest"

import { resolveOffsetIndex, resolveStepIndex } from "@/utils/dirNavigation"

describe("resolveStepIndex", () => {
  it("기본 next/prev는 한 칸씩 이동한다", () => {
    expect(resolveStepIndex(1, 5, 1, false, "next")).toBe(2)
    expect(resolveStepIndex(1, 5, 1, false, "prev")).toBe(0)
  })

  it("양면 보기(step 2)는 두 칸씩 이동한다", () => {
    expect(resolveStepIndex(0, 6, 2, false, "next")).toBe(2)
    expect(resolveStepIndex(4, 6, 2, false, "prev")).toBe(2)
  })

  it("비루프 prev는 첫 장에서 멈춘다", () => {
    expect(resolveStepIndex(0, 5, 1, false, "prev")).toBeNull()
  })

  it("비루프 next는 끝에서 clamp하고 마지막 장에서는 멈춘다", () => {
    expect(resolveStepIndex(3, 5, 2, false, "next")).toBe(4)
    expect(resolveStepIndex(4, 5, 1, false, "next")).toBeNull()
  })

  it("루프면 양 끝에서 wrap한다", () => {
    expect(resolveStepIndex(0, 5, 1, true, "prev")).toBe(4)
    expect(resolveStepIndex(4, 5, 1, true, "next")).toBe(0)
    expect(resolveStepIndex(4, 5, 2, true, "next")).toBe(1)
  })

  it("장수가 1장 이하면 이동하지 않는다", () => {
    expect(resolveStepIndex(0, 1, 1, true, "next")).toBeNull()
    expect(resolveStepIndex(0, 0, 1, false, "next")).toBeNull()
  })
})

describe("resolveOffsetIndex", () => {
  it("비루프는 양 끝으로 clamp한다", () => {
    expect(resolveOffsetIndex(2, 5, 10, false)).toBe(4)
    expect(resolveOffsetIndex(2, 5, -10, false)).toBe(0)
    expect(resolveOffsetIndex(1, 5, 2, false)).toBe(3)
  })

  it("루프는 wrap한다", () => {
    expect(resolveOffsetIndex(4, 5, 3, true)).toBe(2)
    expect(resolveOffsetIndex(0, 5, -1, true)).toBe(4)
  })
})
