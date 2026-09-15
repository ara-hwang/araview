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
  __resetUpdateModuleForTests,
  checkForUpdatesNow,
  dismissUpdate,
  relaunchAfterUpdate,
  requestUpdateCheck,
  startUpdateDownload,
  useAppVersion
} from "@/hooks/useUpdater"
import { useUpdateStore } from "@/store/updateStore"

beforeEach(() => {
  vi.clearAllMocks()
  __resetUpdateModuleForTests()
})

describe("checkForUpdatesNow", () => {
  it("업데이트가 없으면 latest를 반환하고 성공 토스트를 표시한다", async () => {
    mockCheck.mockResolvedValueOnce(null)

    await expect(checkForUpdatesNow()).resolves.toBe("latest")

    expect(mockCheck).toHaveBeenCalledTimes(1)
    expect(mockToast.success).toHaveBeenCalledWith("toast.update.latest")
    expect(useUpdateStore.getState().stage).toBe("idle")
  })

  it("업데이트가 있으면 available을 반환하고 Dialog 상태를 연다", async () => {
    mockCheck.mockResolvedValueOnce({ version: "9.9.9", body: "notes" })

    await expect(checkForUpdatesNow()).resolves.toBe("available")

    const state = useUpdateStore.getState()
    expect(state.stage).toBe("available")
    expect(state.version).toBe("9.9.9")
    expect(state.body).toBe("notes")
    expect(mockToast.message).not.toHaveBeenCalled()
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

  it("Dialog이 열려 있으면 새 확인을 busy로 돌려보낸다", async () => {
    mockCheck.mockResolvedValueOnce({ version: "9.9.9", body: null })
    await checkForUpdatesNow()
    expect(useUpdateStore.getState().stage).toBe("available")

    await expect(checkForUpdatesNow()).resolves.toBe("busy")
    expect(mockCheck).toHaveBeenCalledTimes(1)
  })

  it("확인에 실패하면 failed를 반환하고 에러 토스트를 표시한다", async () => {
    mockCheck.mockRejectedValueOnce(new Error("offline"))

    await expect(checkForUpdatesNow()).resolves.toBe("failed")

    expect(mockToast.error).toHaveBeenCalledWith(
      "toast.update.checkFail",
      expect.objectContaining({ description: "offline" })
    )
  })
})

describe("startUpdateDownload", () => {
  it("다운로드 진행률을 저장하고 완료 시 ready가 된다", async () => {
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
    await checkForUpdatesNow()

    await startUpdateDownload()

    expect(downloadAndInstall).toHaveBeenCalledTimes(1)
    const state = useUpdateStore.getState()
    expect(state.stage).toBe("ready")
    expect(state.pct).toBe(100)
  })

  it("다운로드에 실패하면 에러를 저장한다", async () => {
    const downloadAndInstall = vi
      .fn()
      .mockRejectedValueOnce(new Error("disk full"))
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()

    const state = useUpdateStore.getState()
    expect(state.stage).toBe("downloading")
    expect(state.error).toBe("disk full")
  })
})

describe("dismissUpdate", () => {
  it("진행 중에는 닫기를 무시한다", async () => {
    let resolveDownload!: () => void
    const downloadAndInstall = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveDownload = resolve
        })
    )
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()
    const pending = startUpdateDownload()
    await vi.waitFor(() =>
      expect(useUpdateStore.getState().stage).toBe("downloading")
    )

    dismissUpdate(false)
    expect(useUpdateStore.getState().stage).toBe("downloading")

    resolveDownload()
    await pending
    expect(useUpdateStore.getState().stage).toBe("ready")
  })

  it("deferred로 닫으면 다음 실행 적용 토스트를 표시한다", async () => {
    mockCheck.mockResolvedValueOnce({ version: "9.9.9", body: null })
    await checkForUpdatesNow()

    dismissUpdate(true)

    expect(useUpdateStore.getState().stage).toBe("idle")
    expect(mockToast.success).toHaveBeenCalledWith("toast.update.deferred")
  })
})

describe("relaunchAfterUpdate", () => {
  it("relaunch를 호출한다", async () => {
    mockRelaunch.mockResolvedValueOnce(undefined)

    await relaunchAfterUpdate()

    expect(mockRelaunch).toHaveBeenCalledTimes(1)
  })

  it("relaunch 실패 시 에러 토스트를 표시한다", async () => {
    mockRelaunch.mockRejectedValueOnce(new Error("denied"))

    await relaunchAfterUpdate()

    expect(mockToast.error).toHaveBeenCalledWith(
      "toast.update.restartFail",
      expect.objectContaining({ description: "denied" })
    )
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
