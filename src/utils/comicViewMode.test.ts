import { describe, expect, it } from "vitest"

import type { ComicInfo } from "@/types"
import { comicReadingDirection, resolveComicViewMode } from "@/utils/comicViewMode"

function info(manga: string | null): ComicInfo {
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
    manga,
    pages: null
  }
}

describe("comicReadingDirection", () => {
  it("reads right-to-left only from YesAndRightToLeft (case-insensitive)", () => {
    expect(comicReadingDirection(info("YesAndRightToLeft"))).toBe("right-to-left")
    expect(comicReadingDirection(info(" yesandrighttoleft "))).toBe("right-to-left")
  })

  it("treats No as left-to-right", () => {
    expect(comicReadingDirection(info("No"))).toBe("left-to-right")
  })

  it("returns null when the direction is unknown", () => {
    expect(comicReadingDirection(info("Yes"))).toBeNull()
    expect(comicReadingDirection(info("Unknown"))).toBeNull()
    expect(comicReadingDirection(info(null))).toBeNull()
    expect(comicReadingDirection(null)).toBeNull()
  })
})

describe("resolveComicViewMode", () => {
  it("returns null when the option is off", () => {
    expect(resolveComicViewMode("single", false, info("YesAndRightToLeft"))).toBeNull()
  })

  it("leaves webtoon alone", () => {
    expect(resolveComicViewMode("webtoon", true, info("YesAndRightToLeft"))).toBeNull()
  })

  it("defaults to left-to-right without metadata", () => {
    expect(resolveComicViewMode("single", true, null)).toBe("left-to-right")
  })

  it("follows ComicInfo direction over the configured one", () => {
    expect(resolveComicViewMode("left-to-right", true, info("YesAndRightToLeft"))).toBe(
      "right-to-left"
    )
    expect(resolveComicViewMode("right-to-left", true, info("No"))).toBe("left-to-right")
  })

  it("keeps the configured direction when ComicInfo has none", () => {
    expect(resolveComicViewMode("right-to-left", true, info("Yes"))).toBe("right-to-left")
    expect(resolveComicViewMode("single", true, info("Yes"))).toBe("left-to-right")
  })
})
