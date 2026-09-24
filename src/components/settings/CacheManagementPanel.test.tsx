import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type { CacheStats } from "@/types"

const h = vi.hoisted(() => ({
  confirm: vi.fn(),
  updateSettings: vi.fn(),
  clear: vi.fn(),
  refresh: vi.fn(),
  manager: {
    stats: null as CacheStats | null,
    loading: false,
    clearingScope: null,
    error: null as string | null
  }
}))

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: h.confirm
}))

vi.mock("@/hooks/useCacheManager", () => ({
  useCacheManager: () => ({
    ...h.manager,
    refresh: h.refresh,
    clear: h.clear
  })
}))

vi.mock("@/store/settingsStore", () => ({
  updateSettings: h.updateSettings,
  useSettingsStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({ cacheStorageMode: "persistent", language: "en" })
}))

vi.mock("@/components/ui/toast", () => ({
  toast: {
    success: vi.fn(),
    info: vi.fn(),
    error: vi.fn()
  }
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) =>
      values ? `${key}:${Object.values(values).join(",")}` : key
  })
}))

import { CacheManagementPanel } from "@/components/settings/CacheManagementPanel"

const stats: CacheStats = {
  storage_mode: "persistent",
  persistent_available: true,
  total_bytes: 1024,
  file_count: 2,
  protected_bytes: 0,
  protected_file_count: 0,
  total_limit_bytes: 2 * 1024 * 1024 * 1024,
  categories: [
    {
      key: "thumbnails",
      bytes: 1024,
      file_count: 2,
      protected_bytes: 0,
      protected_file_count: 0,
      limit_bytes: 500 * 1024 * 1024
    }
  ]
}

beforeEach(() => {
  cleanup()
  vi.clearAllMocks()
  h.manager.stats = stats
  h.manager.loading = false
  h.manager.clearingScope = null
  h.manager.error = null
  h.updateSettings.mockResolvedValue(true)
  h.clear.mockResolvedValue({
    removed_bytes: 1024,
    removed_file_count: 2,
    failed_file_count: 0,
    stats: { ...stats, total_bytes: 0, file_count: 0, categories: [] }
  })
  h.refresh.mockResolvedValue(stats)
})

afterEach(() => {
  cleanup()
})

describe("CacheManagementPanel", () => {
  it("종류별 사용량과 전체 삭제 버튼을 표시한다", () => {
    render(<CacheManagementPanel />)

    expect(screen.getByText("settings.cacheManagement.total")).toBeTruthy()
    expect(screen.getByText("settings.cacheManagement.categories.thumbnails")).toBeTruthy()
    expect(screen.getByRole("button", { name: /settings.cacheManagement.clearAll/ })).toBeTruthy()
  })

  it("전체 삭제는 확인 후에만 실행한다", async () => {
    h.confirm.mockResolvedValue(true)
    render(<CacheManagementPanel />)

    fireEvent.click(screen.getByRole("button", { name: /settings.cacheManagement.clearAll/ }))

    await waitFor(() => expect(h.confirm).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(h.clear).toHaveBeenCalledWith("all"))
  })

  it("영구 캐시 스위치는 저장 설정을 변경한다", async () => {
    render(<CacheManagementPanel />)
    const toggle = screen.getByRole("switch", { name: "settings.cacheStorage.toggle" })

    fireEvent.click(toggle)

    await waitFor(() => expect(h.confirm).toHaveBeenCalledTimes(1))
    expect(h.updateSettings).toHaveBeenCalledWith({ cacheStorageMode: "temporary" })
  })
})
