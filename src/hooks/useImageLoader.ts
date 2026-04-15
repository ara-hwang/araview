import { useCallback } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import type { DirectoryImages } from "@/types"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { useSettingsStore } from "@/store/settingsStore"
import { useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"

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
        void useRecentFilesStore.getState().add(filePath)
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
        const message = String(e)
        useAppStore.setState({ error: message })
        useAppStore.setState({ imageInfo: null })
        toast.error("이미지를 불러오지 못했습니다", {
          description: message
        })
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
      if (files.length === 0) return

      // 첫 번째 드롭 항목만 사용 (파일 또는 폴더)
      const first = files[0]
      const droppedPath = (first as { path?: string }).path
      if (!droppedPath) return

      try {
        const resolved = await invoke<string>("resolve_dropped_path", {
          path: droppedPath
        })
        await loadImage(resolved)
      } catch (err) {
        toast.error("드롭한 항목을 열 수 없습니다", {
          description: String(err)
        })
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
    handleDragOver,
    getOrLoadImage
  }
}
