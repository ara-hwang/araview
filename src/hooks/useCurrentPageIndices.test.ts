import { renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it } from "vitest"

import { useCurrentPageIndices } from "@/hooks/useCurrentPageIndices"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore, type ViewMode } from "@/store/settingsStore"
import type { ComicInfo } from "@/types"

const IMAGES = ["/pics/1.jpg", "/pics/2.jpg", "/pics/3.jpg", "/pics/4.jpg", "/pics/5.jpg"]

function setup(
  index: number,
  viewMode: ViewMode,
  options?: { coverAlone?: boolean; loop?: boolean; comicInfo?: ComicInfo | null }
) {
  useAppStore.setState({
    dirImages: { images: IMAGES, current_index: index, availability: [] },
    comicInfo: options?.comicInfo ?? null
  })
  useSettingsStore.setState({
    viewMode,
    showCoverAlone: options?.coverAlone ?? true,
    loopNavigation: options?.loop ?? false
  })
}

const current = () => [...renderHook(() => useCurrentPageIndices()).result.current].sort()

beforeEach(() => {
  useSettingsStore.setState({ viewMode: "single", showCoverAlone: true, loopNavigation: false })
  useAppStore.setState({ comicInfo: null })
})

describe("useCurrentPageIndices", () => {
  it("단일 보기는 현재 장 하나만 담는다", () => {
    setup(2, "single")
    expect(current()).toEqual([2])
  })

  it("웹툰 보기는 중앙 장 하나만 담는다", () => {
    setup(2, "webtoon")
    expect(current()).toEqual([2])
  })

  it("양쪽 보기는 쌍 두 장을 담는다", () => {
    setup(2, "left-to-right", { coverAlone: false })
    expect(current()).toEqual([2, 3])
  })

  it("오른쪽부터 두 페이지도 같은 쌍을 담는다", () => {
    setup(2, "right-to-left", { coverAlone: false })
    expect(current()).toEqual([2, 3])
  })

  it("표지 단독 화면은 표지만 담는다", () => {
    setup(0, "left-to-right", { coverAlone: true })
    expect(current()).toEqual([0])
  })

  it("표지 단독에서 다음 화면부터는 두 장을 담는다", () => {
    setup(1, "left-to-right", { coverAlone: true })
    expect(current()).toEqual([1, 2])
  })

  it("마지막 홀수 장은 혼자 보므로 한 장만 담는다", () => {
    setup(4, "left-to-right", { coverAlone: false })
    expect(current()).toEqual([4])
  })

  it("루프면 화면 밖 다음 장을 첫 장으로 wrap한다", () => {
    setup(4, "left-to-right", { coverAlone: false, loop: true })
    expect(current()).toEqual([0, 4])
  })

  it("ComicInfo 표지가 0번이 아니면 표지 화면을 단독으로 본다", () => {
    const comicInfo = {
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
      pages: [{ image: 2, page_type: "FrontCover" }]
    } as ComicInfo
    setup(2, "left-to-right", { coverAlone: true, comicInfo })
    expect(current()).toEqual([2])
  })
})
