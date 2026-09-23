import { renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { mockCheck, mockGetVersion, mockToast } = vi.hoisted(() => ({
  mockCheck: vi.fn(),
  mockGetVersion: vi.fn(),
  mockToast: {
    add: vi.fn(),
    close: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn()
  }
}))

vi.mock("@tauri-apps/plugin-updater", () => ({ check: mockCheck }))
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mockGetVersion }))
vi.mock("@/components/ui/toast", () => ({ toast: mockToast }))
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
  computeDownloadPct,
  dismissUpdate,
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
    expect(mockToast.success).not.toHaveBeenCalled()
    expect(mockToast.error).not.toHaveBeenCalled()
    expect(mockToast.info).not.toHaveBeenCalled()
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

describe("computeDownloadPct", () => {
  it("정상 범위에서는 반올림한 정수를 돌려준다", () => {
    expect(computeDownloadPct(0, 100)).toBe(0)
    expect(computeDownloadPct(50, 100)).toBe(50)
    expect(computeDownloadPct(1, 3)).toBe(33)
    expect(computeDownloadPct(2, 3)).toBe(67)
    expect(computeDownloadPct(100, 100)).toBe(100)
  })

  it("누적값이 총 길이를 초과해도 100에 고정한다", () => {
    expect(computeDownloadPct(150, 100)).toBe(100)
    expect(computeDownloadPct(10274949 + 8192, 10274949)).toBe(100)
  })

  it("총 길이를 알 수 없거나 비정상이면 null을 돌려준다", () => {
    expect(computeDownloadPct(50, undefined)).toBeNull()
    expect(computeDownloadPct(50, null)).toBeNull()
    expect(computeDownloadPct(50, 0)).toBeNull()
    expect(computeDownloadPct(50, -100)).toBeNull()
    expect(computeDownloadPct(50, Number.NaN)).toBeNull()
    expect(computeDownloadPct(50, Number.POSITIVE_INFINITY)).toBeNull()
  })

  it("누적값이 비정상이면 null을 돌려준다", () => {
    expect(computeDownloadPct(Number.NaN, 100)).toBeNull()
    expect(computeDownloadPct(-1, 100)).toBeNull()
    expect(computeDownloadPct(Number.POSITIVE_INFINITY, 100)).toBeNull()
  })
})

describe("startUpdateDownload", () => {
  it("다운로드 진행률과 누적 바이트를 저장하고 완료 시 installing이 된다", async () => {
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
    expect(state.stage).toBe("installing")
    expect(state.pct).toBe(100)
    expect(state.downloadedBytes).toBe(50)
    expect(state.totalBytes).toBe(100)
  })

  it("Finished에서 바를 100으로 마무리한다", async () => {
    let pctAtFinished: number | null | undefined
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 100 } })
      onEvent({ event: "Progress", data: { chunkLength: 30 } })
      onEvent({ event: "Finished" })
      pctAtFinished = useUpdateStore.getState().pct
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: "notes",
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()

    expect(pctAtFinished).toBe(100)
    expect(useUpdateStore.getState().stage).toBe("installing")
  })

  it("총 길이를 알 수 없으면 pct는 null이고 바이트만 누적된다", async () => {
    let pctAfterProgress: number | null | undefined = -1
    let bytesAfterProgress: number | undefined
    let totalAfterProgress: number | null | undefined
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: null } })
      onEvent({ event: "Progress", data: { chunkLength: 30 } })
      onEvent({ event: "Progress", data: { chunkLength: 12 } })
      const state = useUpdateStore.getState()
      pctAfterProgress = state.pct
      bytesAfterProgress = state.downloadedBytes
      totalAfterProgress = state.totalBytes
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()

    expect(pctAfterProgress).toBeNull()
    expect(bytesAfterProgress).toBe(42)
    expect(totalAfterProgress).toBeNull()
  })

  it("Started에서 누적 바이트를 초기화한다", async () => {
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Progress", data: { chunkLength: 50 } })
      onEvent({ event: "Started", data: { contentLength: 100 } })
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()

    const state = useUpdateStore.getState()
    expect(state.downloadedBytes).toBe(0)
    expect(state.pct).toBeNull()
  })

  it("비정상 chunk는 무시하고 유효한 chunk로 진행률을 계산한다", async () => {
    const seen: Array<number | null> = []
    let bytesAfterInvalid: number | undefined
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 100 } })
      onEvent({ event: "Progress", data: { chunkLength: Number.NaN } })
      onEvent({ event: "Progress", data: { chunkLength: -5 } })
      bytesAfterInvalid = useUpdateStore.getState().downloadedBytes
      seen.push(useUpdateStore.getState().pct)
      onEvent({ event: "Progress", data: { chunkLength: 50 } })
      seen.push(useUpdateStore.getState().pct)
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()

    expect(bytesAfterInvalid).toBe(0)
    expect(seen).toEqual([null, 50])
  })

  it("다운로드에 실패하면 에러를 저장한다", async () => {
    const downloadAndInstall = vi.fn().mockRejectedValueOnce(new Error("disk full"))
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
    await vi.waitFor(() => expect(useUpdateStore.getState().stage).toBe("downloading"))

    dismissUpdate()
    expect(useUpdateStore.getState().stage).toBe("downloading")

    resolveDownload()
    await pending
    // Finished 없이 resolve되는 경로는 Windows에서 도달하지 않으므로
    // stage를 바꾸지 않는다(방어적 유지).
    expect(useUpdateStore.getState().stage).toBe("downloading")
  })

  it("installing 단계에서도 닫기를 무시한다", async () => {
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Finished" })
      return Promise.resolve()
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()
    expect(useUpdateStore.getState().stage).toBe("installing")

    dismissUpdate()
    expect(useUpdateStore.getState().stage).toBe("installing")
  })

  it("installing 단계에서 에러가 나면 닫을 수 있다", async () => {
    const downloadAndInstall = vi.fn((onEvent: (e: unknown) => void) => {
      onEvent({ event: "Finished" })
      return Promise.reject(new Error("denied"))
    })
    mockCheck.mockResolvedValueOnce({
      version: "9.9.9",
      body: null,
      downloadAndInstall
    })
    await checkForUpdatesNow()

    await startUpdateDownload()
    expect(useUpdateStore.getState().stage).toBe("installing")
    expect(useUpdateStore.getState().error).toBe("denied")

    dismissUpdate()
    expect(useUpdateStore.getState().stage).toBe("idle")
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
