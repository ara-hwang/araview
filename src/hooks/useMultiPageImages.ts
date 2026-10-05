import { useEffect, useState } from "react"

import { useCoverLayout } from "@/hooks/useCoverLayout"
import { useEffectiveViewMode, useSoloIndices } from "@/hooks/useEffectiveViewMode"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { isArchiveFilePath } from "@/utils/archiveFile"
import { withCoverSolo } from "@/utils/comicCover"
import { dualPageIndices } from "@/utils/dirNavigation"

// viewMode에 따라 현재 이미지 외에 주변 이미지를 로드해 반환한다.
//   - single               : 빈 배열 (ImageContainer가 기본 단일 렌더를 사용)
//   - left-to-right / right-to-left : [current, next]
//   - webtoon              : 빈 배열 (WebtoonContinuousView가 전 구간 지연 로드)
//
// 실제 캐시는 useImageLoader에서 관리하는 동일한 인스턴스를 재사용하기 위해
// getOrLoadImage를 인자로 받는다.

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>
type AwaitPaintReady = (info: ImageInfo, filePath: string) => Promise<void>

export type MultiPage = {
  path: string
  info: ImageInfo
}

export function useMultiPageImages(
  getOrLoadImage: GetOrLoadImage,
  awaitPaintReady?: AwaitPaintReady
) {
  const viewMode = useEffectiveViewMode()
  const solo = useSoloIndices()
  const cover = useCoverLayout()
  const loopNavigation = useSettingsStore((s) => s.loopNavigation)
  const dirImages = useAppStore((s) => s.dirImages)
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
      cover.coverAlone,
      cover.coverIndex,
      loopNavigation,
      withCoverSolo(cover, solo),
      true
    )

    // 폴더 안 아카이브는 solo 배치로 이미 단독 화면이지만, 이미지로 그릴 수 없으므로
    // 만약을 위해 로드 대상에서도 뺀다.
    const paths = targetIndices
      .map((i) => images[i])
      .filter((p): p is string => !!p && !isArchiveFilePath(p))
    let cancelled = false

    Promise.all(
      paths.map(async (p) => {
        try {
          const info = await getOrLoadImage(p)
          // 두 장이 모두 그려질 준비가 될 때까지 이전 화면을 유지한다. 먼저 바꾸면
          // 한 장만 먼저 나타났다가 나머지가 로드될 때 옆으로 밀린다.
          await awaitPaintReady?.(info, p)
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
    cover,
    dirImages.images,
    dirImages.current_index,
    getOrLoadImage,
    awaitPaintReady,
    solo
  ])

  return { viewMode, pages }
}
