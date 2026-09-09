import { useCallback } from "react"
import { useSettingsStore } from "@/store/settingsStore"
import { updateDirImagesIndex, useAppStore } from "@/store/appStore"

type LoadImageFn = (
  filePath: string,
  options?: { refreshDirectory?: boolean }
) => Promise<void>

type LoadArchiveImageFn = (
  archivePath: string,
  entryName: string
) => Promise<void>

export function useDirectoryNavigation(
  loadImage: LoadImageFn,
  loadArchiveImageByIndex?: LoadArchiveImageFn
) {
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

  const navigateImage = useCallback(
    async (direction: "prev" | "next") => {
      if (!dirImages || dirImages.images.length <= 1) return

      let newIndex: number
      if (direction === "prev") {
        if (dirImages.current_index === 0) {
          if (!loopNavigation) return
          newIndex = dirImages.images.length - 1
        } else {
          newIndex = dirImages.current_index - 1
        }
      } else {
        if (dirImages.current_index === dirImages.images.length - 1) {
          if (!loopNavigation) return
          newIndex = 0
        } else {
          newIndex = dirImages.current_index + 1
        }
      }

      if (archivePath && loadArchiveImageByIndex) {
        await loadArchiveImageByIndex(archivePath, dirImages.images[newIndex])
        updateDirImagesIndex(newIndex)
        return
      }

      await loadImage(dirImages.images[newIndex], { refreshDirectory: false })
      updateDirImagesIndex(newIndex)
    },
    [dirImages, loadImage, loopNavigation, archivePath, loadArchiveImageByIndex]
  )

  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!dirImages || dirImages.images.length === 0) return

      const clamped = Math.max(0, Math.min(index, dirImages.images.length - 1))

      if (archivePath && loadArchiveImageByIndex) {
        await loadArchiveImageByIndex(archivePath, dirImages.images[clamped])
        updateDirImagesIndex(clamped)
        return
      }

      await loadImage(dirImages.images[clamped], { refreshDirectory: false })
      updateDirImagesIndex(clamped)
    },
    [dirImages, loadImage, archivePath, loadArchiveImageByIndex]
  )

  return { navigateImage, navigateToIndex }
}
