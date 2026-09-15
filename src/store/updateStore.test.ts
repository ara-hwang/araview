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
    expect(state.error).toBeNull()
  })

  it("available, downloading, ready 순서로 전이한다", () => {
    const store = useUpdateStore.getState()
    store.showAvailable("1.2.0", "notes")
    expect(useUpdateStore.getState().stage).toBe("available")

    useUpdateStore.getState().startDownload()
    expect(useUpdateStore.getState().stage).toBe("downloading")

    useUpdateStore.getState().setProgress(50)
    expect(useUpdateStore.getState().pct).toBe(50)

    useUpdateStore.getState().markReady()
    const ready = useUpdateStore.getState()
    expect(ready.stage).toBe("ready")
    expect(ready.pct).toBe(100)
  })

  it("에러를 저장하고 dismiss로 초기화한다", () => {
    useUpdateStore.getState().showAvailable("1.2.0", null)
    useUpdateStore.getState().startDownload()
    useUpdateStore.getState().setDownloadError("disk full")

    expect(useUpdateStore.getState().error).toBe("disk full")

    useUpdateStore.getState().dismiss()
    expect(useUpdateStore.getState().stage).toBe("idle")
    expect(useUpdateStore.getState().error).toBeNull()
  })
})
