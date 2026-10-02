import { useCallback } from "react"

import { useLayoutViewMode, useSoloIndices } from "@/hooks/useEffectiveViewMode"
import { updateDirImagesIndex, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { resolveCoverIndex } from "@/utils/comicCover"
import {
  resolveDualStepIndex,
  resolveOffsetIndex,
  resolvePairStart,
  resolveStepIndex
} from "@/utils/dirNavigation"

type LoadImageFn = (filePath: string, options?: { refreshDirectory?: boolean }) => Promise<void>

type LoadArchiveImageFn = (archivePath: string, entryName: string) => Promise<void>

export function useDirectoryNavigation(
  loadImage: LoadImageFn,
  loadArchiveImageByIndex?: LoadArchiveImageFn
) {
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  // 폴더 아카이브 미리보기(그리기는 단일)에서도 양쪽 배치로 넘겨야 짝 정렬이 유지된다.
  const viewMode = useLayoutViewMode()
  const solo = useSoloIndices()
  const showCoverAlone = useSettingsStore((state) => state.showCoverAlone)
  const comicInfo = useAppStore((state) => state.comicInfo)
  const coverIndex = resolveCoverIndex(comicInfo, dirImages?.images.length ?? 0)

  // 양쪽 보기(ltr/rtl)는 2장씩 넘기고, 나머지는 1장씩 넘긴다.
  const isDualView = viewMode === "left-to-right" || viewMode === "right-to-left"

  const navigateImage = useCallback(
    async (direction: "prev" | "next") => {
      if (!dirImages || dirImages.images.length <= 1) return

      const newIndex = isDualView
        ? resolveDualStepIndex(
            dirImages.current_index,
            dirImages.images.length,
            loopNavigation,
            direction,
            showCoverAlone,
            coverIndex,
            solo
          )
        : resolveStepIndex(
            dirImages.current_index,
            dirImages.images.length,
            1,
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
    [
      dirImages,
      loadImage,
      loopNavigation,
      archivePath,
      loadArchiveImageByIndex,
      isDualView,
      showCoverAlone,
      coverIndex,
      solo
    ]
  )

  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!dirImages || dirImages.images.length === 0) return

      // 양쪽 모드는 쌍 중간에 착지해도 쌍 시작으로 스냅해 두 화면이 겹치지 않게 한다.
      const clamped = isDualView
        ? resolvePairStart(index, dirImages.images.length, showCoverAlone, coverIndex, solo)
        : Math.max(0, Math.min(index, dirImages.images.length - 1))

      if (archivePath && loadArchiveImageByIndex) {
        await loadArchiveImageByIndex(archivePath, dirImages.images[clamped])
        updateDirImagesIndex(clamped)
        return
      }

      await loadImage(dirImages.images[clamped], { refreshDirectory: false })
      updateDirImagesIndex(clamped)
    },
    [
      dirImages,
      loadImage,
      archivePath,
      loadArchiveImageByIndex,
      isDualView,
      showCoverAlone,
      coverIndex,
      solo
    ]
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
