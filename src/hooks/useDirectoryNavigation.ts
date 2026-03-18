import { useCallback } from "react"
import { useSettingsStore } from "@/store/settingsStore"
import { updateDirImagesIndex, useAppStore } from "@/store/appStore"

type LoadImageFn = (
  filePath: string,
  options?: { refreshDirectory?: boolean }
) => Promise<void>

export function useDirectoryNavigation(loadImage: LoadImageFn) {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

  // 이전 또는 다음 이미지로 이동
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

      await loadImage(dirImages.images[newIndex], { refreshDirectory: false })
      updateDirImagesIndex(newIndex)
    },
    [dirImages, loadImage, loopNavigation]
  )

  // 원하는 인덱스로 이동
  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!dirImages || dirImages.images.length === 0) return

      // 인덱스를 0과 이미지 개수 - 1 사이로 제한
      const clamped = Math.max(0, Math.min(index, dirImages.images.length - 1))

      await loadImage(dirImages.images[clamped], { refreshDirectory: false })
      updateDirImagesIndex(clamped)
    },
    [dirImages, loadImage]
  )

  return { navigateImage, navigateToIndex }
}
