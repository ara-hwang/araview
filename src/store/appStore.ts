import { create } from "zustand"

import { updateSettings, useSettingsStore, type ViewMode } from "@/store/settingsStore"
import {
  ComicInfo,
  DirectoryImages,
  ExifData,
  ImageDetails,
  ImageHistogram,
  ImageInfo
} from "@/types"
import { isSvgImageInfo } from "@/utils/imageRendering"
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
  /** 아카이브(만화) 모드에서 읽은 ComicInfo.xml 메타데이터. 없으면 null */
  comicInfo: ComicInfo | null
  /** ComicInfo.xml 파싱 실패 메시지 (부재와 구분용) */
  comicInfoError: string | null
  /**
   * 만화 자동 양쪽 보기가 정한 보기 모드. null이면 `settings.viewMode`를 그대로 쓴다.
   * 보기 모드를 직접 바꾸면 null로 돌아간다. 영속화하지 않음.
   */
  comicViewMode: ViewMode | null
  histogramData: ImageHistogram | null
  imageDetails: ImageDetails | null
  showExifPanel: boolean
  rotation: 0 | 90 | 180 | 270
  flipH: boolean
  flipV: boolean
  /**
   * 핏 잠금. true면 윈도우 리사이즈(컨테이너 변화) 시 기억된 fitMode를 다시 적용하고,
   * 수동 줌(zoomInBy/zoomOutBy) 시 false로 풀려 리사이즈해도 줌을 유지한다. 영속화하지 않음.
   */
  isFitLocked: boolean
  /** 아카이브 모드: 현재 열린 아카이브 파일 경로 (null이면 일반 모드) */
  archivePath: string | null
  /**
   * 폴더 탐색 중 아카이브 미리보기. `archivePath`는 null이고 폴더 `dirImages`는 유지된다.
   * 첫 페이지만 표시하며, 「만화 열기」 시 `archivePath` 모드로 전환한다.
   */
  archivePreviewPath: string | null
  /** 이번 세션에 로드 실패한 경로/엔트리 (썸네일 오류 배지용, 영속화하지 않음) */
  failedPaths: string[]
  /**
   * 캐시된 저해상 썸네일 경로. 큰 이미지가 풀사이즈로 디코드되는 동안
   * 먼저 깔아 보여주는 프리뷰이며, 원본 로드(onLoad)에서 지운다.
   */
  previewPath: string | null
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
    current_index: 0,
    availability: []
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
  comicInfo: null,
  comicInfoError: null,
  comicViewMode: null,
  histogramData: null,
  imageDetails: null,
  showExifPanel: false,
  rotation: 0,
  flipH: false,
  flipV: false,
  isFitLocked: true,
  archivePath: null,
  archivePreviewPath: null,
  failedPaths: [],
  previewPath: null
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
    position: { x: 0, y: 0 },
    isFitLocked: true
  }))
  // 선택한 맞춤 모드를 기억해 다음 이미지와 재시작 후에도 유지한다.
  void updateSettings({ fitMode: mode })
}

export const MAX_ZOOM = 10
/** 벡터(SVG)는 표시 크기에서 재래스터되므로 증분 확대 상한을 높인다. */
export const MAX_ZOOM_VECTOR = 40

const isVectorImage = (): boolean => isSvgImageInfo(useAppStore.getState().imageInfo)

export const zoomInBy = (factor = 1.25) => {
  const { zoom } = useAppStore.getState()
  const next = Math.min(zoom * factor, isVectorImage() ? MAX_ZOOM_VECTOR : MAX_ZOOM)
  useAppStore.setState({ zoom: next, isFitLocked: false })
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
  useAppStore.setState({ zoom: next, isFitLocked: false })
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
  useAppStore.setState({
    zoom,
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false,
    isFitLocked: true
  })
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
  useAppStore.setState({
    zoom,
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false,
    isFitLocked: true
  })
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
      isFitLocked: true,
      error: null,
      previewPath: null
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
    isFitLocked: true,
    error: null,
    previewPath: null
  })
}

export const applyImageNaturalSize = (width: number, height: number) => {
  if (width <= 0 || height <= 0) return
  const prev = useAppStore.getState().imageSize
  if (prev.width === width && prev.height === height) return
  // SVG는 백엔드가 헤더(viewBox 등)에서 치수를 복원한다. 브라우저 natural은
  // viewBox-only처럼 고유 크기가 없을 때 기본값(300x150 계열)이라 종횡비만
  // 같으면 백엔드 값을 유지해 첫 페인트 보정 깜빡임을 막는다.
  const info = useAppStore.getState().imageInfo
  const isSvg = isSvgImageInfo(info)
  if (
    isSvg &&
    typeof info?.width === "number" &&
    typeof info?.height === "number" &&
    info.width > 0 &&
    info.height > 0 &&
    prev.width === info.width &&
    prev.height === info.height &&
    Math.abs(width / height - info.width / info.height) < 0.01
  ) {
    return
  }
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
  useAppStore.setState({ ...initialApp })
}

export const startDrag = (clientX: number, clientY: number) => {
  const { position } = useAppStore.getState()
  useAppStore.setState({
    isDragging: true,
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
    useAppStore.setState({ position: next })
  } else {
    useAppStore.setState({ position: { x: newX, y: newY } })
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
    useAppStore.setState({
      position: clampPosition(newX, newY, maxX, maxY)
    })
  } else {
    useAppStore.setState({ position: { x: newX, y: newY } })
  }
}

export const panLeft = () => panBy(PAN_STEP_PX, 0)
export const panRight = () => panBy(-PAN_STEP_PX, 0)
export const panUp = () => panBy(0, PAN_STEP_PX)
export const panDown = () => panBy(0, -PAN_STEP_PX)

/** 사용자가 보기 모드를 직접 고른다. 만화 자동 양쪽 보기 결정은 해제된다. */
export const applyManualViewMode = (viewMode: ViewMode) => {
  useAppStore.setState({ comicViewMode: null })
  void updateSettings({ viewMode })
}
