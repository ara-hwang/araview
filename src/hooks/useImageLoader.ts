import { useCallback } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import type { DirectoryImages } from "@/types"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { useSettingsStore } from "@/store/settingsStore"
import { useAppStore } from "@/store/appStore"

type LoadImageOptions = {
  refreshDirectory?: boolean
  onAfterLoad?: () => void
}

export function useImageLoader() {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance } =
    useImageCache()

  const loadImage = useCallback(
    async (filePath: string, options?: LoadImageOptions) => {
      const refreshDirectory = options?.refreshDirectory ?? true

      useAppStore.setState({ loading: true })
      try {
        const imgInfo = await getOrLoadImage(filePath)
        useAppStore.setState({ imageInfo: imgInfo })
        options?.onAfterLoad?.()

        let resolvedDirInfo = dirImages

        if (
          refreshDirectory ||
          !resolvedDirInfo ||
          !resolvedDirInfo.images.includes(filePath)
        ) {
          resolvedDirInfo = await invoke<DirectoryImages>(
            "get_directory_images",
            { filePath }
          )
          useAppStore.setState({ dirImages: resolvedDirInfo })
        }

        if (resolvedDirInfo) {
          const resolvedIndex = resolvedDirInfo.images.indexOf(filePath)
          const nextIndex =
            resolvedIndex >= 0 ? resolvedIndex : resolvedDirInfo.current_index
          prefetchNearbyImages(
            resolvedDirInfo.images,
            nextIndex,
            loopNavigation,
            getPrefetchDistance()
          )
        }
      } catch (e) {
        useAppStore.setState({ error: String(e) })
        useAppStore.setState({ imageInfo: null })
      } finally {
        useAppStore.setState({ loading: false })
      }
    },
    [
      dirImages,
      getOrLoadImage,
      getPrefetchDistance,
      prefetchNearbyImages,
      loopNavigation
    ]
  )

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: [...SUPPORTED_IMAGE_EXTENSIONS]
        }
      ]
    })
    if (selected) await loadImage(selected)
  }, [loadImage])

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const files = e.dataTransfer.files
      if (files.length > 0) {
        const file = files[0]
        const filePath = (file as { path?: string }).path
        if (filePath) await loadImage(filePath)
      }
    },
    [loadImage]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])

  return {
    loadImage,
    handleOpenFile,
    handleDrop,
    handleDragOver
  }
}
