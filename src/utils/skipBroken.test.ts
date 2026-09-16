import { describe, expect, it } from "vitest"

import { findSkipTarget } from "./skipBroken"

const images = ["a.png", "b.png", "c.png", "d.png"]

describe("findSkipTarget", () => {
  it("실패 다음 인덱스를 반환한다", () => {
    expect(findSkipTarget(images, 0, new Set(["a.png"]), false)).toBe(1)
  })

  it("이미 실패한 후보는 건너뛴다", () => {
    expect(findSkipTarget(images, 0, new Set(["a.png", "b.png"]), false)).toBe(2)
  })

  it("loop가 꺼져 있으면 끝에서 null을 반환한다", () => {
    expect(findSkipTarget(images, 3, new Set(["d.png"]), false)).toBeNull()
  })

  it("loop가 켜져 있으면 처음으로 감싼다", () => {
    expect(findSkipTarget(images, 3, new Set(["d.png"]), true)).toBe(0)
  })

  it("모두 실패했으면 null을 반환한다", () => {
    expect(findSkipTarget(images, 0, new Set(images), false)).toBeNull()
  })

  it("빈 목록이면 null을 반환한다", () => {
    expect(findSkipTarget([], 0, new Set(), true)).toBeNull()
  })
})
