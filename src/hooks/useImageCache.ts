import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useRef } from "react"

import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"

import type { ImageInfo } from "../types"
import { getCacheByteLimit, getCacheLimit, getPrefetchDistance } from "../utils/cacheConfig"
import { getPrefetchOrder } from "../utils/prefetchOrder"

// Tauri 백엔드에서 불러온 이미지를 메모리 캐시에 저장하고,
// 설정에 따라 캐시 용량/프리패치 범위를 제어하는 훅

/** 픽셀 프리웜 보관 상한. 메타 캐시와 별개로 작게 유지해 메모리 팽창을 막는다. */
const MAX_PAINT_PRELOADS = 12

export function useImageCache() {
  const cacheMode = useSettingsStore((state) => state.cacheMode)
  const imageCacheRef = useRef<Map<string, ImageInfo>>(new Map())
  const imageCacheBytesRef = useRef<Map<string, number>>(new Map())
  const totalCacheBytesRef = useRef<number>(0)
  const inflightLoadsRef = useRef<Map<string, Promise<ImageInfo>>>(new Map())
  const paintPreloadsRef = useRef<Map<string, HTMLImageElement>>(new Map())

  const cacheLimit = useMemo(() => getCacheLimit(cacheMode), [cacheMode])
  const cacheByteLimit = useMemo(() => getCacheByteLimit(cacheMode), [cacheMode])

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
    totalCacheBytesRef.current = Math.max(0, totalCacheBytesRef.current - prevBytes)
    // 메타 제거 시 픽셀 프리웜도 함께 해제해 메모리를 돌려준다.
    const preloaded = paintPreloadsRef.current.get(filePath)
    if (preloaded) {
      paintPreloadsRef.current.delete(filePath)
      try {
        preloaded.removeAttribute("src")
      } catch {
        // 해제 실패는 무시 (GC에 맡김)
      }
    }
  }, [])

  // LRU 비슷하게, 오래된 항목부터 제거해서 캐시를 예산 이하로 유지
  const trimCacheToBudget = useCallback(
    (limitCount: number, limitBytes: number) => {
      const cache = imageCacheRef.current
      while (cache.size > limitCount || totalCacheBytesRef.current > limitBytes) {
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
    [cacheByteLimit, cacheLimit, deleteFromCache, estimateImageBytes, trimCacheToBudget]
  )

  // ImageInfo(메타) 확보 후 WebView 디코딩까지 미리 끝내는 픽셀 프리웜.
  // 다음 페이지 체감 지연의 주범이 <img> 첫 디코딩이므로, 이웃 경로의
  // asset URL을 HTMLImageElement로 미리 로드해 브라우저 캐시를 데운다.
  const warmPaintImage = useCallback((imgInfo: ImageInfo, key: string) => {
    const preloaded = paintPreloadsRef.current
    if (preloaded.has(key)) return
    try {
      const url = convertFileSrc(imgInfo.file_path)
      const img = new Image()
      // decoding async 힌트: 파서 블로킹 없이 백그라운드 디코드 유도
      try {
        ;(img as HTMLImageElement & { decoding?: string }).decoding = "async"
      } catch {
        // 구형 환경 무시
      }
      img.src = url
      // ref에 보관해야 GC되지 않고 디코드 결과가 유지된다.
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

  // 현재 인덱스를 기준으로 앞/뒤 prefetchDistance 만큼의 이미지를 미리 로드.
  // 메타(ImageInfo) 확보에 이어 픽셀 프리웜(new Image)까지 수행해
  // 다음 페이지의 WebView 첫 디코딩 지연을 줄인다.
  const prefetchNearbyImages = useCallback(
    (images: string[], index: number, loopNavigation: boolean, prefetchDistance: number) => {
      if (!images.length || prefetchDistance <= 0) return

      const ordered = getPrefetchOrder(images.length, index, loopNavigation, prefetchDistance)

      for (const targetIndex of ordered) {
        const targetPath = images[targetIndex]
        if (!targetPath) continue
        if (paintPreloadsRef.current.has(targetPath)) continue

        const cached = imageCacheRef.current.get(targetPath)
        if (cached) {
          // 메타는 이미 있음: 픽셀만 데우면 된다.
          warmPaintImage(cached, targetPath)
          continue
        }

        const inflight = inflightLoadsRef.current.get(targetPath)
        if (inflight) {
          void inflight.then((info) => warmPaintImage(info, targetPath)).catch(() => {})
          continue
        }

        void getOrLoadImage(targetPath)
          .then((info) => warmPaintImage(info, targetPath))
          .catch(() => {})
      }
    },
    [getOrLoadImage, warmPaintImage]
  )

  // 캐시 모드가 바뀌면 즉시 캐시 크기를 재조정
  useEffect(() => {
    trimCacheToBudget(cacheLimit, cacheByteLimit)
    if (cacheMode === "off") {
      // off 모드에서는 이웃 예열을 하지 않으므로 기존 프리웜도 비운다.
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
    getPrefetchDistance: () => getPrefetchDistance(cacheMode)
  }
}
