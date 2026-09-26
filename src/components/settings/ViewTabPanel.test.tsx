import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {}
    })
  }
}))

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn(), info: vi.fn() }
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

// 설정 스토어가 실제 i18n 체인(i18next + 로케일 JSON)을 끌어오면 이 파일의 import가
// 수 초씩 걸린다. 다른 설정 테스트와 동일하게 가볍게 대체한다.
vi.mock("@/i18n", () => ({
  default: { t: (key: string) => key, language: "ko" },
  detectSystemLanguage: () => "ko",
  normalizeLanguage: (value: unknown) => (value === "ko" || value === "en" ? value : null),
  setI18nLanguage: async () => {}
}))

import { ViewTabPanel } from "@/components/settings/ViewTabPanel"
import { DEFAULT_SETTINGS, useSettingsStore } from "@/store/settingsStore"

beforeEach(() => {
  cleanup()
  useSettingsStore.setState({
    ...DEFAULT_SETTINGS,
    webtoonImageGap: 12,
    webtoonPageBoundaries: false,
    webtoonFitWidth: false,
    webtoonShowProgress: true,
    webtoonThumbnailJump: true
  })
})

describe("ViewTabPanel webtoon settings", () => {
  it("웹툰 간격 슬라이더는 하나의 이름 있는 슬라이더로 렌더링한다", () => {
    render(<ViewTabPanel />)

    const sliders = Array.from(document.querySelectorAll<HTMLInputElement>('input[type="range"]'))
    expect(sliders).toHaveLength(1)
    expect(sliders[0].getAttribute("aria-labelledby")).toBe("settings-webtoon-gap-label")
    expect(document.getElementById("settings-webtoon-gap-label")?.textContent).toContain(
      "settings.webtoon.imageGap"
    )
  })

  it("이미지 표시 모드와 픽셀 아트 자동 감지 스위치를 설정에 반영한다", async () => {
    render(<ViewTabPanel />)

    fireEvent.click(screen.getByRole("radio", { name: "settings.imageRendering.pixelated" }))
    await waitFor(() => {
      expect(useSettingsStore.getState().imageScalingMode).toBe("pixelated")
    })

    fireEvent.click(screen.getByRole("radio", { name: "settings.imageRendering.auto" }))
    fireEvent.click(screen.getByRole("switch", { name: "settings.imageRendering.autoDetect" }))
    await waitFor(() => {
      expect(useSettingsStore.getState().imageScalingMode).toBe("auto")
      expect(useSettingsStore.getState().autoDetectPixelArt).toBe(false)
    })
  })

  it("페이지 경계와 Fit width 스위치를 설정에 반영한다", async () => {
    render(<ViewTabPanel />)

    fireEvent.click(screen.getByRole("switch", { name: "settings.webtoon.pageBoundaries" }))
    fireEvent.click(screen.getByRole("switch", { name: "settings.webtoon.fitWidth" }))

    await waitFor(() => {
      expect(useSettingsStore.getState().webtoonPageBoundaries).toBe(true)
      expect(useSettingsStore.getState().webtoonFitWidth).toBe(true)
    })
  })
})
