import { act, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const h = vi.hoisted(() => ({
  exif: null as null | (() => Promise<unknown>)
}))

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (cmd: string) => {
    if (cmd === "get_exif_data" && h.exif) return h.exif()
    return Promise.resolve(null)
  }
}))

import { useExifLoader } from "@/hooks/useExifLoader"
import { closeImage, useAppStore } from "@/store/appStore"

beforeEach(() => {
  closeImage()
  useAppStore.setState({
    imageInfo: {
      file_path: "/pics/a.jpg",
      source_path: "/pics/a.jpg",
      mime_type: "image/jpeg",
      file_name: "a.jpg",
      file_size: 1,
      width: 1,
      height: 1
    },
    exifData: { stale: "1" },
    exifError: "stale"
  })
})

async function reload() {
  const { result } = renderHook(() => useExifLoader())
  await act(async () => {
    await result.current.reloadExif()
  })
}

describe("useExifLoader EXIF 상태", () => {
  it("빈 맵은 오류가 아닌 EXIF 없음 상태다", async () => {
    h.exif = async () => ({})
    await reload()
    expect(useAppStore.getState().exifData).toBeNull()
    expect(useAppStore.getState().exifError).toBeNull()
  })

  it("태그가 있으면 그대로 보여준다", async () => {
    h.exif = async () => ({ Make: "Camera" })
    await reload()
    expect(useAppStore.getState().exifData).toEqual({ Make: "Camera" })
    expect(useAppStore.getState().exifError).toBeNull()
  })

  it("명령이 실패하면 오류 상태로 남긴다", async () => {
    h.exif = async () => {
      throw { code: "not_found", message: "File not found" }
    }
    await reload()
    expect(useAppStore.getState().exifData).toBeNull()
    expect(useAppStore.getState().exifError).toBe("File not found")
  })
})

describe("useExifLoader 패널 열기", () => {
  it("읽기가 끝나기 전에 패널을 열고 로딩 상태를 보여준다", async () => {
    let finish: (value: unknown) => void = () => {}
    h.exif = () => new Promise((resolve) => (finish = resolve))
    const { result } = renderHook(() => useExifLoader())

    let pending: Promise<void> = Promise.resolve()
    act(() => {
      pending = result.current.toggleExifPanel()
    })
    expect(useAppStore.getState().showExifPanel).toBe(true)
    expect(useAppStore.getState().infoLoading).toBe(true)
    // 직전 이미지의 값은 새 패널에 남기지 않는다.
    expect(useAppStore.getState().exifData).toBeNull()

    await act(async () => {
      finish({ Make: "Camera" })
      await pending
    })
    expect(useAppStore.getState().infoLoading).toBe(false)
    expect(useAppStore.getState().exifData).toEqual({ Make: "Camera" })
  })
})
