import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

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
/** 로딩 청크 크기. 청크가 끝날 때마다 그려서 먼저 끝난 썸네일부터 보인다. */
const DEFAULT_CHUNK_SIZE = 8
/** 배경 청크 사이 지연(ms). 스트립 전체를 천천히 채우기 위한 간격. */
const DEFAULT_CHUNK_DELAY_MS = 60
/** 보이는 창이 바뀔 때 배경 큐를 다시 시작하기 전 대기(ms). 스크롤 중 재시작 방지. */
const PRIORITY_SETTLE_MS = 150
/**
 * 처음 잡히는 보이는 창의 대기(ms). 크기 측정과 현재 항목으로의 스크롤이 연달아
 * 창을 바꾸므로 한 프레임만 모아, 지나가는 창의 썸네일을 요청하지 않는다.
 */
const PRIORITY_FIRST_SETTLE_MS = 16

export type ThumbnailSrcsOptions = {
  /** 백엔드 썸네일 최대 변 길이 (기본 128) */
  maxSide?: number
  /** 아카이브 모드일 때 엔트리 이름. 설정하면 추출+리사이즈 전용 커맨드를 쓴다. */
  archivePath?: string | null
  /** 클라우드 placeholder 등 썸네일 생성을 건너뛸 경로 */
  skipThumbnailPaths?: ReadonlySet<string>
  /**
   * 먼저 채울 경로 (보이는 창). 나머지는 청크 사이에 지연을 두고 이어서 로드한다.
   * 생략하면 목록 전체를 보이는 창으로 보고 지연 없이 로드한다.
   */
  priorityPaths?: readonly string[]
  /** 로딩 청크 크기 */
  chunkSize?: number
  /** 배경 청크 사이 지연(ms) */
  chunkDelayMs?: number
}

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms))

const NO_PATHS: readonly string[] = []

/**
 * 결과를 받아도 되는 로드 세대. 캐시 무효화, 아카이브, 썸네일 크기가 바뀌면 새
 * 세대가 되어 이전 세대의 늦은 응답을 버린다 (다른 아카이브의 같은 엔트리 이름
 * 포함). 요청 중인 경로를 함께 들고 있어 재시작한 큐가 같은 경로를 또 요청하지
 * 않는다.
 */
type LoadContext = { inflight: Set<string> }

function sameItems(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false
  }
  return true
}

/**
 * 내용이 같으면 이전 배열을 그대로 돌려준다. 호출부가 렌더마다 새 배열을
 * 넘겨도(그리드의 slice) 로드 effect가 재시작되지 않고, 목록 전체를 이어 붙인
 * 문자열 키를 렌더마다 만들 필요도 없다.
 */
function useStableList(list: readonly string[]): readonly string[] {
  const ref = useRef(list)
  if (ref.current !== list && !sameItems(ref.current, list)) ref.current = list
  return ref.current
}

// 썸네일 스트립/그리드용 축소 이미지 URL을 로드하는 훅.
// 백엔드 썸네일이 실패하면(아카이브 엔트리, SVG 등)
// 원본 로드로 조용히 폴백한다.
// 인덱스 이동 시 깜빡임을 막기 위해 이전 목록에 남아 있는 항목은 유지하고
// 새로 들어온 경로만 추가로 로드한다.
// 목록 전체를 한 번에 요청하지 않고 보이는 창을 먼저 청크 단위로 채운 뒤
// 나머지를 천천히 이어서 채워 스트립 개수가 흔들리지 않게 한다.
// 큐가 재시작돼도(스크롤, 창 이동) 이미 보낸 요청의 결과는 그대로 쓴다.
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
  const stablePaths = useStableList(paths)
  const hasPriority = options.priorityPaths !== undefined
  const priorityPaths = useStableList(options.priorityPaths ?? NO_PATHS)
  const [settledPriority, setSettledPriority] = useState(priorityPaths)
  const hadPriorityRef = useRef(priorityPaths.length > 0)
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
  const context = useMemo<LoadContext>(
    () => ({ inflight: new Set() }),
    // 값이 바뀔 때마다 새 세대를 만든다.
    [cacheEpoch, archivePath, maxSide]
  )
  const contextRef = useRef(context)
  contextRef.current = context
  const wantedRef = useRef<ReadonlySet<string>>(new Set(stablePaths))

  useEffect(() => {
    if (previousCacheEpochRef.current === cacheEpoch) return
    previousCacheEpochRef.current = cacheEpoch
    setUrls(new Map())
    setFailed(new Set())
    setRetryNonce((value) => value + 1)
  }, [cacheEpoch])

  // 보이는 창이 잠깐 바뀌는 동안에는 큐를 다시 시작하지 않는다. 처음 잡히는 창은
  // 짧게만 기다려 첫 화면이 늦지 않게 한다.
  useEffect(() => {
    const delay = hadPriorityRef.current ? PRIORITY_SETTLE_MS : PRIORITY_FIRST_SETTLE_MS
    const id = window.setTimeout(() => {
      if (priorityPaths.length > 0) hadPriorityRef.current = true
      setSettledPriority(priorityPaths)
    }, delay)
    return () => window.clearTimeout(id)
  }, [priorityPaths])

  /** 세대가 바뀌었거나 목록에서 빠진 경로의 늦은 응답은 버린다. */
  const accepts = useCallback(
    (path: string, ctx: LoadContext) => contextRef.current === ctx && wantedRef.current.has(path),
    []
  )

  const put = useCallback(
    (path: string, filePath: string, ctx: LoadContext) => {
      if (!accepts(path, ctx)) return
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
    },
    [accepts]
  )

  const markFailed = useCallback(
    (path: string, ctx: LoadContext) => {
      if (!accepts(path, ctx)) return
      setFailed((prev) => {
        if (prev.has(path)) return prev
        const next = new Set(prev)
        next.add(path)
        return next
      })
    },
    [accepts]
  )

  // 썸네일 생성 자체가 불가한 입력(SVG 등)만 원본 로드로 폴백한다.
  const loadFallbacks = useCallback(
    async (fallback: Iterable<string>, ctx: LoadContext) => {
      await Promise.all(
        [...fallback].map(async (path) => {
          try {
            const info = await getOrLoadRef.current(path)
            put(path, info.file_path, ctx)
          } catch {
            markFailed(path, ctx)
          }
        })
      )
    },
    [markFailed, put]
  )

  /**
   * 청크 하나를 로드한다. 아카이브와 일반 모두 1회 배치 + 폴백.
   * 큐가 취소돼도 보낸 요청은 끝까지 받아 결과를 반영한다.
   */
  const loadChunk = useCallback(
    async (list: string[], ctx: LoadContext) => {
      if (list.length === 0 || contextRef.current !== ctx) return
      const isStale = () => contextRef.current !== ctx
      for (const path of list) ctx.inflight.add(path)

      try {
        if (archivePath) {
          // 아카이브: 청크를 1회 배치로 보낸다. 백엔드가 아카이브를 워커당 한 번만
          // 열어 추출+리사이즈하고, 실패분(배치 자체의 실패 포함)만 폴백한다.
          const results = await invoke<BatchThumb[]>("generate_archive_thumbnails_batch", {
            archivePath,
            entryNames: list,
            maxSide
          }).catch((): BatchThumb[] => [])
          const loaded = new Set<string>()
          for (const r of results) {
            if (!r.thumb) continue
            put(r.source, r.thumb.file_path, ctx)
            loaded.add(r.source)
          }
          const fallback = list.filter((path) => !loaded.has(path))
          if (isStale() || fallback.length === 0) return
          await loadFallbacks(fallback, ctx)
          return
        }

        const archiveFiles = list.filter((p) => isArchiveFilePath(p))
        const rasterPaths = list.filter((p) => !isArchiveFilePath(p))
        const loaded = new Set<string>()
        const putResults = (results: BatchThumb[]) => {
          for (const r of results) {
            if (!r.thumb) continue
            put(r.source, r.thumb.file_path, ctx)
            loaded.add(r.source)
          }
        }

        // 표지와 일반 이미지는 서로 기다리지 않는다. 표지는 1회 배치 호출이고
        // 백엔드의 인덱스 캐시가 아카이브당 재스캔을 막는다. 배치 자체가 실패하면
        // 아래에서 항목별 load_image 폴백을 시도한다.
        await Promise.all([
          archiveFiles.length > 0 &&
            invoke<BatchThumb[]>("generate_archive_file_thumbnails_batch", {
              archivePaths: archiveFiles,
              maxSide
            }).then(putResults, () => {}),
          rasterPaths.length > 0 &&
            invoke<BatchThumb[]>("generate_thumbnails_batch", {
              filePaths: rasterPaths,
              maxSide
            }).then(putResults, () => {})
        ])

        const stillMissing = list.filter(
          (p) => !loaded.has(p) && !urlsRef.current.has(p) && !failedRef.current.has(p)
        )
        if (isStale() || stillMissing.length === 0) return
        await loadFallbacks(stillMissing, ctx)
      } finally {
        for (const path of list) ctx.inflight.delete(path)
      }
    },
    [archivePath, loadFallbacks, maxSide, put]
  )

  useEffect(() => {
    const list = stablePaths
    const wanted = new Set(list)
    wantedRef.current = wanted
    let cancelled = false

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

    const prioritySet = hasPriority ? new Set(settledPriority) : null
    const isPending = (p: string) =>
      !urlsRef.current.has(p) && !failedRef.current.has(p) && !context.inflight.has(p)
    const isStopped = () => cancelled || contextRef.current !== context

    void (async () => {
      const missing = list.filter((p) => !skipThumbnailPaths?.has(p) && isPending(p))
      if (missing.length === 0) return
      const priority = prioritySet ? missing.filter((p) => prioritySet.has(p)) : missing
      const rest = prioritySet ? missing.filter((p) => !prioritySet.has(p)) : []

      // 보이는 창은 지연 없이 청크 단위로 채워, 끝난 청크부터 바로 그린다.
      for (let i = 0; i < priority.length; i += chunkSize) {
        if (isStopped()) return
        await loadChunk(priority.slice(i, i + chunkSize).filter(isPending), context)
      }

      // 나머지는 청크 단위로 천천히 이어서 채운다. 보이는 창이 아직 없으면 곧
      // 잡힐 창이 먼저 가도록 한 박자 쉰다.
      if (priority.length === 0 && chunkDelayMs > 0) await sleep(chunkDelayMs)
      for (let i = 0; i < rest.length; i += chunkSize) {
        if (isStopped()) return
        const chunk = rest.slice(i, i + chunkSize).filter(isPending)
        if (chunk.length === 0) continue
        await loadChunk(chunk, context)
        if (isStopped()) return
        if (chunkDelayMs > 0) await sleep(chunkDelayMs)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [
    stablePaths,
    hasPriority,
    settledPriority,
    retryNonce,
    context,
    skipThumbnailPaths,
    chunkSize,
    chunkDelayMs,
    loadChunk
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
