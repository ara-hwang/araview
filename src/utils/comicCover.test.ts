import { describe, expect, it } from "vitest"

import type { ComicInfo, ComicPage } from "@/types"
import { resolveCoverIndex } from "@/utils/comicCover"

function comic(pages: ComicPage[] | null): ComicInfo {
  return {
    title: null,
    series: null,
    number: null,
    count: null,
    volume: null,
    summary: null,
    writer: null,
    penciller: null,
    publisher: null,
    genre: null,
    tags: null,
    language_iso: null,
    page_count: null,
    age_rating: null,
    community_rating: null,
    manga: null,
    pages
  }
}

describe("resolveCoverIndex", () => {
  it("FrontCover 페이지 인덱스를 쓴다", () => {
    expect(
      resolveCoverIndex(
        comic([
          { image: 0, page_type: "Story" },
          { image: 1, page_type: "FrontCover" }
        ]),
        5
      )
    ).toBe(1)
  })

  it("Type 표기는 대소문자와 공백을 무시한다", () => {
    expect(resolveCoverIndex(comic([{ image: 2, page_type: " frontcover " }]), 5)).toBe(2)
    expect(resolveCoverIndex(comic([{ image: 2, page_type: "FRONTCOVER" }]), 5)).toBe(2)
  })

  it("여러 개면 첫 FrontCover를 쓴다", () => {
    expect(
      resolveCoverIndex(
        comic([
          { image: 3, page_type: "FrontCover" },
          { image: 1, page_type: "FrontCover" }
        ]),
        5
      )
    ).toBe(3)
  })

  it("FrontCover가 없으면 0번으로 폴백한다", () => {
    expect(resolveCoverIndex(comic([{ image: 2, page_type: "Story" }]), 5)).toBe(0)
    expect(resolveCoverIndex(comic([{ image: 0, page_type: null }]), 5)).toBe(0)
  })

  it("메타데이터가 없으면 0번이다", () => {
    expect(resolveCoverIndex(null, 5)).toBe(0)
    expect(resolveCoverIndex(comic(null), 5)).toBe(0)
    expect(resolveCoverIndex(comic([]), 5)).toBe(0)
  })

  it("목록 범위를 벗어나거나 음수면 0번으로 폴백한다", () => {
    // 1 기반으로 적은 파일(마지막 페이지가 범위를 벗어남).
    expect(resolveCoverIndex(comic([{ image: 5, page_type: "FrontCover" }]), 5)).toBe(0)
    expect(resolveCoverIndex(comic([{ image: -1, page_type: "FrontCover" }]), 5)).toBe(0)
    // 소수점으로 적은 값은 정수로 내림한다.
    expect(resolveCoverIndex(comic([{ image: 1.9, page_type: "FrontCover" }]), 5)).toBe(1)
  })

  it("총 장수가 없으면 0번이다", () => {
    expect(resolveCoverIndex(comic([{ image: 1, page_type: "FrontCover" }]), 0)).toBe(0)
  })
})
