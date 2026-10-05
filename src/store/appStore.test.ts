import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => ({
      get: vi.fn(async () => null),
      set: vi.fn(async () => {}),
      save: vi.fn(async () => {})
    }))
  }
}))

import {
  applyImageNaturalSize,
  applyRememberedFit,
  closeImage,
  panBy,
  resetZoomPan,
  setImageInfoAndResetView,
  setZoomToFit,
  useAppStore,
  zoomInBy,
  zoomOutBy
} from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

beforeEach(() => {
  closeImage()
  useSettingsStore.setState({ fitMode: "auto" })
})

describe("closeImage", () => {
  it("열린 이미지 상태를 초기값으로 되돌린다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        source_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 800,
        height: 600
      },
      dirImages: {
        images: ["/pics/a.jpg", "/pics/b.jpg"],
        current_index: 1,
        availability: ["local", "local"]
      },
      error: "boom",
      loading: true,
      position: { x: 10, y: 20 },
      zoom: 2.5,
      rotation: 90,
      flipH: true,
      flipV: true,
      exifData: {},
      showExifPanel: true,
      archivePath: "/docs/m.cbz",
      archivePreviewPath: "/docs/preview.cbz"
    })

    closeImage()

    const state = useAppStore.getState()
    expect(state.imageInfo).toBeNull()
    expect(state.dirImages).toEqual({ images: [], current_index: 0, availability: [] })
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
    expect(state.archivePreviewPath).toBeNull()
  })
})

describe("panBy", () => {
  it("이미지가 없으면 이동하지 않는다", () => {
    panBy(48, 0)
    expect(useAppStore.getState().position).toEqual({ x: 0, y: 0 })
  })

  it("경계 안에서는 상대 이동한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        source_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 800,
        height: 600
      },
      containerSize: { width: 400, height: 300 },
      imageSize: { width: 800, height: 600 },
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    panBy(48, 0)
    expect(useAppStore.getState().position.x).toBe(48)
    panBy(-48, 0)
    expect(useAppStore.getState().position.x).toBe(0)
  })
})

describe("setImageInfoAndResetView", () => {
  it("이전 파일의 줌/팬/회전 상태에서 새 파일의 맞춤 줌으로 원자 교체한다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 500, height: 500 },
      zoom: 2.5,
      position: { x: 120, y: -80 },
      rotation: 90,
      flipH: true,
      flipV: true,
      error: "boom"
    })
    setImageInfoAndResetView({
      file_path: "/pics/b.jpg",
      source_path: "/pics/b.jpg",
      file_name: "b.jpg",
      file_size: 456,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1000
    })
    const state = useAppStore.getState()
    expect(state.imageInfo?.file_path).toBe("/pics/b.jpg")
    // 2000x1000을 1000x700에 맞춤: min(0.5, 0.7) = 0.5
    expect(state.zoom).toBeCloseTo(0.5)
    expect(state.imageSize).toEqual({ width: 2000, height: 1000 })
    expect(state.position).toEqual({ x: 0, y: 0 })
    expect(state.rotation).toBe(0)
    expect(state.flipH).toBe(false)
    expect(state.flipV).toBe(false)
    expect(state.error).toBeNull()
  })

  it("작은 이미지는 100%를 넘지 않는다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      zoom: 3,
      position: { x: 50, y: 50 }
    })
    setImageInfoAndResetView({
      file_path: "/pics/small.png",
      source_path: "/pics/small.png",
      file_name: "small.png",
      file_size: 10,
      mime_type: "image/png",
      width: 100,
      height: 100
    })
    expect(useAppStore.getState().zoom).toBe(1)
  })

  it("치수를 모르면 이전 크기를 비우고 onLoad 보정에 맡긴다", () => {
    useAppStore.setState({
      imageSize: { width: 2000, height: 1000 },
      zoom: 0.5,
      position: { x: 30, y: 30 }
    })
    setImageInfoAndResetView({
      file_path: "/pics/vector.svg",
      source_path: "/pics/vector.svg",
      file_name: "vector.svg",
      file_size: 10,
      mime_type: "image/svg+xml",
      width: null,
      height: null
    })
    const state = useAppStore.getState()
    expect(state.imageSize).toEqual({ width: 0, height: 0 })
    expect(state.zoom).toBe(1)
    expect(state.position).toEqual({ x: 0, y: 0 })
  })
})

describe("applyImageNaturalSize", () => {
  it("SVG 백엔드 치수와 종횡비가 같으면 natural 덮어쓰기를 건너뛴다", () => {
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/vector.svg",
      source_path: "/pics/vector.svg",
      file_name: "vector.svg",
      file_size: 10,
      mime_type: "image/svg+xml",
      width: 400,
      height: 300
    })
    const before = useAppStore.getState()
    expect(before.imageSize).toEqual({ width: 400, height: 300 })
    // viewBox-only SVG의 브라우저 기본 natural(200x150): 비율만 같으므로 유지
    applyImageNaturalSize(200, 150)
    const after = useAppStore.getState()
    expect(after.imageSize).toEqual({ width: 400, height: 300 })
    expect(after.zoom).toBe(before.zoom)
  })

  it("SVG라도 종횡비가 다르면 natural로 보정한다", () => {
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/vector.svg",
      source_path: "/pics/vector.svg",
      file_name: "vector.svg",
      file_size: 10,
      mime_type: "image/svg+xml",
      width: 400,
      height: 300
    })
    applyImageNaturalSize(100, 100)
    expect(useAppStore.getState().imageSize).toEqual({ width: 100, height: 100 })
  })

  it("래스터는 기존대로 natural로 보정한다", () => {
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/a.jpg",
      source_path: "/pics/a.jpg",
      file_name: "a.jpg",
      file_size: 10,
      mime_type: "image/jpeg",
      width: 400,
      height: 300
    })
    applyImageNaturalSize(200, 150)
    expect(useAppStore.getState().imageSize).toEqual({ width: 200, height: 150 })
  })

  it("증분 확대는 래스터 10x에서 멈춘다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        source_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 10,
        mime_type: "image/jpeg",
        width: 400,
        height: 300
      },
      zoom: 9
    })
    zoomInBy()
    expect(useAppStore.getState().zoom).toBe(10)
    zoomInBy()
    expect(useAppStore.getState().zoom).toBe(10)
  })

  it("증분 확대는 SVG 40x까지 허용한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/vector.svg",
        source_path: "/pics/vector.svg",
        file_name: "vector.svg",
        file_size: 10,
        mime_type: "image/svg+xml",
        width: 400,
        height: 300
      },
      zoom: 9
    })
    zoomInBy()
    expect(useAppStore.getState().zoom).toBeCloseTo(11.25)
    useAppStore.setState({ zoom: 39 })
    zoomInBy()
    expect(useAppStore.getState().zoom).toBe(40)
    zoomInBy()
    expect(useAppStore.getState().zoom).toBe(40)
  })
})

describe("줌 기준점", () => {
  // 2000x1000 이미지를 1000x700 컨테이너에서 본다.
  const open = (zoom: number, position = { x: 0, y: 0 }) =>
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom,
      position
    })

  it("기준점이 없으면 컨테이너 중심 기준으로 위치를 같은 비율로 옮긴다", () => {
    open(1, { x: 100, y: 50 })
    zoomInBy(2)
    expect(useAppStore.getState().position).toEqual({ x: 200, y: 100 })
  })

  it("확대해도 커서 아래의 이미지 지점이 그대로 있다", () => {
    open(1, { x: 100, y: 50 })
    const anchor = { x: 300, y: -120 }
    // 커서 아래 지점의 이미지 좌표(중심 기준): (anchor - position) / zoom
    const before = { x: (anchor.x - 100) / 1, y: (anchor.y - 50) / 1 }
    zoomInBy(2, anchor)
    const { zoom, position } = useAppStore.getState()
    expect(zoom).toBe(2)
    expect((anchor.x - position.x) / zoom).toBeCloseTo(before.x)
    expect((anchor.y - position.y) / zoom).toBeCloseTo(before.y)
  })

  it("축소해도 커서 아래의 이미지 지점이 그대로 있다", () => {
    open(4, { x: 400, y: 200 })
    const anchor = { x: -250, y: 150 }
    const before = { x: (anchor.x - 400) / 4, y: (anchor.y - 200) / 4 }
    zoomOutBy(2, anchor)
    const { zoom, position } = useAppStore.getState()
    expect(zoom).toBe(2)
    expect((anchor.x - position.x) / zoom).toBeCloseTo(before.x)
    expect((anchor.y - position.y) / zoom).toBeCloseTo(before.y)
  })

  it("기준점으로 옮긴 위치도 컨테이너 경계로 clamp한다", () => {
    // 0.5배(1000x500)는 컨테이너 안에 들어오므로 어느 쪽으로도 옮길 수 없다.
    open(1, { x: 500, y: 150 })
    zoomOutBy(2, { x: -500, y: -350 })
    const { zoom, position } = useAppStore.getState()
    expect(zoom).toBe(0.5)
    expect(position).toEqual({ x: 0, y: 0 })
  })

  it("위치가 그대로면 위치 객체를 바꾸지 않는다", () => {
    open(1)
    const before = useAppStore.getState().position
    zoomInBy(2)
    expect(useAppStore.getState().position).toBe(before)
  })
})

describe("setZoomToFit", () => {
  it("너비 맞춤은 컨테이너 너비와 일치한다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 1,
      position: { x: 10, y: 20 }
    })
    setZoomToFit("width")
    const state = useAppStore.getState()
    expect(state.zoom).toBeCloseTo(0.5)
    expect(state.position).toEqual({ x: 0, y: 0 })
    expect(state.imageSize.width * state.zoom).toBeCloseTo(1000)
  })

  it("높이 맞춤은 컨테이너 높이와 일치한다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1400 },
      rotation: 0,
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    setZoomToFit("height")
    const state = useAppStore.getState()
    expect(state.zoom).toBeCloseTo(0.5)
    expect(state.imageSize.height * state.zoom).toBeCloseTo(700)
  })

  it("화면 맞춤은 양쪽 축을 모두 containment한다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    setZoomToFit("screen")
    const state = useAppStore.getState()
    expect(state.zoom).toBeCloseTo(0.5)
    expect(state.imageSize.width * state.zoom).toBeLessThanOrEqual(1000.001)
    expect(state.imageSize.height * state.zoom).toBeLessThanOrEqual(700.001)
  })

  it("90도 회전 시 바뀐 치수 기준으로 맞춘다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 1000, height: 2000 },
      rotation: 90,
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    setZoomToFit("width")
    // 회전하면 보이는 너비는 원본 높이(2000)이므로 1000/2000 = 0.5
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })

  it("컨테이너 크기를 모르면 줌을 바꾸지 않는다", () => {
    useAppStore.setState({
      containerSize: { width: 0, height: 0 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    setZoomToFit("width")
    expect(useAppStore.getState().zoom).toBe(1)
    setZoomToFit("screen")
    expect(useAppStore.getState().zoom).toBe(1)
  })

  it("맞춤 모드를 기억한다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 1,
      position: { x: 0, y: 0 }
    })
    setZoomToFit("width")
    expect(useSettingsStore.getState().fitMode).toBe("width")
    setZoomToFit("height")
    expect(useSettingsStore.getState().fitMode).toBe("height")
    setZoomToFit("screen")
    expect(useSettingsStore.getState().fitMode).toBe("screen")
  })
})

describe("fitMode 기억", () => {
  it("너비 맞춤을 기억해 다음 이미지도 너비에 맞춘다", () => {
    useSettingsStore.setState({ fitMode: "width" })
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/b.jpg",
      source_path: "/pics/b.jpg",
      file_name: "b.jpg",
      file_size: 456,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1000
    })
    // 너비 맞춤: 1000/2000 = 0.5 (높이는 500으로 컨테이너보다 작아도 너비 기준)
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })

  it("높이 맞춤을 기억해 다음 이미지도 높이에 맞춘다", () => {
    useSettingsStore.setState({ fitMode: "height" })
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/c.jpg",
      source_path: "/pics/c.jpg",
      file_name: "c.jpg",
      file_size: 456,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1400
    })
    // 높이 맞춤: 700/1400 = 0.5
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })

  it("화면 맞춤을 기억해 다음 이미지도 화면에 맞춘다", () => {
    useSettingsStore.setState({ fitMode: "screen" })
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/d.jpg",
      source_path: "/pics/d.jpg",
      file_name: "d.jpg",
      file_size: 456,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1000
    })
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
  })

  it("작은 이미지도 너비 맞춤이면 확대한다", () => {
    useSettingsStore.setState({ fitMode: "width" })
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/small.png",
      source_path: "/pics/small.png",
      file_name: "small.png",
      file_size: 10,
      mime_type: "image/png",
      width: 100,
      height: 100
    })
    // 너비 맞춤: 1000/100 = 10
    expect(useAppStore.getState().zoom).toBeCloseTo(10)
  })

  it("auto에서는 작은 이미지를 100%로 둔다", () => {
    useSettingsStore.setState({ fitMode: "auto" })
    useAppStore.setState({ containerSize: { width: 1000, height: 700 } })
    setImageInfoAndResetView({
      file_path: "/pics/small.png",
      source_path: "/pics/small.png",
      file_name: "small.png",
      file_size: 10,
      mime_type: "image/png",
      width: 100,
      height: 100
    })
    expect(useAppStore.getState().zoom).toBe(1)
  })

  it("0 리셋은 맞춤 잠금을 auto로 되돌린다", () => {
    useSettingsStore.setState({ fitMode: "width" })
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 0.5,
      position: { x: 0, y: 0 }
    })
    resetZoomPan()
    expect(useSettingsStore.getState().fitMode).toBe("auto")
  })

  it("applyRememberedFit은 fitMode를 바꾸지 않고 줌만 맞춘다", () => {
    useSettingsStore.setState({ fitMode: "height" })
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1400 },
      rotation: 0,
      zoom: 1,
      position: { x: 10, y: 20 }
    })
    applyRememberedFit()
    expect(useAppStore.getState().zoom).toBeCloseTo(0.5)
    expect(useAppStore.getState().position).toEqual({ x: 0, y: 0 })
    expect(useSettingsStore.getState().fitMode).toBe("height")
  })
})

describe("isFitLocked", () => {
  it("초기 상태와 closeImage 후에는 잠긴다", () => {
    expect(useAppStore.getState().isFitLocked).toBe(true)
    useAppStore.setState({ isFitLocked: false })
    closeImage()
    expect(useAppStore.getState().isFitLocked).toBe(true)
  })

  it("setZoomToFit은 잠근다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 1,
      position: { x: 0, y: 0 },
      isFitLocked: false
    })
    setZoomToFit("width")
    expect(useAppStore.getState().isFitLocked).toBe(true)
  })

  it("수동 줌(zoomInBy/zoomOutBy)은 잠금을 푼다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 0.5,
      position: { x: 0, y: 0 },
      isFitLocked: true
    })
    zoomInBy()
    expect(useAppStore.getState().isFitLocked).toBe(false)
    useAppStore.setState({ isFitLocked: true })
    zoomOutBy()
    expect(useAppStore.getState().isFitLocked).toBe(false)
  })

  it("resetZoomPan과 applyRememberedFit은 다시 잠근다", () => {
    useSettingsStore.setState({ fitMode: "width" })
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      rotation: 0,
      zoom: 2,
      position: { x: 10, y: 20 },
      isFitLocked: false
    })
    resetZoomPan()
    expect(useAppStore.getState().isFitLocked).toBe(true)
    useAppStore.setState({ isFitLocked: false })
    applyRememberedFit()
    expect(useAppStore.getState().isFitLocked).toBe(true)
  })

  it("setImageInfoAndResetView는 다음 이미지를 잠근 상태로 연다", () => {
    useAppStore.setState({
      containerSize: { width: 1000, height: 700 },
      isFitLocked: false
    })
    setImageInfoAndResetView({
      file_path: "/pics/b.jpg",
      source_path: "/pics/b.jpg",
      file_name: "b.jpg",
      file_size: 456,
      mime_type: "image/jpeg",
      width: 2000,
      height: 1000
    })
    expect(useAppStore.getState().isFitLocked).toBe(true)
  })
})
