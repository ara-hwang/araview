import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useRef, useState } from "react"

import type { ImageInfo, ThumbnailInfo } from "@/types"
import { isArchiveFilePath } from "@/utils/archiveFile"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

type BatchThumb = {
  source: string
  thumb: ThumbnailInfo | null
  error: string | null
}

const THUMB_MAX_SIDE = 128
/** 아카이브 엔트리 썸네일 동시 추출 수. 압축 해제 경합을 제한한다. */
const ARCHIVE_THUMB_CONCURRENCY = 4

export type ThumbnailSrcsOptions = {
  /** 백엔드 썸네일 최대 변 길이 (기본 128) */
  maxSide?: number
  /** 아카이브 모드일 때 엔트리 이름. 설정하면 추출+리사이즈 전용 커맨드를 쓴다. */
  archivePath?: string | null
  /** 클라우드 placeholder 등 썸네일 생성을 건너뛸 경로 */
  skipThumbnailPaths?: ReadonlySet<string>
}

// 썸네일 스트립/그리드용 축소 이미지 URL을 로드하는 훅.
// 백엔드 썸네일이 실패하면(아카이브 엔트리, SVG 등)
// 원본 로드로 조용히 폴백한다.
// 인덱스 이동 시 깜빡임을 막기 위해 이전 목록에 남아 있는 항목은 유지하고
// 새로 들어온 경로만 추가로 로드한다.
export function useThumbnailSrcs(
  paths: string[],
  getOrLoadImage: GetOrLoadImage,
  options: ThumbnailSrcsOptions = {}
): {
  urls: Map<string, string>
  failed: Set<string>
  retry: (path: string) => void
} {
  const [urls, setUrls] = useState<Map<string, string>>(() => new Map())
  const [failed, setFailed] = useState<Set<string>>(() => new Set())
  const [retryNonce, setRetryNonce] = useState(0)
  const pathsKey = paths.join("\0")
  const maxSide = options.maxSide ?? THUMB_MAX_SIDE
  const archivePath = options.archivePath ?? null
  const skipThumbnailPaths = options.skipThumbnailPaths
  const getOrLoadRef = useRef(getOrLoadImage)
  getOrLoadRef.current = getOrLoadImage
  const urlsRef = useRef(urls)
  urlsRef.current = urls
  const failedRef = useRef(failed)
  failedRef.current = failed

  useEffect(() => {
    const list = pathsKey === "" ? [] : pathsKey.split("\0")
    const wanted = new Set(list)
    let cancelled = false

    // 이전 목록 중 새 윈도우에 없는 항목만 제거, 나머지는 유지 (깜빡임 방지)
    setUrls((prev) => {
      let changed = false
      const next = new Map<string, string>()
      for (const [k, v] of prev) {
        if (wanted.has(k)) next.set(k, v)
        else changed = true
      }
      if (!changed && next.size === prev.size) return prev
      return next
    })
    setFailed((prev) => {
      let changed = false
      const next = new Set<string>()
      for (const p of prev) {
        if (wanted.has(p)) next.add(p)
        else changed = true
      }
      if (!changed && next.size === prev.size) return prev
      return next
    })

    const put = (path: string, filePath: string) => {
      if (cancelled) return
      const src = convertFileSrc(filePath)
      setUrls((prev) => {
        if (prev.get(path) === src) return prev
        const next = new Map(prev)
        next.set(path, src)
        return next
      })
      setFailed((prev) => {
        if (!prev.has(path)) return prev
        const next = new Set(prev)
        next.delete(path)
        return next
      })
    }

    const markFailed = (path: string) => {
      if (cancelled) return
      setFailed((prev) => {
        if (prev.has(path)) return prev
        const next = new Set(prev)
        next.add(path)
        return next
      })
    }

    // 썸네일 생성 자체가 불가한 입력(SVG 등)만 원본 로드로 폴백한다.
    const loadFallbacks = async (fallback: Iterable<string>) => {
      await Promise.all(
        [...fallback].map(async (path) => {
          try {
            const info = await getOrLoadRef.current(path)
            put(path, info.file_path)
          } catch {
            markFailed(path)
          }
        })
      )
    }

    void (async () => {
      const missing = list.filter((p) => {
        if (skipThumbnailPaths?.has(p)) return false
        return !urlsRef.current.has(p) && !failedRef.current.has(p)
      })
      if (missing.length === 0) return

      if (archivePath) {
        // 아카이브: 엔트리별 추출+리사이즈. 동시성을 제한하고 실패분만 폴백한다.
        const fallback: string[] = []
        let cursor = 0
        const worker = async () => {
          while (!cancelled) {
            const index = cursor
            cursor += 1
            if (index >= missing.length) return
            const path = missing[index]
            try {
              const thumb = await invoke<ThumbnailInfo>("generate_archive_thumbnail", {
                archivePath,
                entryName: path,
                maxSide
              })
              put(path, thumb.file_path)
            } catch {
              fallback.push(path)
            }
          }
        }
        await Promise.all(
          Array.from({ length: Math.min(ARCHIVE_THUMB_CONCURRENCY, missing.length) }, worker)
        )
        if (cancelled || fallback.length === 0) return
        await loadFallbacks(fallback)
        return
      }

      const archiveFiles = missing.filter((p) => isArchiveFilePath(p))
      const rasterPaths = missing.filter((p) => !isArchiveFilePath(p))
      const loaded = new Set<string>()

      for (const archiveFile of archiveFiles) {
        try {
          const thumb = await invoke<ThumbnailInfo>("generate_archive_file_thumbnail", {
            archivePath: archiveFile,
            maxSide
          })
          put(archiveFile, thumb.file_path)
          loaded.add(archiveFile)
        } catch {
          // 아래 raster 폴백과 동일하게 load_image 시도
        }
      }

      // 일반: 윈도우 1회 배치 호출. 실패 항목만 개별 폴백한다.
      if (rasterPaths.length > 0) {
        try {
          const results = await invoke<BatchThumb[]>("generate_thumbnails_batch", {
            filePaths: rasterPaths,
            maxSide
          })
          if (cancelled) return
          for (const r of results) {
            if (r.thumb) {
              put(r.source, r.thumb.file_path)
              loaded.add(r.source)
            }
          }
        } catch {
          if (cancelled) return
        }
      }

      const stillMissing = missing.filter((p) => !loaded.has(p))
      if (cancelled || stillMissing.length === 0) return
      await loadFallbacks(stillMissing)
    })()

    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathsKey, retryNonce, maxSide, archivePath, skipThumbnailPaths])

  const retry = useCallback((path: string) => {
    setFailed((prev) => {
      if (!prev.has(path)) return prev
      const next = new Set(prev)
      next.delete(path)
      return next
    })
    setUrls((prev) => {
      if (!prev.has(path)) return prev
      const next = new Map(prev)
      next.delete(path)
      return next
    })
    setRetryNonce((n) => n + 1)
  }, [])

  return { urls, failed, retry }
}
