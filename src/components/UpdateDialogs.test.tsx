import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const { mockDismiss, mockStart } = vi.hoisted(() => ({
  mockDismiss: vi.fn(),
  mockStart: vi.fn()
}))

vi.mock("@/hooks/useUpdater", () => ({
  dismissUpdate: mockDismiss,
  startUpdateDownload: mockStart
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}(${Object.values(opts).join(",")})` : key
  })
}))

import { UpdateDialogs } from "@/components/UpdateDialogs"
import { useUpdateStore } from "@/store/updateStore"

beforeEach(() => {
  vi.clearAllMocks()
  useUpdateStore.getState().dismiss()
})

afterEach(() => {
  cleanup()
  useUpdateStore.getState().dismiss()
})

describe("UpdateDialogs", () => {
  it("available 단계에서 다운로드와 나중에 버튼을 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", "notes")
    render(<UpdateDialogs />)

    expect(screen.getByTestId("update-dialog")).toBeTruthy()
    fireEvent.click(screen.getByTestId("update-download"))
    expect(mockStart).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByTestId("update-later"))
    expect(mockDismiss).toHaveBeenCalledTimes(1)
  })

  it("available 단계에서 해당 버전의 GitHub 릴리스 페이지 링크를 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", "notes")
    render(<UpdateDialogs />)
    const link = screen.getByRole("link", { name: "dialog.update.releasePage" })
    expect(link.getAttribute("href")).toBe(
      "https://github.com/ara-hwang/araview/releases/tag/v9.9.9"
    )
  })

  it("downloading 단계에서 진행률과 누적 바이트를 표시한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setProgress(50, 1024 * 1024, 2 * 1024 * 1024)
    render(<UpdateDialogs />)

    const bar = screen.getByTestId("update-progress")
    expect(bar.getAttribute("aria-valuenow")).toBe("50")
    expect(screen.getByTestId("update-bytes").textContent).toContain("1.0 MB")
    expect(screen.getByTestId("update-bytes").textContent).toContain("2.0 MB")
    expect(screen.getByTestId("update-bytes").textContent).toContain("50")
    expect(screen.queryByTestId("update-retry")).toBeNull()
  })

  it("총 길이를 모르면 이동형 불확정 바와 누적 바이트만 표시한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setProgress(null, 300, null)
    render(<UpdateDialogs />)

    const bar = screen.getByTestId("update-progress")
    expect(bar.getAttribute("aria-valuenow")).toBeNull()
    expect(bar.firstElementChild?.className).toContain("animate-progress-indeterminate")
    expect(screen.getByTestId("update-bytes").textContent).toContain("300 B")
  })

  it("downloading 에러 시 다시 시도와 취소를 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setDownloadError("disk full")
    render(<UpdateDialogs />)

    fireEvent.click(screen.getByTestId("update-retry"))
    expect(mockStart).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByText("dialog.cancel"))
    expect(mockDismiss).toHaveBeenCalledTimes(1)
  })

  it("installing 단계에서 인계 안내를 표시하고 닫기 수단을 숨긴다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().markInstalling()
    render(<UpdateDialogs />)

    expect(screen.getByText("dialog.update.installingTitle")).toBeTruthy()
    expect(screen.getByText("dialog.update.installingDesc")).toBeTruthy()
    expect(screen.getByTestId("update-progress").getAttribute("aria-valuenow")).toBe("100")
    expect(screen.queryByText("Close")).toBeNull()
    expect(screen.queryByTestId("update-retry")).toBeNull()
  })

  it("installing 에러 시 다시 시도와 취소를 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().markInstalling()
    useUpdateStore.getState().setDownloadError("denied")
    render(<UpdateDialogs />)

    expect(screen.getByText("dialog.update.installFail")).toBeTruthy()
    fireEvent.click(screen.getByTestId("update-retry"))
    expect(mockStart).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByText("dialog.cancel"))
    expect(mockDismiss).toHaveBeenCalledTimes(1)
  })

  it("idle 단계에서는 Dialog를 렌더하지 않는다", () => {
    render(<UpdateDialogs />)
    expect(screen.queryByTestId("update-dialog")).toBeNull()
  })
})
