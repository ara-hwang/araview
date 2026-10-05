import { useCallback } from "react"

import { getCoverLayout } from "@/hooks/useCoverLayout"
import { getLayoutViewMode, getSoloIndices } from "@/hooks/useEffectiveViewMode"
import { useAppStore } from "@/store/appStore"
import { getSettings } from "@/store/settingsStore"
import { isArchiveFilePath } from "@/utils/archiveFile"
import { withCoverSolo } from "@/utils/comicCover"
import {
  resolveDualStepIndex,
  resolveOffsetIndex,
  resolvePairStart,
  resolveStepIndex
} from "@/utils/dirNavigation"

type LoadImageFn = (filePath: string, options?: { refreshDirectory?: boolean }) => Promise<void>

type LoadArchiveImageFn = (archivePath: string, entryName: string) => Promise<void>

/** 항목을 로드해 치수를 알아내는 함수(`getOrLoadImage`). 결과는 쓰지 않는다. */
type ProbeImageFn = (pathOrEntry: string) => Promise<unknown>

/**
 * 이동 계산에 쓰는 현재 상태. 렌더 클로저가 아니라 호출 시점의 스토어에서 읽어,
 * 직전 이동이 그려진 직후의 입력도 새 위치에서 출발한다.
 */
function readNavigationState() {
  const { dirImages, archivePath } = useAppStore.getState()
  const settings = getSettings()
  // 폴더 아카이브 미리보기(그리기는 단일)에서도 양쪽 배치로 넘겨야 짝 정렬이 유지된다.
  const viewMode = getLayoutViewMode()
  // 양쪽 보기(ltr/rtl)는 2장씩 넘기고, 나머지는 1장씩 넘긴다.
  const isDualView = viewMode === "left-to-right" || viewMode === "right-to-left"
  const cover = getCoverLayout()
  return {
    dirImages,
    archivePath,
    isDualView,
    loopNavigation: settings.loopNavigation,
    showCoverAlone: cover.coverAlone,
    coverIndex: cover.coverIndex,
    // 단독 화면 판정은 양쪽 보기에서만 쓴다. 단일 보기에서는 목록 순회를 생략한다.
    solo: isDualView ? withCoverSolo(cover, getSoloIndices()) : undefined
  }
}

/**
 * 폴더/아카이브 이동. 목표 항목의 로드만 요청하고 current_index는 쓰지 않는다.
 * 인덱스는 로더가 그 항목을 최신 로드로 그릴 때 맞추므로, 더 새 로드에 밀린
 * 느린 로드가 끝나도 화면과 인덱스가 어긋나지 않는다.
 */
export function useDirectoryNavigation(
  loadImage: LoadImageFn,
  loadArchiveImageByIndex?: LoadArchiveImageFn,
  probeImage?: ProbeImageFn
) {
  /**
   * 양쪽 보기에서 이동할 자리의 페이지를 먼저 로드해 넓은 페이지인지 확인한다.
   * 치수는 로드해야 알 수 있어, 모르는 채로 짝을 잡으면 넓은 페이지가 짝의
   * 둘째 장으로 밀려 뒤로 넘길 때 건너뛴다. 어차피 곧 그릴 페이지라 로드는 버려지지 않는다.
   */
  const probeWidePages = useCallback(
    async (indices: readonly number[]) => {
      if (!probeImage || !getSettings().showWidePageAlone) return
      const { images } = useAppStore.getState().dirImages
      const targets = indices
        .map((index) => images[index])
        .filter((path): path is string => path !== undefined && !isArchiveFilePath(path))
      await Promise.all(targets.map((path) => probeImage(path).catch(() => {})))
    },
    [probeImage]
  )

  const loadAt = useCallback(
    async (images: readonly string[], archivePath: string | null, index: number) => {
      const target = images[index]
      if (target === undefined) return
      if (archivePath && loadArchiveImageByIndex) {
        await loadArchiveImageByIndex(archivePath, target)
        return
      }
      await loadImage(target, { refreshDirectory: false })
    },
    [loadImage, loadArchiveImageByIndex]
  )

  const navigateImage = useCallback(
    async (direction: "prev" | "next") => {
      const before = readNavigationState()
      if (before.dirImages.images.length <= 1) return
      if (before.isDualView && direction === "prev") {
        const current = before.dirImages.current_index
        await probeWidePages([current - 1, current - 2])
      }
      const nav = readNavigationState()
      const { dirImages } = nav
      if (dirImages.images.length <= 1) return

      const newIndex = nav.isDualView
        ? resolveDualStepIndex(
            dirImages.current_index,
            dirImages.images.length,
            nav.loopNavigation,
            direction,
            nav.showCoverAlone,
            nav.coverIndex,
            nav.solo,
            true
          )
        : resolveStepIndex(
            dirImages.current_index,
            dirImages.images.length,
            1,
            nav.loopNavigation,
            direction
          )
      if (newIndex === null) return

      await loadAt(dirImages.images, nav.archivePath, newIndex)
    },
    [loadAt, probeWidePages]
  )

  const navigateToIndex = useCallback(
    async (index: number) => {
      if (readNavigationState().isDualView) await probeWidePages([index])
      const nav = readNavigationState()
      const { dirImages } = nav
      if (dirImages.images.length === 0) return

      // 양쪽 모드는 쌍 중간에 착지해도 쌍 시작으로 스냅해 두 화면이 겹치지 않게 한다.
      const clamped = nav.isDualView
        ? resolvePairStart(
            index,
            dirImages.images.length,
            nav.showCoverAlone,
            nav.coverIndex,
            nav.solo,
            dirImages.current_index
          )
        : Math.max(0, Math.min(index, dirImages.images.length - 1))

      await loadAt(dirImages.images, nav.archivePath, clamped)
    },
    [loadAt, probeWidePages]
  )

  /** 점프 이동: 오프셋만큼 이동하되 루프 설정에 따라 wrap/clamp */
  const navigateByOffset = useCallback(
    async (offset: number) => {
      const { dirImages } = useAppStore.getState()
      if (dirImages.images.length === 0) return
      const target = resolveOffsetIndex(
        dirImages.current_index,
        dirImages.images.length,
        offset,
        getSettings().loopNavigation
      )
      await navigateToIndex(target)
    },
    [navigateToIndex]
  )

  return { navigateImage, navigateToIndex, navigateByOffset }
}
