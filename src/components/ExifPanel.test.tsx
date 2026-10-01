import { cleanup, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "ko" } })
}))

import { ExifPanel } from "@/components/ExifPanel"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo } from "@/types"

beforeEach(() => {
  cleanup()
  closeImage()
})

const svgInfo = {
  file_path: "/pics/vector.svg",
  source_path: "/pics/vector.svg",
  file_name: "vector.svg",
  file_size: 456,
  mime_type: "image/svg+xml",
  width: 800,
  height: 600
}

const svgDetails = {
  file_path: "/pics/vector.svg",
  file_size: 456,
  width: 800,
  height: 600,
  color_mode: "unknown",
  bits_per_channel: null,
  created_unix: null,
  modified_unix: null,
  dpi_x: null,
  dpi_y: null,
  icc_status: "unchecked" as const,
  icc_name: null,
  icc_bytes: null
}

describe("ExifPanel SVG", () => {
  it("벡터 안내 문구를 표시한다", () => {
    useAppStore.setState({
      imageInfo: svgInfo,
      imageDetails: svgDetails,
      histogramData: null,
      exifData: null,
      exifError: null,
      showExifPanel: true
    })

    render(<ExifPanel />)

    expect(screen.getByText("histogram.vectorUnavailable")).not.toBeNull()
    expect(screen.getByText("details.colorVector")).not.toBeNull()
  })

  it("래스터는 기존 문구를 유지한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        source_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 2000,
        height: 1000
      },
      imageDetails: { ...svgDetails, file_path: "/pics/a.jpg" },
      histogramData: null,
      exifData: null,
      exifError: null,
      showExifPanel: true
    })

    render(<ExifPanel />)

    expect(screen.getByText("histogram.unavailable")).not.toBeNull()
  })
})

const comicInfo: ComicInfo = {
  title: "첫 화",
  series: "테스트 시리즈",
  number: "2",
  count: 12,
  volume: 2024,
  summary: "요약 첫 줄\n둘째 줄",
  writer: "글 작가",
  penciller: null,
  inker: null,
  colorist: null,
  letterer: null,
  cover_artist: null,
  editor: null,
  year: 2024,
  month: 3,
  day: 5,
  publisher: "테스트 출판사",
  genre: null,
  tags: null,
  language_iso: "ko",
  page_count: 20,
  age_rating: null,
  community_rating: "8.5",
  manga: "YesAndRightToLeft",
  pages: [{ image: 0, page_type: "FrontCover" }]
}

function openArchivePanel(
  options: { comicInfo?: ComicInfo; comicInfoError?: string; showComicInfo?: boolean } = {}
) {
  useSettingsStore.setState({ showComicInfo: options.showComicInfo ?? true })
  useAppStore.setState({
    imageInfo: { ...svgInfo, file_path: "/docs/comic.cbz" },
    imageDetails: { ...svgDetails, file_path: "/docs/comic.cbz" },
    histogramData: null,
    exifData: null,
    exifError: null,
    archivePath: "/docs/comic.cbz",
    comicInfo: options.comicInfo ?? null,
    comicInfoError: options.comicInfoError ?? null,
    showExifPanel: true
  })
}

describe("ExifPanel Comic 섹션", () => {
  it("메타데이터가 있으면 시리즈와 필드를 표시한다", () => {
    openArchivePanel({ comicInfo })
    render(<ExifPanel />)

    expect(screen.getByText("comic.section")).not.toBeNull()
    expect(screen.getByText("테스트 시리즈 #2")).not.toBeNull()
    expect(screen.getByText("첫 화")).not.toBeNull()
    expect(screen.getByText("comic.writer")).not.toBeNull()
    expect(screen.getByText("글 작가")).not.toBeNull()
    expect(screen.getByText("2024-03-05")).not.toBeNull()
    expect(screen.getByText("comic.directionRtl")).not.toBeNull()
    // 없는 필드는 행 자체를 만들지 않는다.
    expect(screen.queryByText("comic.penciller")).toBeNull()

    const summary = screen.getByText(/요약 첫 줄/)
    expect(summary.className).toContain("whitespace-pre-wrap")
    // 줄바꿈을 그대로 보존한다.
    expect(summary.textContent).toBe("요약 첫 줄\n둘째 줄")
  })

  it("파싱 실패는 에러 문구로 표시한다", () => {
    openArchivePanel({ comicInfoError: "Failed to parse ComicInfo.xml" })
    render(<ExifPanel />)

    expect(screen.getByText("comic.loadFail")).not.toBeNull()
    expect(screen.getByText("Failed to parse ComicInfo.xml")).not.toBeNull()
  })

  it("만화 정보 표시를 끄면 메타데이터가 있어도 섹션을 숨긴다", () => {
    openArchivePanel({ comicInfo, showComicInfo: false })
    render(<ExifPanel />)

    expect(screen.queryByText("comic.section")).toBeNull()
    expect(screen.queryByText("테스트 시리즈 #2")).toBeNull()
    // 파일/히스토그램 섹션은 그대로 남는다.
    expect(screen.getByText("details.title")).not.toBeNull()
  })

  it("만화 정보 표시를 끄면 파싱 오류도 숨긴다", () => {
    openArchivePanel({ comicInfoError: "Failed to parse ComicInfo.xml", showComicInfo: false })
    render(<ExifPanel />)

    expect(screen.queryByText("comic.loadFail")).toBeNull()
    expect(screen.queryByText("Failed to parse ComicInfo.xml")).toBeNull()
  })

  it("아카이브가 아니거나 메타데이터가 없으면 섹션을 숨긴다", () => {
    openArchivePanel()
    const first = render(<ExifPanel />)
    expect(first.queryByText("comic.section")).toBeNull()
    first.unmount()

    // 일반 이미지에 comicInfo가 남아 있어도(이전 아카이브 잔여) 숨긴다.
    useAppStore.setState({
      archivePath: null,
      imageInfo: svgInfo,
      imageDetails: svgDetails,
      comicInfo,
      showExifPanel: true
    })
    render(<ExifPanel />)
    expect(screen.queryByText("comic.section")).toBeNull()
  })
})
