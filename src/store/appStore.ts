import { create } from "zustand"

import { updateSettings, useSettingsStore } from "@/store/settingsStore"
import { DirectoryImages, ExifData, ImageDetails, ImageHistogram, ImageInfo } from "@/types"
import {
  clampPosition,
  getFitZoomFromSizes,
  getMinZoom,
  getOrientedImageSize,
  getPositionBounds,
  getZoomForFitMode
} from "@/utils/zoomPanUtils"

type AppState = {
  zoom: number
  dirImages: DirectoryImages
  imageInfo: ImageInfo | null
  error: string | null
  loading: boolean
  position: { x: number; y: number }
  isDragging: boolean
  containerElement: HTMLDivElement | null
  imageElement: HTMLImageElement | null
  viewportSize: { width: number; height: number }
  containerSize: { width: number; height: number }
  imageSize: { width: number; height: number }
  dragStart: { x: number; y: number }
  exifData: ExifData | null
  exifError: string | null
  histogramData: ImageHistogram | null
  imageDetails: ImageDetails | null
  showExifPanel: boolean
  rotation: 0 | 90 | 180 | 270
  flipH: boolean
  flipV: boolean
  /** 아카이브 모드: 현재 열린 아카이브 파일 경로 (null이면 일반 모드) */
  archivePath: string | null
  /** 이번 세션에 로드 실패한 경로/엔트리 (썸네일 오류 배지용, 영속화하지 않음) */
  failedPaths: string[]
}

type AppStoreActions = {
  setZoom: (nextZoom: AppState["zoom"]) => void
  setDirImages: (nextDirImages: AppState["dirImages"]) => void
  setImageInfo: (nextImageInfo: AppState["imageInfo"]) => void
  setError: (nextError: AppState["error"]) => void
  setLoading: (nextLoading: AppState["loading"]) => void
  setPosition: (nextPosition: AppState["position"]) => void
  setIsDragging: (nextIsDragging: AppState["isDragging"]) => void
  setContainerElement: (nextContainerElement: AppState["containerElement"]) => void
  setImageElement: (nextImageElement: AppState["imageElement"]) => void
  setViewportSize: (nextViewportSize: AppState["viewportSize"]) => void
  setContainerSize: (nextContainerSize: AppState["containerSize"]) => void
  setImageSize: (nextImageSize: AppState["imageSize"]) => void
  setDragStart: (nextDragStart: AppState["dragStart"]) => void
  setExifData: (nextExifData: AppState["exifData"]) => void
  setExifError: (nextExifError: AppState["exifError"]) => void
  setHistogramData: (nextHistogramData: AppState["histogramData"]) => void
  setImageDetails: (nextImageDetails: AppState["imageDetails"]) => void
  setShowExifPanel: (nextShowExifPanel: AppState["showExifPanel"]) => void
  addFailedPath: (filePath: string) => void
  removeFailedPath: (filePath: string) => void
  clearFailedPaths: () => void
}

const initialApp: AppState = {
  zoom: 1,
  imageInfo: null,
  dirImages: {
    images: [],
    current_index: 0
  },
  error: null,
  loading: false,
  position: { x: 0, y: 0 },
  isDragging: false,
  containerElement: null,
  imageElement: null,
  viewportSize: { width: 0, height: 0 },
  containerSize: { width: 0, height: 0 },
  imageSize: { width: 0, height: 0 },
  dragStart: { x: 0, y: 0 },
  exifData: null,
  exifError: null,
  histogramData: null,
  imageDetails: null,
  showExifPanel: false,
  rotation: 0,
  flipH: false,
  flipV: false,
  archivePath: null,
  failedPaths: []
}

export const useAppStore = create<AppState & AppStoreActions>((set) => ({
  ...initialApp,
  setZoom: (nextZoom) => set({ zoom: nextZoom }),
  setDirImages: (nextDirImages) => set({ dirImages: nextDirImages }),
  setImageInfo: (nextImageInfo) => set({ imageInfo: nextImageInfo }),
  setError: (nextError) => set({ error: nextError }),
  setLoading: (nextLoading) => set({ loading: nextLoading }),
  setPosition: (nextPosition) => set({ position: nextPosition }),
  setIsDragging: (nextIsDragging) => set({ isDragging: nextIsDragging }),
  setContainerElement: (nextContainerElement) => set({ containerElement: nextContainerElement }),
  setImageElement: (nextImageElement) => set({ imageElement: nextImageElement }),
  setViewportSize: (nextViewportSize) => set({ viewportSize: nextViewportSize }),
  setContainerSize: (nextContainerSize) => set({ containerSize: nextContainerSize }),
  setImageSize: (nextImageSize) => set({ imageSize: nextImageSize }),
  setDragStart: (nextDragStart) => set({ dragStart: nextDragStart }),
  setExifData: (nextExifData) => set({ exifData: nextExifData }),
  setExifError: (nextExifError) => set({ exifError: nextExifError }),
  setHistogramData: (nextHistogramData) => set({ histogramData: nextHistogramData }),
  setImageDetails: (nextImageDetails) => set({ imageDetails: nextImageDetails }),
  setShowExifPanel: (nextShowExifPanel) => set({ showExifPanel: nextShowExifPanel }),
  addFailedPath: (filePath) =>
    set((s) =>
      s.failedPaths.includes(filePath) ? s : { failedPaths: [...s.failedPaths, filePath] }
    ),
  removeFailedPath: (filePath) =>
    set((s) => ({ failedPaths: s.failedPaths.filter((p) => p !== filePath) })),
  clearFailedPaths: () => set({ failedPaths: [] })
}))

export const getApp = () => useAppStore.getState()

export const getFitZoom = (): number => {
  const { containerSize, imageSize, rotation } = useAppStore.getState()
  const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
  return getFitZoomFromSizes(
    containerSize.width,
    containerSize.height,
    oriented.width,
    oriented.height
  )
}

export const setZoomToFit = (mode: "width" | "height" | "screen") => {
  const { containerSize, imageSize, rotation } = useAppStore.getState()
  const { width: cw, height: ch } = containerSize
  const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
  const { width: iw, height: ih } = oriented

  if (iw <= 0 || ih <= 0) return
  if (cw <= 0 || ch <= 0) return

  const zoom = getZoomForFitMode(mode, cw, ch, iw, ih)

  if (!Number.isFinite(zoom) || zoom <= 0) return

  useAppStore.setState(() => ({
    zoom,
    position: { x: 0, y: 0 }
  }))
  // 선택한 맞춤 모드를 기억해 다음 이미지와 재시작 후에도 유지한다.
  void updateSettings({ fitMode: mode })
}

export const zoomInBy = (factor = 1.25) => {
  const { zoom } = useAppStore.getState()
  const next = Math.min(zoom * factor, 10)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomOutBy = (factor = 1.25) => {
  const { zoom, containerSize, imageSize, rotation } = useAppStore.getState()
  const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
  const minZoom = getMinZoom(
    containerSize.width,
    containerSize.height,
    oriented.width,
    oriented.height
  )
  const next = Math.max(zoom / factor, minZoom)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomIn = () => zoomInBy(1.25)
export const zoomOut = () => zoomOutBy(1.25)

export const resetZoomPan = () => {
  const { containerSize, imageSize, rotation } = useAppStore.getState()
  const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
  const zoom = getZoomForFitMode(
    "auto",
    containerSize.width,
    containerSize.height,
    oriented.width,
    oriented.height
  )
  useAppStore.setState((s) => ({
    ...s,
    zoom,
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false
  }))
  // 0(실제 크기/자동 맞춤)은 맞춤 잠금을 해제하고 기억한다.
  void updateSettings({ fitMode: "auto" })
}

/**
 * 기억된 맞춤 모드를 현재 이미지에 다시 적용한다.
 * 이미지 전환(onAfterLoad/onLoad)용으로, fitMode 자체는 바꾸지 않는다.
 */
export const applyRememberedFit = () => {
  const { containerSize, imageSize, rotation } = useAppStore.getState()
  const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
  const fitMode = useSettingsStore.getState().fitMode
  const zoom = getZoomForFitMode(
    fitMode,
    containerSize.width,
    containerSize.height,
    oriented.width,
    oriented.height
  )
  useAppStore.setState((s) => ({
    ...s,
    zoom,
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false
  }))
}

/**
 * 새 파일로 전환할 때 이미지 메타와 뷰(줌/팬/회전/반전)를 원자적으로 교체한다.
 * 백엔드 치수로 기억된 맞춤 모드의 줌을 즉시 계산하므로, 이전 파일의 줌에서 새 줌으로
 * 보간되는 전환 애니메이션 없이 첫 페인트부터 올바른 배율로 표시된다.
 * 치수를 모르면 1로 두고 onLoad의 applyImageNaturalSize가 보정한다.
 */
export const setImageInfoAndResetView = (imgInfo: ImageInfo) => {
  const { containerSize } = useAppStore.getState()
  const fitMode = useSettingsStore.getState().fitMode
  const w = imgInfo.width ?? 0
  const h = imgInfo.height ?? 0
  if (w > 0 && h > 0) {
    const computed = getZoomForFitMode(fitMode, containerSize.width, containerSize.height, w, h)
    const finalZoom = Number.isFinite(computed) && computed > 0 ? computed : 1
    useAppStore.setState({
      imageInfo: imgInfo,
      imageSize: { width: w, height: h },
      zoom: finalZoom,
      position: { x: 0, y: 0 },
      rotation: 0,
      flipH: false,
      flipV: false,
      error: null
    })
    return
  }
  useAppStore.setState({
    imageInfo: imgInfo,
    // 치수 미상: 이전 파일 크기가 남지 않게 비우고 onLoad 보정에 맡긴다.
    imageSize: { width: 0, height: 0 },
    zoom: 1,
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false,
    error: null
  })
}

export const applyImageNaturalSize = (width: number, height: number) => {
  if (width <= 0 || height <= 0) return
  const prev = useAppStore.getState().imageSize
  if (prev.width === width && prev.height === height) return
  useAppStore.setState({ imageSize: { width, height } })
  // onLoad 보정도 기억된 맞춤 모드를 따른다. fitMode 자체는 바꾸지 않는다.
  applyRememberedFit()
}

export const rotateCW = () => {
  useAppStore.setState((s) => {
    const next = ((s.rotation + 90) % 360) as AppState["rotation"]
    return { rotation: next }
  })
}

export const rotateCCW = () => {
  useAppStore.setState((s) => {
    const next = ((s.rotation + 270) % 360) as AppState["rotation"]
    return { rotation: next }
  })
}

export const flipHorizontal = () => {
  useAppStore.setState((s) => ({ flipH: !s.flipH }))
}

export const flipVertical = () => {
  useAppStore.setState((s) => ({ flipV: !s.flipV }))
}

export const updateDirImagesIndex = (nextIndex: number) => {
  const { dirImages } = useAppStore.getState()
  if (!dirImages?.images?.length) return

  useAppStore.setState({
    dirImages: {
      ...dirImages,
      current_index: Math.max(0, Math.min(nextIndex, dirImages.images.length - 1))
    }
  })
}

/** 열린 이미지를 닫고 뷰어 상태를 초기값으로 되돌린다 (홈 귀환용) */
export const closeImage = () => {
  useAppStore.setState({
    ...initialApp,
    dirImages: { images: [], current_index: 0 },
    position: { x: 0, y: 0 },
    viewportSize: { width: 0, height: 0 },
    containerSize: { width: 0, height: 0 },
    imageSize: { width: 0, height: 0 },
    dragStart: { x: 0, y: 0 }
  })
}

export const startDrag = (clientX: number, clientY: number) => {
  const { position } = useAppStore.getState()
  useAppStore.setState({ isDragging: true })
  useAppStore.setState({
    dragStart: { x: clientX - position.x, y: clientY - position.y }
  })
}

export const moveDrag = (clientX: number, clientY: number) => {
  const state = useAppStore.getState()
  if (!state.isDragging) return
  const newX = clientX - state.dragStart.x
  const newY = clientY - state.dragStart.y

  const { width: cw, height: ch } = state.containerSize
  const oriented = getOrientedImageSize(
    state.imageSize.width,
    state.imageSize.height,
    state.rotation
  )
  const { width: iw, height: ih } = oriented

  if (cw > 0 && ch > 0 && iw > 0 && ih > 0) {
    const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, state.zoom)
    const next = clampPosition(newX, newY, maxX, maxY)
    useAppStore.setState((s) => ({ ...s, position: next }))
  } else {
    useAppStore.setState((s) => ({ ...s, position: { x: newX, y: newY } }))
  }
}

export const PAN_STEP_PX = 48

/** 키보드 팬: 항상 동작, 컨테이너 경계로 clamp */
export const panBy = (dx: number, dy: number) => {
  const state = useAppStore.getState()
  if (!state.imageInfo) return
  const newX = state.position.x + dx
  const newY = state.position.y + dy
  const { width: cw, height: ch } = state.containerSize
  const oriented = getOrientedImageSize(
    state.imageSize.width,
    state.imageSize.height,
    state.rotation
  )
  const { width: iw, height: ih } = oriented
  if (cw > 0 && ch > 0 && iw > 0 && ih > 0) {
    const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, state.zoom)
    useAppStore.setState((s) => ({
      ...s,
      position: clampPosition(newX, newY, maxX, maxY)
    }))
  } else {
    useAppStore.setState((s) => ({ ...s, position: { x: newX, y: newY } }))
  }
}

export const panLeft = () => panBy(PAN_STEP_PX, 0)
export const panRight = () => panBy(-PAN_STEP_PX, 0)
export const panUp = () => panBy(0, PAN_STEP_PX)
export const panDown = () => panBy(0, -PAN_STEP_PX)
