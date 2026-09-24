import { describe, expect, it } from "vitest"

import { calculateWebtoonScrollMetrics } from "@/utils/webtoonProgress"

describe("calculateWebtoonScrollMetrics", () => {
  const pages = [
    { index: 0, top: 0, height: 800 },
    { index: 1, top: 808, height: 1200 },
    { index: 2, top: 2016, height: 800 }
  ]

  it("가시 영역 중앙에 가장 가까운 페이지를 현재 위치로 고른다", () => {
    const metrics = calculateWebtoonScrollMetrics({
      pages,
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: 700,
      scrollHeight: 3600,
      fallbackIndex: 0
    })

    expect(metrics.currentIndex).toBe(0)
    expect(metrics.percent).toBe(25)
  })

  it("긴 페이지를 지나면 중앙에 보이는 다음 페이지를 고른다", () => {
    const metrics = calculateWebtoonScrollMetrics({
      pages: [
        { index: 0, top: -1500, height: 800 },
        { index: 1, top: -692, height: 1200 },
        { index: 2, top: 516, height: 800 }
      ],
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: 1500,
      scrollHeight: 3600,
      fallbackIndex: 0
    })

    expect(metrics.currentIndex).toBe(1)
    expect(metrics.percent).toBe(54)
  })

  it("스크롤 값을 0~100 범위로 제한한다", () => {
    const before = calculateWebtoonScrollMetrics({
      pages,
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: -200,
      scrollHeight: 2816,
      fallbackIndex: 0
    })
    const after = calculateWebtoonScrollMetrics({
      pages: [
        { index: 0, top: -2016, height: 800 },
        { index: 1, top: -1208, height: 1200 },
        { index: 2, top: 0, height: 800 }
      ],
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: 9999,
      scrollHeight: 2816,
      fallbackIndex: 0
    })

    expect(before.percent).toBe(0)
    expect(after.percent).toBe(100)
    expect(after.currentIndex).toBe(2)
  })

  it("스크롤할 구간이 없으면 전체를 읽을 수 있는 100%로 본다", () => {
    const metrics = calculateWebtoonScrollMetrics({
      pages: [{ index: 3, top: 0, height: 400 }],
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: 0,
      scrollHeight: 400,
      fallbackIndex: 3
    })

    expect(metrics).toEqual({ currentIndex: 3, percent: 100 })
  })

  it("페이지 측정값이 없으면 현재 인덱스 fallback을 유지한다", () => {
    const metrics = calculateWebtoonScrollMetrics({
      pages: [],
      viewportTop: 0,
      viewportHeight: 800,
      scrollTop: 0,
      scrollHeight: 1600,
      fallbackIndex: 4
    })

    expect(metrics).toEqual({ currentIndex: 4, percent: 0 })
  })

  it("레이아웃 높이가 0이면 현재 인덱스를 덮어쓰지 않는다", () => {
    const metrics = calculateWebtoonScrollMetrics({
      pages: [{ index: 0, top: 0, height: 0 }],
      viewportTop: 0,
      viewportHeight: 0,
      scrollTop: 0,
      scrollHeight: 800,
      fallbackIndex: 4
    })

    expect(metrics.currentIndex).toBe(4)
  })
})
