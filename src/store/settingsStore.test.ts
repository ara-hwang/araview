import { describe, expect, it } from "vitest"

import { DEFAULT_SHORTCUTS } from "@/constants/shortcuts"
import { sanitizeSettings } from "@/store/settingsStore"

describe("sanitizeSettings", () => {
  it("손상되지 않은 저장값은 그대로 통과한다", () => {
    const sane = sanitizeSettings({
      language: "en",
      loopNavigation: true,
      cacheMode: "extended",
      cacheStorageMode: "temporary",
      viewMode: "webtoon",
      webtoonImageGap: 16,
      webtoonPageBoundaries: true,
      webtoonFitWidth: true,
      webtoonShowProgress: false,
      webtoonThumbnailJump: false,
      sortKey: "size",
      sortDescending: true,
      fitMode: "width",
      dockPosition: "left",
      dockVisible: false,
      dockThumbSize: "l",
      dockShowName: true,
      dockShowIndex: true
    })
    expect(sane.language).toBe("en")
    expect(sane.loopNavigation).toBe(true)
    expect(sane.cacheMode).toBe("extended")
    expect(sane.cacheStorageMode).toBe("temporary")
    expect(sane.viewMode).toBe("webtoon")
    expect(sane.webtoonImageGap).toBe(16)
    expect(sane.webtoonPageBoundaries).toBe(true)
    expect(sane.webtoonFitWidth).toBe(true)
    expect(sane.webtoonShowProgress).toBe(false)
    expect(sane.webtoonThumbnailJump).toBe(false)
    expect(sane.sortKey).toBe("size")
    expect(sane.sortDescending).toBe(true)
    expect(sane.fitMode).toBe("width")
    expect(sane.dockPosition).toBe("left")
    expect(sane.dockVisible).toBe(false)
    expect(sane.dockThumbSize).toBe("l")
    expect(sane.dockShowName).toBe(true)
    expect(sane.dockShowIndex).toBe(true)
  })

  it("도크 기본값은 아래쪽 펼침이다", () => {
    const sane = sanitizeSettings({})
    expect(sane.dockPosition).toBe("bottom")
    expect(sane.dockVisible).toBe(true)
    expect(sane.dockThumbSize).toBe("s")
    expect(sane.dockShowName).toBe(false)
    expect(sane.dockShowIndex).toBe(false)
  })

  it("웹툰 세부 설정의 기본값과 범위를 복원한다", () => {
    const defaults = sanitizeSettings({})
    expect(defaults.webtoonImageGap).toBe(8)
    expect(defaults.webtoonPageBoundaries).toBe(false)
    expect(defaults.webtoonFitWidth).toBe(false)
    expect(defaults.webtoonShowProgress).toBe(true)
    expect(defaults.webtoonThumbnailJump).toBe(true)

    expect(sanitizeSettings({ webtoonImageGap: -10 }).webtoonImageGap).toBe(0)
    expect(sanitizeSettings({ webtoonImageGap: 100 }).webtoonImageGap).toBe(64)
    expect(sanitizeSettings({ webtoonImageGap: "wide" }).webtoonImageGap).toBe(8)
  })

  it("이어보기는 기본 켜짐이고 저장된 값을 따른다", () => {
    expect(sanitizeSettings({}).resumeReading).toBe(true)
    expect(sanitizeSettings({ resumeReading: false }).resumeReading).toBe(false)
    expect(sanitizeSettings({ resumeReading: "no" }).resumeReading).toBe(false)
  })

  it("표지 단독 표시는 기본 켜짐이고 저장된 값을 따른다", () => {
    expect(sanitizeSettings({}).showCoverAlone).toBe(true)
    expect(sanitizeSettings({ showCoverAlone: false }).showCoverAlone).toBe(false)
    expect(sanitizeSettings({ showCoverAlone: "no" }).showCoverAlone).toBe(false)
  })

  it("만화 정보 표시는 기본 켜짐이고 저장된 값을 따른다", () => {
    expect(sanitizeSettings({}).showComicInfo).toBe(true)
    expect(sanitizeSettings({ showComicInfo: false }).showComicInfo).toBe(false)
    expect(sanitizeSettings({ showComicInfo: "yes" }).showComicInfo).toBe(false)
  })

  it("영구 캐시는 기본 켜짐이고 저장된 값을 따른다", () => {
    expect(sanitizeSettings({}).cacheStorageMode).toBe("persistent")
    expect(sanitizeSettings({ cacheStorageMode: "temporary" }).cacheStorageMode).toBe("temporary")
    expect(sanitizeSettings({ cacheStorageMode: "forever" }).cacheStorageMode).toBe("persistent")
  })

  it("표시 해상도 상한은 기본 원본이고 저장된 값을 따른다", () => {
    expect(sanitizeSettings({}).maxResolution).toBe("original")
    expect(sanitizeSettings({ maxResolution: "4k" }).maxResolution).toBe("4k")
    expect(sanitizeSettings({ maxResolution: "1080p" }).maxResolution).toBe("1080p")
    expect(sanitizeSettings({ maxResolution: "8k" }).maxResolution).toBe("original")
  })

  it("이미지 표시 방식과 픽셀 아트 자동 감지를 복원한다", () => {
    expect(sanitizeSettings({}).imageScalingMode).toBe("auto")
    expect(sanitizeSettings({}).autoDetectPixelArt).toBe(true)
    expect(sanitizeSettings({ imageScalingMode: "pixelated" }).imageScalingMode).toBe("pixelated")
    expect(sanitizeSettings({ imageScalingMode: "cubic" }).imageScalingMode).toBe("auto")
    expect(sanitizeSettings({ autoDetectPixelArt: false }).autoDetectPixelArt).toBe(false)
    expect(sanitizeSettings({ autoDetectPixelArt: "yes" }).autoDetectPixelArt).toBe(false)
  })

  it("잘못된 union 값은 기본값으로 되돌린다", () => {
    const sane = sanitizeSettings({
      cacheMode: "turbo",
      maxResolution: "8k",
      viewMode: "grid",
      viewerBackground: "neon",
      sortKey: "random",
      language: "fr",
      fitMode: "cover",
      dockPosition: "center",
      dockThumbSize: "xl"
    })
    expect(sane.cacheMode).toBe("nearby")
    expect(sane.maxResolution).toBe("original")
    expect(sane.viewMode).toBe("single")
    expect(sane.viewerBackground).toBe("theme")
    expect(sane.sortKey).toBe("name")
    expect(sane.language).toBe("ko")
    expect(sane.fitMode).toBe("auto")
    expect(sane.dockPosition).toBe("bottom")
    expect(sane.dockThumbSize).toBe("s")
  })

  it("truthy 비불리언은 false로 정규화한다", () => {
    const sane = sanitizeSettings({
      loopNavigation: 1,
      skipBrokenFiles: 1
    })
    expect(sane.loopNavigation).toBe(false)
    expect(sane.skipBrokenFiles).toBe(false)
  })

  it("null이나 원시값 입력에도 기본값을 반환한다", () => {
    for (const bad of [null, undefined, 42, "oops", []]) {
      const sane = sanitizeSettings(bad)
      expect(sane.viewMode).toBe("single")
      expect(sane.shortcuts).toEqual(DEFAULT_SHORTCUTS)
    }
  })
})
