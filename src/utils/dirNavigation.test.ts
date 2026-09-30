import { describe, expect, it } from "vitest"

import {
  dualPageIndices,
  dualPageOffsets,
  resolveDualStepIndex,
  resolveOffsetIndex,
  resolvePairStart,
  resolveStepIndex
} from "@/utils/dirNavigation"

describe("resolveStepIndex", () => {
  it("기본 next/prev는 한 칸씩 이동한다", () => {
    expect(resolveStepIndex(1, 5, 1, false, "next")).toBe(2)
    expect(resolveStepIndex(1, 5, 1, false, "prev")).toBe(0)
  })

  it("양쪽 보기(step 2)는 두 칸씩 이동한다", () => {
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

describe("resolvePairStart", () => {
  it("표지 단독이 꺼지면 짝수 시작에 맞춘다", () => {
    expect(resolvePairStart(0, 6, false)).toBe(0)
    expect(resolvePairStart(1, 6, false)).toBe(0)
    expect(resolvePairStart(2, 6, false)).toBe(2)
    expect(resolvePairStart(5, 6, false)).toBe(4)
  })

  it("표지 단독이 켜지면 0 뒤로 홀수 시작에 맞춘다", () => {
    expect(resolvePairStart(0, 6, true)).toBe(0)
    expect(resolvePairStart(1, 6, true)).toBe(1)
    expect(resolvePairStart(2, 6, true)).toBe(1)
    expect(resolvePairStart(3, 6, true)).toBe(3)
    expect(resolvePairStart(4, 6, true)).toBe(3)
    expect(resolvePairStart(5, 6, true)).toBe(5)
  })

  it("범위를 벗어난 인덱스는 clamp한다", () => {
    expect(resolvePairStart(99, 6, true)).toBe(5)
    expect(resolvePairStart(-3, 6, false)).toBe(0)
    expect(resolvePairStart(3, 0, true)).toBe(0)
  })
})

describe("dualPageOffsets", () => {
  it("표지 단독이 꺼지면 항상 두 장을 로드한다", () => {
    expect(dualPageOffsets(0, 6, false)).toEqual([0, 1])
    expect(dualPageOffsets(4, 6, false)).toEqual([0, 1])
  })

  it("표지 단독이 켜지면 첫 장만 단독 로드한다", () => {
    expect(dualPageOffsets(0, 6, true)).toEqual([0])
    expect(dualPageOffsets(1, 6, true)).toEqual([0, 1])
    expect(dualPageOffsets(3, 6, true)).toEqual([0, 1])
  })

  it("총 장수가 없으면 빈 배열이다", () => {
    expect(dualPageOffsets(0, 0, true)).toEqual([])
  })
})

describe("dualPageIndices", () => {
  it("표지 단독이 꺼지면 현재와 다음 인덱스를 돌려준다", () => {
    expect(dualPageIndices(0, 6, false)).toEqual([0, 1])
    expect(dualPageIndices(4, 6, false)).toEqual([4, 5])
  })

  it("표지 단독이 켜진 단독 화면은 한 장만 돌려준다", () => {
    expect(dualPageIndices(0, 6, true)).toEqual([0])
    expect(dualPageIndices(2, 6, true)).toEqual([2, 3])
  })

  it("마지막 장에서 다음 장이 없으면 한 장만 남긴다", () => {
    expect(dualPageIndices(5, 6, false)).toEqual([5])
    expect(dualPageIndices(4, 5, false)).toEqual([4])
  })

  it("루프면 목록 밖 오프셋을 wrap하고 중복은 한 번만 넣는다", () => {
    expect(dualPageIndices(5, 6, false, 0, true)).toEqual([5, 0])
    // 1장짜리 목록에서 wrap하면 같은 인덱스만 남는다.
    expect(dualPageIndices(0, 1, false, 0, true)).toEqual([0])
  })

  it("범위를 벗어난 현재 인덱스는 clamp한다", () => {
    expect(dualPageIndices(99, 6, false)).toEqual([5])
    expect(dualPageIndices(-3, 6, false)).toEqual([0, 1])
    expect(dualPageIndices(0, 0, false)).toEqual([])
  })

  it("표지가 0번이 아니면 표지 화면과 표지 앞 페이지는 단독이다", () => {
    const TOTAL = 8
    const COVER = 3
    expect(dualPageIndices(COVER, TOTAL, true, COVER)).toEqual([COVER])
    expect(dualPageIndices(2, TOTAL, true, COVER)).toEqual([2])
    expect(dualPageIndices(4, TOTAL, true, COVER)).toEqual([4, 5])
  })
})

describe("resolveDualStepIndex", () => {
  it("표지 단독이 꺼지면 step 2와 같은 결과를 낸다", () => {
    for (const total of [2, 3, 5, 6, 7]) {
      for (const loop of [false, true]) {
        for (const direction of ["prev", "next"] as const) {
          for (let current = 0; current < total; current += 1) {
            expect(resolveDualStepIndex(current, total, loop, direction, false)).toBe(
              resolveStepIndex(current, total, 2, loop, direction)
            )
          }
        }
      }
    }
  })

  it("표지 단독이 켜지면 표지에서 다음은 2페이지다", () => {
    expect(resolveDualStepIndex(0, 6, false, "next", true)).toBe(1)
    expect(resolveDualStepIndex(1, 6, false, "next", true)).toBe(3)
    expect(resolveDualStepIndex(3, 6, false, "next", true)).toBe(5)
  })

  it("표지 단독이 켜지면 2페이지에서 이전은 표지다", () => {
    expect(resolveDualStepIndex(1, 6, false, "prev", true)).toBe(0)
    expect(resolveDualStepIndex(3, 6, false, "prev", true)).toBe(1)
    expect(resolveDualStepIndex(5, 6, false, "prev", true)).toBe(3)
  })

  it("쌍이 페이지를 모두 덮으면 끝에서 멈춘다", () => {
    // 5장: [0], [1,2], [3,4]라 4는 3과 이미 함께 본 상태다.
    expect(resolveDualStepIndex(3, 5, false, "next", true)).toBeNull()
    // 6장: [0], [1,2], [3,4], [5]라 마지막 한 장은 단독으로 본다.
    expect(resolveDualStepIndex(3, 6, false, "next", true)).toBe(5)
    expect(resolveDualStepIndex(5, 6, false, "next", true)).toBeNull()
  })

  it("표지 단독 모드의 루프는 마지막 화면과 표지를 잇는다", () => {
    expect(resolveDualStepIndex(3, 5, true, "next", true)).toBe(0)
    expect(resolveDualStepIndex(0, 5, true, "prev", true)).toBe(3)
    expect(resolveDualStepIndex(5, 6, true, "next", true)).toBe(0)
    expect(resolveDualStepIndex(0, 6, true, "prev", true)).toBe(5)
    expect(resolveDualStepIndex(0, 6, true, "next", true)).toBe(1)
  })

  it("장수가 1장 이하면 표지 단독 모드에서도 이동하지 않는다", () => {
    expect(resolveDualStepIndex(0, 1, true, "next", true)).toBeNull()
    expect(resolveDualStepIndex(0, 0, false, "prev", true)).toBeNull()
  })

  it("쌍 중간 인덱스에서도 현재 화면 기준으로 이동한다", () => {
    expect(resolveDualStepIndex(2, 6, false, "next", true)).toBe(3)
    expect(resolveDualStepIndex(2, 6, false, "prev", true)).toBe(0)
  })
})

describe("dirNavigation ComicInfo 표지 인덱스", () => {
  // coverIndex 3, total 8 → 화면: [0,1], [2], [3](표지), [4,5], [6,7]
  const TOTAL = 8
  const COVER = 3

  it("표지가 0번이 아니면 표지 뒤부터 쌍을 맞춘다", () => {
    expect(resolvePairStart(0, TOTAL, true, COVER)).toBe(0)
    expect(resolvePairStart(1, TOTAL, true, COVER)).toBe(0)
    expect(resolvePairStart(2, TOTAL, true, COVER)).toBe(2)
    expect(resolvePairStart(3, TOTAL, true, COVER)).toBe(3)
    expect(resolvePairStart(4, TOTAL, true, COVER)).toBe(4)
    expect(resolvePairStart(5, TOTAL, true, COVER)).toBe(4)
    expect(resolvePairStart(6, TOTAL, true, COVER)).toBe(6)
  })

  it("표지와 앞에 남는 페이지는 단독 화면이다", () => {
    expect(dualPageOffsets(COVER, TOTAL, true, COVER)).toEqual([0])
    expect(dualPageOffsets(2, TOTAL, true, COVER)).toEqual([0])
    expect(dualPageOffsets(0, TOTAL, true, COVER)).toEqual([0, 1])
    expect(dualPageOffsets(4, TOTAL, true, COVER)).toEqual([0, 1])
  })

  it("다음/이전이 표지 기준 화면 순서를 따른다", () => {
    expect(resolveDualStepIndex(0, TOTAL, false, "next", true, COVER)).toBe(2)
    expect(resolveDualStepIndex(2, TOTAL, false, "next", true, COVER)).toBe(3)
    expect(resolveDualStepIndex(3, TOTAL, false, "next", true, COVER)).toBe(4)
    expect(resolveDualStepIndex(4, TOTAL, false, "next", true, COVER)).toBe(6)
    expect(resolveDualStepIndex(6, TOTAL, false, "next", true, COVER)).toBeNull()
    expect(resolveDualStepIndex(3, TOTAL, false, "prev", true, COVER)).toBe(2)
    expect(resolveDualStepIndex(0, TOTAL, false, "prev", true, COVER)).toBeNull()
  })

  it("표지가 마지막 페이지여도 목록 안에서 동작한다", () => {
    expect(resolvePairStart(0, 5, true, 4)).toBe(0)
    expect(dualPageOffsets(4, 5, true, 4)).toEqual([0])
    expect(resolveDualStepIndex(0, 5, false, "next", true, 4)).toBe(2)
    expect(resolveDualStepIndex(2, 5, false, "next", true, 4)).toBe(4)
  })

  it("표지 단독이 꺼지면 표지 인덱스를 무시한다", () => {
    expect(resolvePairStart(3, TOTAL, false, COVER)).toBe(2)
    expect(dualPageOffsets(3, TOTAL, false, COVER)).toEqual([0, 1])
    expect(resolveDualStepIndex(0, TOTAL, false, "next", false, COVER)).toBe(2)
  })
})
