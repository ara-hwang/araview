import { useMemo } from "react"

import { useCoverLayout } from "@/hooks/useCoverLayout"
import { useLayoutViewMode, useSoloIndices } from "@/hooks/useEffectiveViewMode"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { withCoverSolo } from "@/utils/comicCover"
import { dualPageIndices } from "@/utils/dirNavigation"

/**
 * 지금 화면에 떠 있는 이미지 인덱스 집합.
 * 양쪽 보기는 두 장이 함께 떠 있으므로 둘 다 담고, 단일/웹툰/단독 화면(표지,
 * 표지 바로 앞 페이지, 마지막 홀수 장)은 한 장만 담는다.
 * 도크와 썸네일 그리드가 화면 상태를 같은 기준으로 판단하도록 한 곳에 모아 둔다.
 */
export function useCurrentPageIndices(): Set<number> {
  const currentIndex = useAppStore((state) => state.dirImages.current_index)
  const total = useAppStore((state) => state.dirImages.images.length)
  const viewMode = useLayoutViewMode()
  const solo = useSoloIndices()
  const cover = useCoverLayout()
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

  return useMemo(() => {
    if (viewMode !== "left-to-right" && viewMode !== "right-to-left") {
      return new Set([currentIndex])
    }
    return new Set(
      dualPageIndices(
        currentIndex,
        total,
        cover.coverAlone,
        cover.coverIndex,
        loopNavigation,
        withCoverSolo(cover, solo)
      )
    )
  }, [viewMode, currentIndex, total, cover, loopNavigation, solo])
}
