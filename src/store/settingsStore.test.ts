import { describe, expect, it } from "vitest"

import { DEFAULT_SHORTCUTS } from "@/constants/shortcuts"
import { sanitizeSettings } from "@/store/settingsStore"

describe("sanitizeSettings", () => {
  it("손상되지 않은 저장값은 그대로 통과한다", () => {
    const sane = sanitizeSettings({
      language: "en",
      loopNavigation: true,
      cacheMode: "extended",
      viewMode: "webtoon",
      slideshowIntervalMs: 5000,
      sortKey: "size",
      sortDescending: true
    })
    expect(sane.language).toBe("en")
    expect(sane.loopNavigation).toBe(true)
    expect(sane.cacheMode).toBe("extended")
    expect(sane.viewMode).toBe("webtoon")
    expect(sane.slideshowIntervalMs).toBe(5000)
    expect(sane.sortKey).toBe("size")
    expect(sane.sortDescending).toBe(true)
  })

  it("잘못된 union 값은 기본값으로 되돌린다", () => {
    const sane = sanitizeSettings({
      cacheMode: "turbo",
      viewMode: "grid",
      viewerBackground: "neon",
      sortKey: "random",
      language: "fr"
    })
    expect(sane.cacheMode).toBe("nearby")
    expect(sane.viewMode).toBe("single")
    expect(sane.viewerBackground).toBe("theme")
    expect(sane.sortKey).toBe("name")
    expect(sane.language).toBe("ko")
  })

  it("범위를 벗어난 슬라이드쇼 간격은 기본값으로 되돌린다", () => {
    expect(sanitizeSettings({ slideshowIntervalMs: 100 }).slideshowIntervalMs).toBe(3000)
    expect(sanitizeSettings({ slideshowIntervalMs: 60000 }).slideshowIntervalMs).toBe(3000)
    expect(sanitizeSettings({ slideshowIntervalMs: "fast" }).slideshowIntervalMs).toBe(3000)
    expect(sanitizeSettings({ slideshowIntervalMs: 2500 }).slideshowIntervalMs).toBe(2500)
  })

  it("truthy 비불리언은 false로 정규화한다", () => {
    const sane = sanitizeSettings({
      loopNavigation: 1,
      shuffle: "yes",
      skipBrokenFiles: 1
    })
    expect(sane.loopNavigation).toBe(false)
    expect(sane.shuffle).toBe(false)
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
