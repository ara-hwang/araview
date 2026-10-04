import { useMemo } from "react"

import { useAppStore } from "@/store/appStore"
import { getSettings, useSettingsStore } from "@/store/settingsStore"
import { resolveCoverLayout, type CoverLayout } from "@/utils/comicCover"

/**
 * 호출 시점 스토어 기준의 양쪽 보기 표지 배치.
 * ComicInfo는 열린 아카이브에만 적용한다. 폴더 미리보기가 읽어 둔 메타데이터가
 * 폴더 목록의 배치를 바꾸지 않게 한다.
 */
export function getCoverLayout(): CoverLayout {
  const { comicInfo, archivePath, dirImages } = useAppStore.getState()
  return resolveCoverLayout(
    archivePath !== null ? comicInfo : null,
    dirImages.images.length,
    getSettings().showCoverAlone
  )
}

/** `getCoverLayout`의 구독 버전. 값이 같으면 같은 객체를 돌려준다. */
export function useCoverLayout(): CoverLayout {
  const comicInfo = useAppStore((state) => (state.archivePath !== null ? state.comicInfo : null))
  const total = useAppStore((state) => state.dirImages.images.length)
  const showCoverAlone = useSettingsStore((state) => state.showCoverAlone)
  return useMemo(
    () => resolveCoverLayout(comicInfo, total, showCoverAlone),
    [comicInfo, total, showCoverAlone]
  )
}
