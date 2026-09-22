import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const mockConfirm = vi.fn()
const mockSetTheme = vi.fn()

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: (...args: unknown[]) => mockConfirm(...args)
}))

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

vi.mock("@/components/theme-provider", () => ({
  useTheme: () => ({ theme: "system", setTheme: mockSetTheme })
}))

vi.mock("@/components/settings/GeneralTabPanel", () => ({
  GeneralTabPanel: () => <div data-testid="panel-general" />
}))
vi.mock("@/components/settings/ViewTabPanel", () => ({
  ViewTabPanel: () => <div data-testid="panel-view" />
}))
vi.mock("@/components/settings/ListTabPanel", () => ({
  ListTabPanel: () => <div data-testid="panel-list" />
}))
vi.mock("@/components/settings/PerformanceTabPanel", () => ({
  PerformanceTabPanel: () => <div data-testid="panel-performance" />
}))
vi.mock("@/components/settings/ShortcutsTabPanel", () => ({
  ShortcutsTabPanel: () => <div data-testid="panel-shortcuts" />
}))
vi.mock("@/components/settings/ExtensionSettingsPanel", () => ({
  ExtensionSettingsPanel: () => <div data-testid="panel-extensions" />
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
  initReactI18next: {}
}))

vi.mock("@/i18n", () => ({
  default: { t: (key: string) => key, language: "ko" },
  detectSystemLanguage: () => "ko",
  normalizeLanguage: (v: unknown) => (v === "en" || v === "ko" ? v : null),
  setI18nLanguage: async () => {}
}))

import { SettingsDialog } from "@/components/SettingsDialog"

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
  mockConfirm.mockResolvedValue(true)
})

describe("SettingsDialog", () => {
  it("닫힌 상태에서는 다이얼로그를 렌더하지 않는다", () => {
    render(<SettingsDialog open={false} onClose={() => {}} />)
    expect(screen.queryByText("settings.title")).toBeNull()
  })

  it("열리면 타이틀과 일반 패널이 보인다", () => {
    render(<SettingsDialog open onClose={() => {}} />)
    expect(screen.getByText("settings.title")).toBeTruthy()
    expect(screen.getByTestId("panel-general")).toBeTruthy()
  })

  it("사이드바 탭 전환이 패널을 바꾼다", () => {
    render(<SettingsDialog open onClose={() => {}} />)

    fireEvent.click(screen.getByRole("button", { name: "settings.tabs.view" }))
    expect(screen.getByTestId("panel-view")).toBeTruthy()
    expect(screen.queryByTestId("panel-general")).toBeNull()

    fireEvent.click(screen.getByRole("button", { name: "settings.tabs.shortcuts" }))
    expect(screen.getByTestId("panel-shortcuts")).toBeTruthy()
  })

  it("확인 버튼이 onClose를 호출한다", () => {
    const onClose = vi.fn()
    render(<SettingsDialog open onClose={onClose} />)

    fireEvent.click(screen.getByRole("button", { name: "settings.confirm" }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it("초기화 버튼이 확인 후 테마를 system으로 되돌린다", async () => {
    render(<SettingsDialog open onClose={() => {}} />)

    fireEvent.click(screen.getByRole("button", { name: "settings.reset" }))
    await waitFor(() => {
      expect(mockConfirm).toHaveBeenCalledTimes(1)
    })
    await waitFor(() => {
      expect(mockSetTheme).toHaveBeenCalledWith("system")
    })
  })

  it("초기화 확인을 취소하면 테마를 건드리지 않는다", async () => {
    mockConfirm.mockResolvedValue(false)
    render(<SettingsDialog open onClose={() => {}} />)

    fireEvent.click(screen.getByRole("button", { name: "settings.reset" }))
    await waitFor(() => {
      expect(mockConfirm).toHaveBeenCalledTimes(1)
    })
    await new Promise((r) => setTimeout(r, 20))
    expect(mockSetTheme).not.toHaveBeenCalled()
  })
})
