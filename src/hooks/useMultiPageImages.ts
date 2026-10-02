import { useEffect, useState } from "react"
import { useShallow } from "zustand/react/shallow"

import { useEffectiveViewMode } from "@/hooks/useEffectiveViewMode"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { isArchiveFilePath } from "@/utils/archiveFile"
import { resolveCoverIndex } from "@/utils/comicCover"
import { dualPageIndices } from "@/utils/dirNavigation"

// viewMode에 따라 현재 이미지 외에 주변 이미지를 로드해 반환한다.
//   - single               : 빈 배열 (ImageContainer가 기본 단일 렌더를 사용)
//   - left-to-right / right-to-left : [current, next]
//   - webtoon              : 빈 배열 (WebtoonContinuousView가 전 구간 지연 로드)
//
// 실제 캐시는 useImageLoader에서 관리하는 동일한 인스턴스를 재사용하기 위해
// getOrLoadImage를 인자로 받는다.

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

export type MultiPage = {
  path: string
  info: ImageInfo
}

export function useMultiPageImages(getOrLoadImage: GetOrLoadImage) {
  const viewMode = useEffectiveViewMode()
  const { loopNavigation, showCoverAlone } = useSettingsStore(
    useShallow((s) => ({
      loopNavigation: s.loopNavigation,
      showCoverAlone: s.showCoverAlone
    }))
  )
  const dirImages = useAppStore((s) => s.dirImages)
  const comicInfo = useAppStore((s) => s.comicInfo)
  const [pages, setPages] = useState<MultiPage[]>([])

  useEffect(() => {
    if (viewMode === "single" || viewMode === "webtoon") {
      setPages([])
      return
    }

    const images = dirImages.images
    if (!images.length) {
      setPages([])
      return
    }

    const index = dirImages.current_index
    const count = images.length

    // 어떤 인덱스를 화면에 띄울지 결정 (도크 하이라이트와 같은 목록을 쓴다)
    const targetIndices = dualPageIndices(
      index,
      count,
      showCoverAlone,
      resolveCoverIndex(comicInfo, count),
      loopNavigation
    )

    // 폴더 안 아카이브(CBZ/ZIP)는 이미지로 그릴 수 없으므로 짝 페이지에서 뺀다.
    // 짝이 빠지면 현재 이미지가 단독으로 가운데 표시된다.
    const paths = targetIndices
      .map((i) => images[i])
      .filter((p): p is string => !!p && !isArchiveFilePath(p))
    let cancelled = false

    Promise.all(
      paths.map(async (p) => {
        try {
          const info = await getOrLoadImage(p)
          return { path: p, info } as MultiPage
        } catch {
          return null
        }
      })
    ).then((results) => {
      if (cancelled) return
      setPages(results.filter((r): r is MultiPage => r !== null))
    })

    return () => {
      cancelled = true
    }
  }, [
    viewMode,
    loopNavigation,
    showCoverAlone,
    comicInfo,
    dirImages.images,
    dirImages.current_index,
    getOrLoadImage
  ])

  return { viewMode, pages }
}
