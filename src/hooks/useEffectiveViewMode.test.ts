import { beforeEach, describe, expect, it } from "vitest"

import { getEffectiveViewMode } from "@/hooks/useEffectiveViewMode"
import { closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

beforeEach(() => {
  closeImage()
  useAppStore.setState({
    dirImages: { images: ["/pics/a.jpg", "/pics/comic.cbz"], current_index: 1, availability: [] }
  })
})

describe("getEffectiveViewMode 폴더 아카이브", () => {
  it("양쪽 보기에서 현재 항목이 아카이브면 단일 미리보기로 바꾼다", () => {
    useSettingsStore.setState({ viewMode: "left-to-right" })
    expect(getEffectiveViewMode()).toBe("single")
  })

  it("웹툰은 연속 뷰를 유지한다", () => {
    useSettingsStore.setState({ viewMode: "webtoon" })
    expect(getEffectiveViewMode()).toBe("webtoon")
  })
})
