import { create } from "zustand"
import { DirectoryImages, ImageInfo } from "@/types"

type App = {
  theme: "system" | "light" | "dark"
  zoom: number
  dirImages: DirectoryImages
  imageInfo: ImageInfo | null
  error: string | null
  loading: boolean
  position: { x: number; y: number }
  isDragging: boolean
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
  isDragging: false
}

export const useAppStore = create<App>(() => initialApp)

export const getApp = () => useAppStore.getState()

export const subscribeToApp = (listener: (app: App) => void) =>
  useAppStore.subscribe(listener)

export const updateApp = async (partial: Partial<App>) => {
  const next = {
    ...useAppStore.getState(),
    ...partial
  }

  useAppStore.setState(next)
}

export const resetAppState = () => {
  useAppStore.setState(initialApp)
}

export const setLoading = (loading: boolean) => {
  useAppStore.setState((state) => ({
    ...state,
    loading,
    ...(loading ? { error: null } : {})
  }))
}

export const setError = (error: string | null) => {
  useAppStore.setState((state) => ({
    ...state,
    error
  }))
}

export const setImageInfo = (imageInfo: ImageInfo | null) => {
  useAppStore.setState((state) => ({
    ...state,
    imageInfo
  }))
}

export const setDirImages = (dirImages: DirectoryImages) => {
  useAppStore.setState((state) => ({
    ...state,
    dirImages
  }))
}

export const setTheme = (theme: App["theme"]) => {
  useAppStore.setState((state) => ({
    ...state,
    theme
  }))
}

export const setZoom = (zoom: number) => {
  useAppStore.setState((state) => ({
    ...state,
    zoom
  }))
}

export const setPosition = (position: { x: number; y: number }) => {
  useAppStore.setState((state) => ({
    ...state,
    position
  }))
}

export const setDragging = (isDragging: boolean) => {
  useAppStore.setState((state) => ({
    ...state,
    isDragging
  }))
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
