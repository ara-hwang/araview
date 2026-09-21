import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useRef } from "react"

import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

import type { ImageInfo } from "../types"
import { getCacheByteLimit, getCacheLimit, getPrefetchDistance } from "../utils/cacheConfig"
import { imageCacheKey } from "../utils/imageCacheKey"
import {
  clearImageMetaCache,
  deleteInflightImageLoad,
  getCachedImageInfo,
  getInflightImageLoad,
  setCachedImage,
  setInflightImageLoad,
  trimImageMetaCache
} from "../utils/imageMetaCache"
import { getPrefetchOrder } from "../utils/prefetchOrder"

// Tauri 백엔드에서 불러온 이미지를 메모리 캐시에 저장하고,
// 설정에 따라 캐시 용량/프리패치 범위를 제어하는 훅

/** 픽셀 프리웜 보관 상한. 메타 캐시와 별개로 작게 유지해 메모리 팽창을 막는다. */
const MAX_PAINT_PRELOADS = 12

export function useImageCache() {
  const cacheMode = useSettingsStore((state) => state.cacheMode)
  const paintPreloadsRef = useRef<Map<string, HTMLImageElement>>(new Map())

  const cacheLimit = useMemo(() => getCacheLimit(cacheMode), [cacheMode])
  const cacheByteLimit = useMemo(() => getCacheByteLimit(cacheMode), [cacheMode])

  const estimateImageBytes = useCallback((imgInfo: ImageInfo): number => {
    // Asset Protocol 전환 이후 이미지 데이터는 프론트 메모리에 상주하지 않는다.
    // 메타데이터만 캐시하므로, LRU 예산 계산에는 실제 파일 크기를 사용한다.
    return imgInfo.file_size
  }, [])

  const trimCacheToBudget = useCallback((limitCount: number, limitBytes: number) => {
    trimImageMetaCache(limitCount, limitBytes)
  }, [])

  const cacheImage = useCallback(
    (archivePath: string | null, pathOrEntry: string, imgInfo: ImageInfo) => {
      setCachedImage(
        archivePath,
        pathOrEntry,
        imgInfo,
        estimateImageBytes(imgInfo),
        trimCacheToBudget,
        cacheLimit,
        cacheByteLimit
      )
    },
    [cacheByteLimit, cacheLimit, estimateImageBytes, trimCacheToBudget]
  )

  const warmPaintImage = useCallback((imgInfo: ImageInfo, key: string) => {
    const preloaded = paintPreloadsRef.current
    if (preloaded.has(key)) return
    try {
      const url = convertFileSrc(imgInfo.file_path)
      const img = new Image()
      try {
        ;(img as HTMLImageElement & { decoding?: string }).decoding = "async"
      } catch {
        // 구형 환경 무시
      }
      img.src = url
      preloaded.set(key, img)
      while (preloaded.size > MAX_PAINT_PRELOADS) {
        const oldestKey = preloaded.keys().next().value
        if (!oldestKey) break
        const oldest = preloaded.get(oldestKey)
        preloaded.delete(oldestKey)
        try {
          oldest?.removeAttribute("src")
        } catch {
          // 무시
        }
      }
    } catch {
      // jsdom/테스트 환경이나 URL 변환 실패는 조용히 스킵
    }
  }, [])

  const getOrLoadImage = useCallback(
    async (pathOrEntry: string): Promise<ImageInfo> => {
      const scopeAtStart = useAppStore.getState().archivePath
      const cached = getCachedImageInfo(scopeAtStart, pathOrEntry)
      if (cached) return cached

      const inflight = getInflightImageLoad(scopeAtStart, pathOrEntry)
      if (inflight) return inflight

      const promise = (
        scopeAtStart
          ? invoke<ImageInfo>("load_archive_image", {
              archivePath: scopeAtStart,
              entryName: pathOrEntry
            })
          : invoke<ImageInfo>("load_image", { filePath: pathOrEntry })
      )
        .then((imgInfo) => {
          if (useAppStore.getState().archivePath === scopeAtStart) {
            cacheImage(scopeAtStart, pathOrEntry, imgInfo)
          }
          return imgInfo
        })
        .finally(() => {
          deleteInflightImageLoad(scopeAtStart, pathOrEntry)
        })

      setInflightImageLoad(scopeAtStart, pathOrEntry, promise)
      return promise
    },
    [cacheImage]
  )

  const prefetchNearbyImages = useCallback(
    (images: string[], index: number, loopNavigation: boolean, prefetchDistance: number) => {
      if (!images.length || prefetchDistance <= 0) return

      const archivePath = useAppStore.getState().archivePath
      const ordered = getPrefetchOrder(images.length, index, loopNavigation, prefetchDistance)

      for (const targetIndex of ordered) {
        const targetPath = images[targetIndex]
        if (!targetPath) continue
        const paintKey = imageCacheKey(archivePath, targetPath)
        if (paintPreloadsRef.current.has(paintKey)) continue

        const cached = getCachedImageInfo(archivePath, targetPath)
        if (cached) {
          warmPaintImage(cached, paintKey)
          continue
        }

        const inflight = getInflightImageLoad(archivePath, targetPath)
        if (inflight) {
          void inflight.then((info) => warmPaintImage(info, paintKey)).catch(() => {})
          continue
        }

        void getOrLoadImage(targetPath)
          .then((info) => warmPaintImage(info, paintKey))
          .catch(() => {})
      }
    },
    [getOrLoadImage, warmPaintImage]
  )

  useEffect(() => {
    trimCacheToBudget(cacheLimit, cacheByteLimit)
    if (cacheMode === "off") {
      for (const [key, img] of paintPreloadsRef.current) {
        paintPreloadsRef.current.delete(key)
        try {
          img.removeAttribute("src")
        } catch {
          // 무시
        }
      }
    }
  }, [cacheByteLimit, cacheLimit, cacheMode, trimCacheToBudget])

  return {
    getOrLoadImage,
    prefetchNearbyImages,
    getPrefetchDistance: () => getPrefetchDistance(cacheMode),
    clearImageMetaCache
  }
}
