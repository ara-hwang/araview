import { describe, expect, it } from "vitest"

import { computeStripWindow, stripOffsetForIndex, stripScrollToReveal } from "@/utils/stripWindow"

const base = { itemSize: 48, gap: 4, padding: 4 }

describe("stripOffsetForIndex", () => {
  it("패딩과 간격을 반영한 시작 오프셋을 돌려준다", () => {
    expect(stripOffsetForIndex(0, 48, 4, 4)).toBe(4)
    expect(stripOffsetForIndex(3, 48, 4, 4)).toBe(4 + 3 * 52)
  })
})

describe("computeStripWindow", () => {
  it("항목이 없으면 빈 창을 돌려준다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 0,
      viewportSize: 400,
      itemCount: 0
    })
    expect(win).toEqual({ startIndex: 0, endIndex: 0, totalSize: 0 })
  })

  it("전체 크기는 패딩과 간격을 포함한다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 0,
      viewportSize: 400,
      itemCount: 10
    })
    expect(win.totalSize).toBe(4 * 2 + 10 * 48 + 9 * 4)
  })

  it("뷰포트에 들어가는 항목만 overscan과 함께 그린다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 0,
      viewportSize: 200,
      itemCount: 100,
      overscan: 2
    })
    expect(win.startIndex).toBe(0)
    // 200px 뷰포트에 4개 남짓 + 앞뒤 overscan 2가 더해진다.
    expect(win.endIndex).toBe(9)
  })

  it("스크롤이 내려가면 창도 따라 내려간다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 520,
      viewportSize: 200,
      itemCount: 100,
      overscan: 2
    })
    expect(win.startIndex).toBe(7)
    expect(win.endIndex).toBe(16)
  })

  it("끝에서는 항목 수를 넘지 않는다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 10000,
      viewportSize: 200,
      itemCount: 12
    })
    expect(win.endIndex).toBe(12)
    expect(win.startIndex).toBeLessThan(12)
  })

  it("뷰포트가 0이어도 안전한 창을 돌려준다", () => {
    const win = computeStripWindow({
      ...base,
      scrollOffset: 0,
      viewportSize: 0,
      itemCount: 5,
      overscan: 0
    })
    expect(win.startIndex).toBe(0)
    expect(win.endIndex).toBeGreaterThan(0)
    expect(win.endIndex).toBeLessThanOrEqual(5)
  })
})

describe("stripScrollToReveal", () => {
  it("이미 보이면 스크롤을 바꾸지 않는다", () => {
    expect(stripScrollToReveal({ ...base, index: 2, scrollOffset: 100, viewportSize: 200 })).toBe(
      100
    )
  })

  it("왼쪽으로 벗어나면 앞쪽으로 맞춘다", () => {
    const next = stripScrollToReveal({
      ...base,
      index: 1,
      scrollOffset: 300,
      viewportSize: 200
    })
    expect(next).toBeLessThan(300)
    expect(next).toBe(stripOffsetForIndex(1, 48, 4, 4) - 4)
  })

  it("오른쪽으로 벗어나면 뒤쪽으로 맞춘다", () => {
    const next = stripScrollToReveal({
      ...base,
      index: 20,
      scrollOffset: 0,
      viewportSize: 200
    })
    expect(next).toBe(stripOffsetForIndex(20, 48, 4, 4) + 48 - 200 + 4)
  })

  it("음수 인덱스나 0 뷰포트는 현재 값을 유지한다", () => {
    expect(stripScrollToReveal({ ...base, index: -1, scrollOffset: 50, viewportSize: 200 })).toBe(
      50
    )
    expect(stripScrollToReveal({ ...base, index: 3, scrollOffset: 50, viewportSize: 0 })).toBe(50)
  })
})
