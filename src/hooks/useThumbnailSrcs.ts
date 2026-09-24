import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useRef, useState } from "react"

import { useCacheInvalidationStore } from "@/store/cacheInvalidationStore"
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
/** 배경 로딩 청크 크기. 한 번에 너무 많이 디코드하지 않도록 나눈다. */
const DEFAULT_CHUNK_SIZE = 8
/** 청크 사이 지연(ms). 스트립 전체를 천천히 채우기 위한 간격. */
const DEFAULT_CHUNK_DELAY_MS = 60
/** 보이는 창이 바뀔 때 배경 큐를 다시 시작하기 전 대기(ms). 스크롤 중 재시작 방지. */
const PRIORITY_SETTLE_MS = 150

export type ThumbnailSrcsOptions = {
  /** 백엔드 썸네일 최대 변 길이 (기본 128) */
  maxSide?: number
  /** 아카이브 모드일 때 엔트리 이름. 설정하면 추출+리사이즈 전용 커맨드를 쓴다. */
  archivePath?: string | null
  /** 클라우드 placeholder 등 썸네일 생성을 건너뛸 경로 */
  skipThumbnailPaths?: ReadonlySet<string>
  /** 먼저 채울 경로 (보이는 창). 나머지는 청크로 천천히 이어서 로드한다. */
  priorityPaths?: readonly string[]
  /** 배경 로딩 청크 크기 */
  chunkSize?: number
  /** 청크 사이 지연(ms) */
  chunkDelayMs?: number
}

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms))

// 썸네일 스트립/그리드용 축소 이미지 URL을 로드하는 훅.
// 백엔드 썸네일이 실패하면(아카이브 엔트리, SVG 등)
// 원본 로드로 조용히 폴백한다.
// 인덱스 이동 시 깜빡임을 막기 위해 이전 목록에 남아 있는 항목은 유지하고
// 새로 들어온 경로만 추가로 로드한다.
// 목록 전체를 한 번에 요청하지 않고 보이는 창을 먼저 채운 뒤
// 나머지를 청크 단위로 천천히 이어서 채워 스트립 개수가 흔들리지 않게 한다.
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
  const cacheEpoch = useCacheInvalidationStore((state) => state.epoch)
  const previousCacheEpochRef = useRef(cacheEpoch)
  const cacheEpochRef = useRef(cacheEpoch)
  cacheEpochRef.current = cacheEpoch
  const pathsKey = paths.join("\0")
  const priorityKey = (options.priorityPaths ?? []).join("\0")
  const [settledPriorityKey, setSettledPriorityKey] = useState(priorityKey)
  const maxSide = options.maxSide ?? THUMB_MAX_SIDE
  const archivePath = options.archivePath ?? null
  const skipThumbnailPaths = options.skipThumbnailPaths
  const chunkSize = Math.max(1, options.chunkSize ?? DEFAULT_CHUNK_SIZE)
  const chunkDelayMs = Math.max(0, options.chunkDelayMs ?? DEFAULT_CHUNK_DELAY_MS)
  const getOrLoadRef = useRef(getOrLoadImage)
  getOrLoadRef.current = getOrLoadImage
  const urlsRef = useRef(urls)
  urlsRef.current = urls
  const failedRef = useRef(failed)
  failedRef.current = failed

  useEffect(() => {
    if (previousCacheEpochRef.current === cacheEpoch) return
    previousCacheEpochRef.current = cacheEpoch
    setUrls(new Map())
    setFailed(new Set())
    setRetryNonce((value) => value + 1)
  }, [cacheEpoch])

  // 보이는 창이 잠깐 바뀌는 동안에는 큐를 다시 시작하지 않는다.
  useEffect(() => {
    const id = window.setTimeout(() => setSettledPriorityKey(priorityKey), PRIORITY_SETTLE_MS)
    return () => window.clearTimeout(id)
  }, [priorityKey])

  const put = useCallback((path: string, filePath: string, epoch = cacheEpochRef.current) => {
    if (cacheEpochRef.current !== epoch) return
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
  }, [])

  const markFailed = useCallback((path: string, epoch = cacheEpochRef.current) => {
    if (cacheEpochRef.current !== epoch) return
    setFailed((prev) => {
      if (prev.has(path)) return prev
      const next = new Set(prev)
      next.add(path)
      return next
    })
  }, [])

  // 썸네일 생성 자체가 불가한 입력(SVG 등)만 원본 로드로 폴백한다.
  const loadFallbacks = useCallback(
    async (fallback: Iterable<string>, epoch = cacheEpochRef.current) => {
      await Promise.all(
        [...fallback].map(async (path) => {
          try {
            const info = await getOrLoadRef.current(path)
            put(path, info.file_path, epoch)
          } catch {
            markFailed(path, epoch)
          }
        })
      )
    },
    [markFailed, put]
  )

  /** 주어진 목록을 로드한다. 아카이브는 동시 4개, 일반은 1회 배치 + 폴백. */
  const loadBatch = useCallback(
    async (list: string[], isCancelled: () => boolean, epoch = cacheEpochRef.current) => {
      if (list.length === 0 || isCancelled() || cacheEpochRef.current !== epoch) return
      const isStale = () => isCancelled() || cacheEpochRef.current !== epoch

      if (archivePath) {
        // 아카이브: 엔트리별 추출+리사이즈. 동시성을 제한하고 실패분만 폴백한다.
        const fallback: string[] = []
        let cursor = 0
        const worker = async () => {
          while (!isStale()) {
            const index = cursor
            cursor += 1
            if (index >= list.length) return
            const path = list[index]
            try {
              const thumb = await invoke<ThumbnailInfo>("generate_archive_thumbnail", {
                archivePath,
                entryName: path,
                maxSide
              })
              if (isStale()) return
              put(path, thumb.file_path, epoch)
            } catch {
              fallback.push(path)
            }
          }
        }
        await Promise.all(
          Array.from({ length: Math.min(ARCHIVE_THUMB_CONCURRENCY, list.length) }, worker)
        )
        if (isStale() || fallback.length === 0) return
        await loadFallbacks(fallback, epoch)
        return
      }

      const archiveFiles = list.filter((p) => isArchiveFilePath(p))
      const rasterPaths = list.filter((p) => !isArchiveFilePath(p))
      const loaded = new Set<string>()

      for (const archiveFile of archiveFiles) {
        if (isStale()) return
        try {
          const thumb = await invoke<ThumbnailInfo>("generate_archive_file_thumbnail", {
            archivePath: archiveFile,
            maxSide
          })
          if (isStale()) return
          put(archiveFile, thumb.file_path, epoch)
          loaded.add(archiveFile)
        } catch {
          // 아래 raster 폴백과 동일하게 load_image 시도
        }
      }

      // 일반: 청크 1회 배치 호출. 실패 항목만 개별 폴백한다.
      if (rasterPaths.length > 0) {
        try {
          const results = await invoke<BatchThumb[]>("generate_thumbnails_batch", {
            filePaths: rasterPaths,
            maxSide
          })
          if (isStale()) return
          for (const r of results) {
            if (r.thumb) {
              if (isStale()) return
              put(r.source, r.thumb.file_path, epoch)
              loaded.add(r.source)
            }
          }
        } catch {
          if (isStale()) return
        }
      }

      const stillMissing = list.filter(
        (p) => !loaded.has(p) && !urlsRef.current.has(p) && !failedRef.current.has(p)
      )
      if (isStale() || stillMissing.length === 0) return
      await loadFallbacks(stillMissing, epoch)
    },
    [archivePath, loadFallbacks, maxSide, put]
  )

  useEffect(() => {
    const list = pathsKey === "" ? [] : pathsKey.split("\0")
    const wanted = new Set(list)
    const epoch = cacheEpoch
    let cancelled = false
    const isCancelled = () => cancelled

    // 이전 목록 중 새 목록에 없는 항목만 제거, 나머지는 유지 (깜빡임 방지)
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

    const priority = settledPriorityKey === "" ? [] : settledPriorityKey.split("\0")
    const prioritySet = new Set(priority)

    void (async () => {
      const missing = list.filter((p) => {
        if (skipThumbnailPaths?.has(p)) return false
        return !urlsRef.current.has(p) && !failedRef.current.has(p)
      })
      if (missing.length === 0) return

      // 보이는 창을 먼저 채운다.
      const priorityMissing = missing.filter((p) => prioritySet.has(p))
      if (priorityMissing.length > 0) {
        await loadBatch(priorityMissing, isCancelled, epoch)
        if (cancelled || cacheEpochRef.current !== epoch) return
      }

      // 나머지는 청크 단위로 천천히 이어서 채운다.
      const rest = missing.filter((p) => !prioritySet.has(p))
      for (let i = 0; i < rest.length; i += chunkSize) {
        if (cancelled || cacheEpochRef.current !== epoch) return
        const chunk = rest
          .slice(i, i + chunkSize)
          .filter((p) => !urlsRef.current.has(p) && !failedRef.current.has(p))
        if (chunk.length > 0) await loadBatch(chunk, isCancelled, epoch)
        if (cancelled || cacheEpochRef.current !== epoch) return
        if (chunkDelayMs > 0) await sleep(chunkDelayMs)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [
    pathsKey,
    settledPriorityKey,
    retryNonce,
    maxSide,
    archivePath,
    skipThumbnailPaths,
    chunkSize,
    chunkDelayMs,
    loadBatch,
    cacheEpoch
  ])

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
