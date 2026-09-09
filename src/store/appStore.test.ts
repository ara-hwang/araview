import { beforeEach, describe, expect, it } from "vitest"
import { closeImage, useAppStore } from "@/store/appStore"

beforeEach(() => {
  closeImage()
})

describe("closeImage", () => {
  it("열린 이미지 상태를 초기값으로 되돌린다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg"
      },
      dirImages: { images: ["/pics/a.jpg", "/pics/b.jpg"], current_index: 1 },
      error: "boom",
      loading: true,
      position: { x: 10, y: 20 },
      zoom: 2.5,
      rotation: 90,
      flipH: true,
      flipV: true,
      exifData: {},
      showExifPanel: true,
      archivePath: "/docs/m.cbz"
    })

    closeImage()

    const state = useAppStore.getState()
    expect(state.imageInfo).toBeNull()
    expect(state.dirImages).toEqual({ images: [], current_index: 0 })
    expect(state.error).toBeNull()
    expect(state.loading).toBe(false)
    expect(state.position).toEqual({ x: 0, y: 0 })
    expect(state.zoom).toBe(1)
    expect(state.rotation).toBe(0)
    expect(state.flipH).toBe(false)
    expect(state.flipV).toBe(false)
    expect(state.exifData).toBeNull()
    expect(state.showExifPanel).toBe(false)
    expect(state.archivePath).toBeNull()
  })
})
