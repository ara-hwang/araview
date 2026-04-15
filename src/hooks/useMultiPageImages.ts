import { useEffect, useState } from "react"
import type { ImageInfo } from "@/types"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { useShallow } from "zustand/react/shallow"

// viewMode에 따라 현재 이미지 외에 주변 이미지를 로드해 반환한다.
//   - single               : 빈 배열 (ImageContainer가 기본 단일 렌더를 사용)
//   - left-to-right / right-to-left : [current, next]
//   - webtoon              : [prev, current, next]
//
// 실제 캐시는 useImageLoader에서 관리하는 동일한 인스턴스를 재사용하기 위해
// getOrLoadImage를 인자로 받는다.

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

export type MultiPage = {
  path: string
  info: ImageInfo
}

export function useMultiPageImages(getOrLoadImage: GetOrLoadImage) {
  const { viewMode, loopNavigation } = useSettingsStore(
    useShallow((s) => ({
      viewMode: s.viewMode,
      loopNavigation: s.loopNavigation
    }))
  )
  const dirImages = useAppStore((s) => s.dirImages)
  const [pages, setPages] = useState<MultiPage[]>([])

  useEffect(() => {
    if (viewMode === "single") {
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
    const normalize = (v: number) => ((v % count) + count) % count

    // 어떤 오프셋들을 로드할지 결정
    let offsets: number[]
    if (viewMode === "webtoon") {
      offsets = [-1, 0, 1]
    } else {
      // LTR / RTL: 현재 + 다음
      offsets = [0, 1]
    }

    const targetIndices: number[] = []
    for (const off of offsets) {
      const raw = index + off
      if (raw >= 0 && raw < count) targetIndices.push(raw)
      else if (loopNavigation && count > 1) targetIndices.push(normalize(raw))
    }

    const paths = targetIndices.map((i) => images[i]).filter(Boolean)
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
    dirImages.images,
    dirImages.current_index,
    getOrLoadImage
  ])

  return { viewMode, pages }
}
