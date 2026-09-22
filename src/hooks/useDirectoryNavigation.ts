import { useCallback } from "react"

import { updateDirImagesIndex, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { resolveOffsetIndex, resolveStepIndex } from "@/utils/dirNavigation"

type LoadImageFn = (filePath: string, options?: { refreshDirectory?: boolean }) => Promise<void>

type LoadArchiveImageFn = (archivePath: string, entryName: string) => Promise<void>

export function useDirectoryNavigation(
  loadImage: LoadImageFn,
  loadArchiveImageByIndex?: LoadArchiveImageFn
) {
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  const viewMode = useSettingsStore((state) => state.viewMode)

  // 양면 보기(ltr/rtl)는 2장씩 넘기고, 나머지는 1장씩 넘긴다.
  const step = viewMode === "left-to-right" || viewMode === "right-to-left" ? 2 : 1

  const navigateImage = useCallback(
    async (direction: "prev" | "next") => {
      if (!dirImages || dirImages.images.length <= 1) return

      const newIndex = resolveStepIndex(
        dirImages.current_index,
        dirImages.images.length,
        step,
        loopNavigation,
        direction
      )
      if (newIndex === null) return

      if (archivePath && loadArchiveImageByIndex) {
        await loadArchiveImageByIndex(archivePath, dirImages.images[newIndex])
        updateDirImagesIndex(newIndex)
        return
      }

      await loadImage(dirImages.images[newIndex], { refreshDirectory: false })
      updateDirImagesIndex(newIndex)
    },
    [dirImages, loadImage, loopNavigation, archivePath, loadArchiveImageByIndex, step]
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

  /** 점프 이동: 오프셋만큼 이동하되 루프 설정에 따라 wrap/clamp */
  const navigateByOffset = useCallback(
    async (offset: number) => {
      if (!dirImages || dirImages.images.length === 0) return
      const target = resolveOffsetIndex(
        dirImages.current_index,
        dirImages.images.length,
        offset,
        loopNavigation
      )
      await navigateToIndex(target)
    },
    [dirImages, loopNavigation, navigateToIndex]
  )

  return { navigateImage, navigateToIndex, navigateByOffset }
}
