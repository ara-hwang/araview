import { useCallback } from "react"

import { updateDirImagesIndex, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

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

      const last = dirImages.images.length - 1
      let newIndex: number
      if (direction === "prev") {
        newIndex = dirImages.current_index - step
        if (newIndex < 0) {
          if (!loopNavigation) return
          newIndex =
            ((newIndex % dirImages.images.length) + dirImages.images.length) %
            dirImages.images.length
        }
      } else {
        newIndex = dirImages.current_index + step
        if (newIndex > last) {
          if (!loopNavigation) {
            // 끝에서 멈춤 모드라도 마지막 장은 볼 수 있게 clamps
            if (dirImages.current_index === last) return
            newIndex = last
          } else {
            newIndex = newIndex % dirImages.images.length
          }
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
      const total = dirImages.images.length
      const raw = dirImages.current_index + offset
      const target = loopNavigation
        ? ((raw % total) + total) % total
        : Math.max(0, Math.min(raw, total - 1))
      await navigateToIndex(target)
    },
    [dirImages, loopNavigation, navigateToIndex]
  )

  return { navigateImage, navigateToIndex, navigateByOffset }
}
