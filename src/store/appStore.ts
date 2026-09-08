import { create } from "zustand"
import { DirectoryImages, ExifData, ImageInfo } from "@/types"
import {
  clampPosition,
  getFitZoomFromSizes,
  getMinZoom,
  getPositionBounds
} from "@/utils/zoomPanUtils"

type AppState = {
  theme: "system" | "light" | "dark"
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
  showExifPanel: boolean
  rotation: 0 | 90 | 180 | 270
  flipH: boolean
  flipV: boolean
  /** 아카이브 모드: 현재 열린 아카이브 파일 경로 (null이면 일반 모드) */
  archivePath: string | null
}

type AppStoreActions = {
  setTheme: (nextTheme: AppState["theme"]) => void
  setZoom: (nextZoom: AppState["zoom"]) => void
  setDirImages: (nextDirImages: AppState["dirImages"]) => void
  setImageInfo: (nextImageInfo: AppState["imageInfo"]) => void
  setError: (nextError: AppState["error"]) => void
  setLoading: (nextLoading: AppState["loading"]) => void
  setPosition: (nextPosition: AppState["position"]) => void
  setIsDragging: (nextIsDragging: AppState["isDragging"]) => void
  setContainerElement: (
    nextContainerElement: AppState["containerElement"]
  ) => void
  setImageElement: (nextImageElement: AppState["imageElement"]) => void
  setViewportSize: (nextViewportSize: AppState["viewportSize"]) => void
  setContainerSize: (nextContainerSize: AppState["containerSize"]) => void
  setImageSize: (nextImageSize: AppState["imageSize"]) => void
  setDragStart: (nextDragStart: AppState["dragStart"]) => void
  setExifData: (nextExifData: AppState["exifData"]) => void
  setShowExifPanel: (nextShowExifPanel: AppState["showExifPanel"]) => void
}

const initialApp: AppState = {
  theme: "system",
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
  showExifPanel: false,
  rotation: 0,
  flipH: false,
  flipV: false,
  archivePath: null
}

export const useAppStore = create<AppState & AppStoreActions>((set) => ({
  ...initialApp,
  setTheme: (nextTheme) => set({ theme: nextTheme }),
  setZoom: (nextZoom) => set({ zoom: nextZoom }),
  setDirImages: (nextDirImages) => set({ dirImages: nextDirImages }),
  setImageInfo: (nextImageInfo) => set({ imageInfo: nextImageInfo }),
  setError: (nextError) => set({ error: nextError }),
  setLoading: (nextLoading) => set({ loading: nextLoading }),
  setPosition: (nextPosition) => set({ position: nextPosition }),
  setIsDragging: (nextIsDragging) => set({ isDragging: nextIsDragging }),
  setContainerElement: (nextContainerElement) =>
    set({ containerElement: nextContainerElement }),
  setImageElement: (nextImageElement) =>
    set({ imageElement: nextImageElement }),
  setViewportSize: (nextViewportSize) =>
    set({ viewportSize: nextViewportSize }),
  setContainerSize: (nextContainerSize) =>
    set({ containerSize: nextContainerSize }),
  setImageSize: (nextImageSize) => set({ imageSize: nextImageSize }),
  setDragStart: (nextDragStart) => set({ dragStart: nextDragStart }),
  setExifData: (nextExifData) => set({ exifData: nextExifData }),
  setShowExifPanel: (nextShowExifPanel) =>
    set({ showExifPanel: nextShowExifPanel })
}))

export const getApp = () => useAppStore.getState()

export const getFitZoom = (): number => {
  const { containerSize, imageSize } = useAppStore.getState()
  return getFitZoomFromSizes(
    containerSize.width,
    containerSize.height,
    imageSize.width,
    imageSize.height
  )
}

export const setZoomToFit = (mode: "width" | "height" | "screen") => {
  const { containerSize, imageSize } = useAppStore.getState()
  const { width: cw, height: ch } = containerSize
  const { width: iw, height: ih } = imageSize

  if (iw <= 0 || ih <= 0) return

  const zoom =
    mode === "width"
      ? cw / iw
      : mode === "height"
        ? ch / ih
        : Math.min(cw / iw, ch / ih)

  if (!Number.isFinite(zoom) || zoom <= 0) return

  useAppStore.setState(() => ({
    zoom,
    position: { x: 0, y: 0 }
  }))
}

export const zoomInBy = (factor = 1.25) => {
  const { zoom } = useAppStore.getState()
  const next = Math.min(zoom * factor, 10)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomOutBy = (factor = 1.25) => {
  const { zoom, containerSize, imageSize } = useAppStore.getState()
  const minZoom = getMinZoom(
    containerSize.width,
    containerSize.height,
    imageSize.width,
    imageSize.height
  )
  const next = Math.max(zoom / factor, minZoom)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomIn = () => zoomInBy(1.25)
export const zoomOut = () => zoomOutBy(1.25)

export const resetZoomPan = () => {
  useAppStore.setState((s) => ({
    ...s,
    zoom: Math.min(1, getFitZoom()),
    position: { x: 0, y: 0 },
    rotation: 0,
    flipH: false,
    flipV: false
  }))
}

export const applyImageNaturalSize = (width: number, height: number) => {
  if (width <= 0 || height <= 0) return
  const prev = useAppStore.getState().imageSize
  if (prev.width === width && prev.height === height) return
  useAppStore.setState({ imageSize: { width, height } })
  resetZoomPan()
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
      current_index: Math.max(
        0,
        Math.min(nextIndex, dirImages.images.length - 1)
      )
    }
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
  const { width: iw, height: ih } = state.imageSize

  if (cw > 0 && ch > 0 && iw > 0 && ih > 0) {
    const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, state.zoom)
    const next = clampPosition(newX, newY, maxX, maxY)
    useAppStore.setState((s) => ({ ...s, position: next }))
  } else {
    useAppStore.setState((s) => ({ ...s, position: { x: newX, y: newY } }))
  }
}
