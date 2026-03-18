import { create } from "zustand"
import { DirectoryImages, ImageInfo } from "@/types"
import { clampPosition, getPositionBounds } from "@/utils/zoomPanUtils"

type App = {
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
}

const initialApp: App = {
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
  dragStart: { x: 0, y: 0 }
}

export const useAppStore = create<App>(() => initialApp)

export const getApp = () => useAppStore.getState()

export const updateApp = async (partial: Partial<App>) => {
  const next = {
    ...useAppStore.getState(),
    ...partial
  }

  useAppStore.setState(next)
}

export const getFitZoom = (): number => {
  const { containerSize, imageSize } = useAppStore.getState()
  const { width: cw, height: ch } = containerSize
  const { width: iw, height: ih } = imageSize
  if (iw <= 0 || ih <= 0) return 1
  const fit = Math.min(cw / iw, ch / ih)
  return fit * (1 + 1e-6)
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

export const fitToWidth = () => setZoomToFit("width")
export const fitToHeight = () => setZoomToFit("height")
export const fitToScreen = () => setZoomToFit("screen")

export const zoomInBy = (factor = 1.25) => {
  const { zoom } = useAppStore.getState()
  const next = Math.min(zoom * factor, 10)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomOutBy = (factor = 1.25) => {
  const minZoom = getFitZoom()
  const { zoom } = useAppStore.getState()
  const next = Math.max(zoom / factor, minZoom)
  useAppStore.setState((s) => ({ ...s, zoom: next }))
}

export const zoomIn = () => zoomInBy(1.25)
export const zoomOut = () => zoomOutBy(1.25)

export const resetZoomPan = () => {
  useAppStore.setState((s) => ({ ...s, zoom: 1, position: { x: 0, y: 0 } }))
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

export const stopDrag = () => {
  useAppStore.setState({ isDragging: false })
}
