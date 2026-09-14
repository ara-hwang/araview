import { beforeEach, describe, expect, it, vi } from "vitest"
import { renderHook, waitFor } from "@testing-library/react"

const { mockCheck, mockRelaunch, mockGetVersion, mockToast } = vi.hoisted(
  () => ({
    mockCheck: vi.fn(),
    mockRelaunch: vi.fn(),
    mockGetVersion: vi.fn(),
    mockToast: {
      success: vi.fn(),
      error: vi.fn(),
      message: vi.fn(),
      loading: vi.fn()
    }
  })
)

vi.mock("@tauri-apps/plugin-updater", () => ({ check: mockCheck }))
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mockRelaunch }))
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mockGetVersion }))
vi.mock("sonner", () => ({ toast: mockToast }))
vi.mock("@/i18n", () => ({
  default: {
    t: (key: string, opts?: Record<string, unknown>) =>
      opts?.version !== undefined || opts?.pct !== undefined
        ? `${key}:${opts.version ?? opts.pct}`
        : key
  }
}))

import {
  REQUEST_UPDATE_CHECK_EVENT,
  checkForUpdatesNow,
  requestUpdateCheck,
  useAppVersion
} from "@/hooks/useUpdater"

beforeEach(() => {
  vi.clearAllMocks()
})

describe("checkForUpdatesNow", () => {
  it("업데이트가 없으면 latest를 반환하고 성공 토스트를 표시한다", async () => {
    mockCheck.mockResolvedValueOnce(null)

    await expect(checkForUpdatesNow()).resolves.toBe("latest")

    expect(mockCheck).toHaveBeenCalledTimes(1)
    expect(mockToast.success).toHaveBeenCalledWith("toast.update.latest")
  })

  it("업데이트가 있으면 available을 반환하고 설치 액션을 제공한다", async () => {
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 100 } })
      onEvent({ event: "Progress", data: { chunkLength: 50 } })
      onEvent({ event: "Finished" })
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: "notes",
      downloadAndInstall
    })

    await expect(checkForUpdatesNow()).resolves.toBe("available")

    expect(mockToast.message).toHaveBeenCalledTimes(1)
    const action = mockToast.message.mock.calls[0][1].action
    expect(action.label).toBe("toast.update.install")

    action.onClick()
    await vi.waitFor(() => expect(downloadAndInstall).toHaveBeenCalled())
    await vi.waitFor(() =>
      expect(mockToast.success).toHaveBeenCalledWith(
        "toast.update.installed",
        expect.objectContaining({ id: "araview-update-install" })
      )
    )
    expect(mockToast.loading).toHaveBeenCalledWith(
      "toast.update.downloadingPct:50",
      expect.objectContaining({ id: "araview-update-install" })
    )
  })

  it("확인 중에는 두 번째 요청을 busy로 돌려보낸다", async () => {
    let resolveCheck!: (value: null) => void
    mockCheck.mockReturnValueOnce(
      new Promise<null>((resolve) => {
        resolveCheck = resolve
      })
    )

    const first = checkForUpdatesNow()
    await expect(checkForUpdatesNow()).resolves.toBe("busy")

    resolveCheck(null)
    await expect(first).resolves.toBe("latest")
  })

  it("확인에 실패하면 failed를 반환하고 에러 토스트를 표시한다", async () => {
    mockCheck.mockRejectedValueOnce(new Error("offline"))

    await expect(checkForUpdatesNow()).resolves.toBe("failed")

    expect(mockToast.error).toHaveBeenCalledWith(
      "toast.update.checkFail",
      expect.objectContaining({ description: "offline" })
    )
  })

  it("설치에 실패하면 설치 에러 토스트를 표시한다", async () => {
    const downloadAndInstall = vi
      .fn()
      .mockRejectedValueOnce(new Error("disk full"))
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      downloadAndInstall
    })

    await checkForUpdatesNow()

    const action = mockToast.message.mock.calls[0][1].action
    action.onClick()
    await vi.waitFor(() =>
      expect(mockToast.error).toHaveBeenCalledWith(
        "toast.update.installFail",
        expect.objectContaining({ description: "disk full" })
      )
    )
  })

  it("설치 완료 후 다시 시작 액션이 relaunch를 호출한다", async () => {
    mockRelaunch.mockResolvedValueOnce(undefined)
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined)
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      downloadAndInstall
    })

    await checkForUpdatesNow()

    const installAction = mockToast.message.mock.calls[0][1].action
    installAction.onClick()
    await vi.waitFor(() =>
      expect(mockToast.success).toHaveBeenCalledWith(
        "toast.update.installed",
        expect.anything()
      )
    )
    const installedCall = mockToast.success.mock.calls.find(
      (call) => call[0] === "toast.update.installed"
    )
    installedCall![1].action.onClick()
    await vi.waitFor(() => expect(mockRelaunch).toHaveBeenCalledTimes(1))
  })
})

describe("requestUpdateCheck", () => {
  it("전역 확인 요청 이벤트를 발생시킨다", () => {
    const handler = vi.fn()
    window.addEventListener(REQUEST_UPDATE_CHECK_EVENT, handler)
    try {
      requestUpdateCheck()
      expect(handler).toHaveBeenCalledTimes(1)
    } finally {
      window.removeEventListener(REQUEST_UPDATE_CHECK_EVENT, handler)
    }
  })
})

describe("useAppVersion", () => {
  it("getVersion 결과를 반환한다", async () => {
    mockGetVersion.mockResolvedValueOnce("1.2.3")

    const { result } = renderHook(() => useAppVersion())
    expect(result.current).toBeNull()

    await waitFor(() => expect(result.current).toBe("1.2.3"))
  })

  it("버전 조회 실패 시 null을 유지한다", async () => {
    mockGetVersion.mockRejectedValueOnce(new Error("denied"))

    const { result } = renderHook(() => useAppVersion())

    await waitFor(() => expect(mockGetVersion).toHaveBeenCalled())
    expect(result.current).toBeNull()
  })
})
