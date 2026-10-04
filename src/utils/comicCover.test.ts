import { describe, expect, it } from "vitest"

import type { ComicInfo, ComicPage } from "@/types"
import {
  coverPagesOf,
  explicitCoverPages,
  resolveCoverLayout,
  toggleCoverPage,
  withCoverSolo
} from "@/utils/comicCover"

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
    inker: null,
    colorist: null,
    letterer: null,
    cover_artist: null,
    editor: null,
    year: null,
    month: null,
    day: null,
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

const cover = (image: number): ComicPage => ({ image, page_type: "FrontCover" })

describe("explicitCoverPages", () => {
  it("FrontCover 페이지 인덱스를 쓴다", () => {
    expect(explicitCoverPages(comic([{ image: 0, page_type: "Story" }, cover(1)]), 5)).toEqual([1])
  })

  it("Type 비교는 대소문자와 공백을 무시한다", () => {
    expect(explicitCoverPages(comic([{ image: 2, page_type: " frontcover " }]), 5)).toEqual([2])
    expect(explicitCoverPages(comic([{ image: 2, page_type: "FRONTCOVER" }]), 5)).toEqual([2])
  })

  it("연속된 표지는 모두 담고, 떨어진 표지는 버린다", () => {
    expect(explicitCoverPages(comic([cover(0), cover(1), cover(2)]), 9)).toEqual([0, 1, 2])
    expect(explicitCoverPages(comic([cover(3), cover(1), cover(2), cover(6)]), 9)).toEqual([
      1, 2, 3
    ])
  })

  it("FrontCover가 없거나 메타데이터가 없으면 비어 있다", () => {
    expect(explicitCoverPages(comic([{ image: 2, page_type: "Story" }]), 5)).toEqual([])
    expect(explicitCoverPages(null, 5)).toEqual([])
    expect(explicitCoverPages(comic(null), 5)).toEqual([])
  })

  it("범위를 벗어난 인덱스는 버리고 소수는 내림한다", () => {
    expect(explicitCoverPages(comic([cover(5)]), 5)).toEqual([])
    expect(explicitCoverPages(comic([cover(-1)]), 5)).toEqual([])
    expect(explicitCoverPages(comic([cover(1.9)]), 5)).toEqual([1])
    expect(explicitCoverPages(comic([cover(1)]), 0)).toEqual([])
  })
})

describe("resolveCoverLayout", () => {
  it("지정이 없으면 전역 설정대로 0번을 표지로 본다", () => {
    for (const info of [null, comic(null), comic([{ image: 0, page_type: null }])]) {
      expect(resolveCoverLayout(info, 5, true)).toEqual({
        coverAlone: true,
        coverIndex: 0,
        coverCount: 1
      })
      expect(resolveCoverLayout(info, 5, false).coverAlone).toBe(false)
    }
  })

  it("FrontCover가 명시되면 전역 설정이 꺼져 있어도 단독으로 본다", () => {
    expect(resolveCoverLayout(comic([cover(2), cover(3)]), 9, false)).toEqual({
      coverAlone: true,
      coverIndex: 2,
      coverCount: 2
    })
  })

  it("0번이 표지가 아닌 Type으로 명시되면 전역 설정이 켜져 있어도 쌍으로 본다", () => {
    expect(resolveCoverLayout(comic([{ image: 0, page_type: "Story" }]), 5, true)).toEqual({
      coverAlone: false,
      coverIndex: 0,
      coverCount: 1
    })
  })

  it("FrontCover가 범위 밖이면 전역 설정을 따른다", () => {
    const info = comic([{ image: 0, page_type: "Story" }, cover(9)])
    expect(resolveCoverLayout(info, 5, true).coverAlone).toBe(true)
    expect(resolveCoverLayout(info, 5, false).coverAlone).toBe(false)
  })
})

describe("coverPagesOf / withCoverSolo", () => {
  it("단독 표지 인덱스를 나열한다", () => {
    expect(coverPagesOf({ coverAlone: true, coverIndex: 2, coverCount: 3 })).toEqual([2, 3, 4])
    expect(coverPagesOf({ coverAlone: false, coverIndex: 0, coverCount: 1 })).toEqual([])
  })

  it("표지가 여러 장일 때만 단독 인덱스에 더한다", () => {
    const base = new Set([7])
    expect(withCoverSolo({ coverAlone: true, coverIndex: 0, coverCount: 1 }, base)).toBe(base)
    expect([...withCoverSolo({ coverAlone: true, coverIndex: 0, coverCount: 2 }, base)]).toEqual([
      7, 0, 1
    ])
  })
})

describe("toggleCoverPage", () => {
  it("붙어 있는 페이지를 지정하면 구간이 늘어난다", () => {
    expect(toggleCoverPage([0], 1)).toEqual([0, 1])
    expect(toggleCoverPage([2, 3], 1)).toEqual([1, 2, 3])
  })

  it("떨어진 페이지를 지정하면 그 페이지만 표지가 된다", () => {
    expect(toggleCoverPage([0, 1], 5)).toEqual([5])
    expect(toggleCoverPage([], 3)).toEqual([3])
  })

  it("해제하면 구간에서 빠지고, 중간을 해제하면 앞쪽만 남는다", () => {
    expect(toggleCoverPage([0, 1, 2], 2)).toEqual([0, 1])
    expect(toggleCoverPage([0, 1, 2], 0)).toEqual([1, 2])
    expect(toggleCoverPage([0, 1, 2], 1)).toEqual([0])
    expect(toggleCoverPage([0], 0)).toEqual([])
  })
})
