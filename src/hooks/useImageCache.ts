import { useCallback, useEffect, useMemo, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { ImageInfo } from "../types"
import {
  getCacheByteLimit,
  getCacheLimit,
  getPrefetchDistance
} from "../utils/cacheConfig"
import { useSettingsStore } from "@/store/settingsStore"
import { useAppStore } from "@/store/appStore"

// Tauri 백엔드에서 불러온 이미지를 메모리 캐시에 저장하고,
// 설정에 따라 캐시 용량/프리패치 범위를 제어하는 훅

export function useImageCache() {
  const cacheMode = useSettingsStore((state) => state.cacheMode)
  const imageCacheRef = useRef<Map<string, ImageInfo>>(new Map())
  const imageCacheBytesRef = useRef<Map<string, number>>(new Map())
  const totalCacheBytesRef = useRef<number>(0)
  const inflightLoadsRef = useRef<Map<string, Promise<ImageInfo>>>(new Map())

  const cacheLimit = useMemo(() => getCacheLimit(cacheMode), [cacheMode])
  const cacheByteLimit = useMemo(
    () => getCacheByteLimit(cacheMode),
    [cacheMode]
  )

  const estimateImageBytes = useCallback((imgInfo: ImageInfo): number => {
    // Asset Protocol 전환 이후 이미지 데이터는 프론트 메모리에 상주하지 않는다.
    // 메타데이터만 캐시하므로, LRU 예산 계산에는 실제 파일 크기를 사용한다.
    return imgInfo.file_size
  }, [])

  const deleteFromCache = useCallback((filePath: string) => {
    const bytesMap = imageCacheBytesRef.current
    const prevBytes = bytesMap.get(filePath) ?? 0
    imageCacheRef.current.delete(filePath)
    bytesMap.delete(filePath)
    totalCacheBytesRef.current = Math.max(
      0,
      totalCacheBytesRef.current - prevBytes
    )
  }, [])

  // LRU 비슷하게, 오래된 항목부터 제거해서 캐시를 예산 이하로 유지
  const trimCacheToBudget = useCallback(
    (limitCount: number, limitBytes: number) => {
      const cache = imageCacheRef.current
      while (
        cache.size > limitCount ||
        totalCacheBytesRef.current > limitBytes
      ) {
        const oldestKey = cache.keys().next().value
        if (!oldestKey) break
        deleteFromCache(oldestKey)
      }
    },
    [deleteFromCache]
  )

  const cacheImage = useCallback(
    (filePath: string, imgInfo: ImageInfo) => {
      const cache = imageCacheRef.current
      if (cache.has(filePath)) deleteFromCache(filePath)
      cache.set(filePath, imgInfo)

      const bytes = estimateImageBytes(imgInfo)
      imageCacheBytesRef.current.set(filePath, bytes)
      totalCacheBytesRef.current += bytes

      trimCacheToBudget(cacheLimit, cacheByteLimit)
    },
    [
      cacheByteLimit,
      cacheLimit,
      deleteFromCache,
      estimateImageBytes,
      trimCacheToBudget
    ]
  )

  // 단일 이미지를 캐시/진행 중 요청을 우선 확인한 뒤 필요한 경우만 실제 invoke 호출
  const getOrLoadImage = useCallback(
    async (filePath: string): Promise<ImageInfo> => {
      const cached = imageCacheRef.current.get(filePath)
      if (cached) return cached

      const inflight = inflightLoadsRef.current.get(filePath)
      if (inflight) return inflight

      // Listing keys in archive mode are zip entry names, not filesystem paths.
      const archivePath = useAppStore.getState().archivePath
      const promise = (
        archivePath
          ? invoke<ImageInfo>("load_archive_image", {
              archivePath,
              entryName: filePath
            })
          : invoke<ImageInfo>("load_image", { filePath })
      )
        .then((imgInfo) => {
          cacheImage(filePath, imgInfo)
          return imgInfo
        })
        .finally(() => {
          inflightLoadsRef.current.delete(filePath)
        })

      inflightLoadsRef.current.set(filePath, promise)
      return promise
    },
    [cacheImage]
  )

  // 현재 인덱스를 기준으로 앞/뒤 prefetchDistance 만큼의 이미지를 미리 로드
  const prefetchNearbyImages = useCallback(
    (
      images: string[],
      index: number,
      loopNavigation: boolean,
      prefetchDistance: number
    ) => {
      if (!images.length || prefetchDistance <= 0) return

      const targets = new Set<number>()
      const imageCount = images.length
      const normalizeIndex = (value: number) =>
        ((value % imageCount) + imageCount) % imageCount

      for (let offset = 1; offset <= prefetchDistance; offset += 1) {
        const prevIndex = index - offset
        if (prevIndex >= 0) targets.add(prevIndex)
        else if (loopNavigation && imageCount > 1)
          targets.add(normalizeIndex(prevIndex))

        const nextIndex = index + offset
        if (nextIndex < imageCount) targets.add(nextIndex)
        else if (loopNavigation && imageCount > 1)
          targets.add(normalizeIndex(nextIndex))
      }

      for (const targetIndex of targets) {
        const targetPath = images[targetIndex]
        if (!targetPath) continue
        if (imageCacheRef.current.has(targetPath)) continue
        if (inflightLoadsRef.current.has(targetPath)) continue
        void getOrLoadImage(targetPath).catch(() => {})
      }
    },
    [getOrLoadImage]
  )

  // 캐시 모드가 바뀌면 즉시 캐시 크기를 재조정
  useEffect(() => {
    trimCacheToBudget(cacheLimit, cacheByteLimit)
  }, [cacheByteLimit, cacheLimit, trimCacheToBudget])

  return {
    getOrLoadImage,
    prefetchNearbyImages,
    getPrefetchDistance: () => getPrefetchDistance(cacheMode)
  }
}
