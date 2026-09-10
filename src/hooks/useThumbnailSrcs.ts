import { useCallback, useEffect, useRef, useState } from "react"
import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import type { ImageInfo, ThumbnailInfo } from "@/types"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

type BatchThumb = {
  source: string
  thumb: ThumbnailInfo | null
  error: string | null
}

const THUMB_MAX_SIDE = 128

// 썸네일 스트립용 축소 이미지 URL을 로드하는 훅.
// 백엔드 `generate_thumbnail`이 실패하면(아카이브 엔트리, SVG/HEIC 등)
// 원본 로드로 조용히 폴백한다.
// 인덱스 이동 시 깜빡임을 막기 위해 이전 목록에 남아 있는 항목은 유지하고
// 새로 들어온 경로만 추가로 로드한다.
export function useThumbnailSrcs(
  paths: string[],
  getOrLoadImage: GetOrLoadImage
): {
  urls: Map<string, string>
  failed: Set<string>
  retry: (path: string) => void
} {
  const [urls, setUrls] = useState<Map<string, string>>(() => new Map())
  const [failed, setFailed] = useState<Set<string>>(() => new Set())
  const [retryNonce, setRetryNonce] = useState(0)
  const pathsKey = paths.join("\0")
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

    void (async () => {
      const missing = list.filter(
        (p) => !urlsRef.current.has(p) && !failedRef.current.has(p)
      )
      if (missing.length === 0) return
      // 윈도우 1회 배치 호출. 실패 항목만 개별 폴백한다.
      let batchFailed: Set<string> | null = null
      try {
        const results = await invoke<BatchThumb[]>(
          "generate_thumbnails_batch",
          { filePaths: missing, maxSide: THUMB_MAX_SIDE }
        )
        if (cancelled) return
        batchFailed = new Set<string>()
        for (const r of results) {
          if (r.thumb) put(r.source, r.thumb.file_path)
          else batchFailed.add(r.source)
        }
      } catch {
        if (cancelled) return
        batchFailed = new Set(missing)
      }
      if (cancelled || !batchFailed || batchFailed.size === 0) return
      await Promise.all(
        [...batchFailed].map(async (path) => {
          try {
            const info = await getOrLoadRef.current(path)
            put(path, info.file_path)
          } catch {
            markFailed(path)
          }
        })
      )
    })()

    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathsKey, retryNonce])

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
