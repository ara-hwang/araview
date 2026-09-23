import { beforeEach, describe, expect, it } from "vitest"

import { useUpdateStore } from "@/store/updateStore"

beforeEach(() => {
  useUpdateStore.getState().dismiss()
})

describe("updateStore", () => {
  it("초기 상태는 idle이다", () => {
    const state = useUpdateStore.getState()
    expect(state.stage).toBe("idle")
    expect(state.version).toBeNull()
    expect(state.pct).toBeNull()
    expect(state.downloadedBytes).toBe(0)
    expect(state.totalBytes).toBeNull()
    expect(state.error).toBeNull()
  })

  it("available, downloading, installing 순서로 전이한다", () => {
    const store = useUpdateStore.getState()
    store.showAvailable("1.2.0", "notes")
    expect(useUpdateStore.getState().stage).toBe("available")

    useUpdateStore.getState().startDownload()
    expect(useUpdateStore.getState().stage).toBe("downloading")

    useUpdateStore.getState().setProgress(50, 512, 1024)
    const downloading = useUpdateStore.getState()
    expect(downloading.pct).toBe(50)
    expect(downloading.downloadedBytes).toBe(512)
    expect(downloading.totalBytes).toBe(1024)

    useUpdateStore.getState().markInstalling()
    const installing = useUpdateStore.getState()
    expect(installing.stage).toBe("installing")
    expect(installing.pct).toBe(100)
  })

  it("다시 다운로드를 시작하면 진행 값이 초기화된다", () => {
    useUpdateStore.getState().showAvailable("1.2.0", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setProgress(50, 512, 1024)

    useUpdateStore.getState().startDownload()
    const state = useUpdateStore.getState()
    expect(state.pct).toBeNull()
    expect(state.downloadedBytes).toBe(0)
    expect(state.totalBytes).toBeNull()
  })

  it("에러를 저장하고 dismiss로 초기화한다", () => {
    useUpdateStore.getState().showAvailable("1.2.0", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setDownloadError("disk full")

    expect(useUpdateStore.getState().error).toBe("disk full")

    useUpdateStore.getState().dismiss()
    const state = useUpdateStore.getState()
    expect(state.stage).toBe("idle")
    expect(state.error).toBeNull()
    expect(state.downloadedBytes).toBe(0)
  })
})
