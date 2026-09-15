import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, screen } from "@testing-library/react"

const { mockDismiss, mockStart, mockRelaunch } = vi.hoisted(() => ({
  mockDismiss: vi.fn(),
  mockStart: vi.fn(),
  mockRelaunch: vi.fn()
}))

vi.mock("@/hooks/useUpdater", () => ({
  dismissUpdate: mockDismiss,
  startUpdateDownload: mockStart,
  relaunchAfterUpdate: mockRelaunch
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
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
    expect(mockDismiss).toHaveBeenCalledWith(false)
  })

  it("downloading 단계에서 진행률을 표시하고 에러 시 다시 시도를 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setProgress(50)
    render(<UpdateDialogs />)

    const bar = screen.getByTestId("update-progress")
    expect(bar.getAttribute("aria-valuenow")).toBe("50")
    expect(screen.queryByTestId("update-retry")).toBeNull()
    cleanup()

    useUpdateStore.getState().setDownloadError("disk full")
    render(<UpdateDialogs />)
    fireEvent.click(screen.getByTestId("update-retry"))
    expect(mockStart).toHaveBeenCalled()
  })

  it("ready 단계에서 다시 시작과 닫은 후 적용을 제공한다", () => {
    useUpdateStore.getState().showAvailable("9.9.9", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().markReady()
    render(<UpdateDialogs />)

    fireEvent.click(screen.getByTestId("update-restart-now"))
    expect(mockRelaunch).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByTestId("update-apply-on-exit"))
    expect(mockDismiss).toHaveBeenCalledWith(true)
  })

  it("idle 단계에서는 Dialog를 렌더하지 않는다", () => {
    render(<UpdateDialogs />)
    expect(screen.queryByTestId("update-dialog")).toBeNull()
  })
})
