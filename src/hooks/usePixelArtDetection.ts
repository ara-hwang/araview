import { invoke } from "@tauri-apps/api/core"
import { useEffect, useState } from "react"

import type { PixelArtDetection } from "@/types"

const MAX_DETECTION_CACHE_ENTRIES = 128
const MAX_CONCURRENT_DETECTIONS = 2

const FALLBACK_DETECTION: PixelArtDetection = {
  classification: "uncertain",
  confidence: 0,
  pixel_scale: null,
  method: "unsupported"
}

type QueueEntry = {
  key: string
  filePath: string
  epoch: number
  priority: boolean
  resolve: (result: PixelArtDetection) => void
}

type SharedRequest = {
  key: string
  promise: Promise<PixelArtDetection>
  consumers: number
  started: boolean
  entry: QueueEntry
}

type DetectionLease = {
  promise: Promise<PixelArtDetection>
  release: () => void
  promote: () => void
}

const detectionCache = new Map<string, PixelArtDetection>()
const inflightDetections = new Map<string, SharedRequest>()
const pendingDetections: QueueEntry[] = []
let activeDetectionCount = 0
let cacheEpoch = 0

function cacheKey(filePath: string, fileSize: number | undefined): string {
  return `${filePath}\u0000${fileSize ?? ""}`
}

function cacheResult(key: string, result: PixelArtDetection): void {
  detectionCache.delete(key)
  detectionCache.set(key, result)
  while (detectionCache.size > MAX_DETECTION_CACHE_ENTRIES) {
    const oldest = detectionCache.keys().next().value
    if (!oldest) break
    detectionCache.delete(oldest)
  }
}

function removePending(entry: QueueEntry): void {
  const index = pendingDetections.indexOf(entry)
  if (index >= 0) pendingDetections.splice(index, 1)
}

function releaseRequest(request: SharedRequest): void {
  request.consumers = Math.max(0, request.consumers - 1)
  if (request.consumers > 0 || request.started) return
  removePending(request.entry)
  if (inflightDetections.get(request.key) === request) {
    inflightDetections.delete(request.key)
  }
  request.entry.resolve(FALLBACK_DETECTION)
}

function promoteRequest(request: SharedRequest): void {
  if (request.started) return
  request.entry.priority = true
  pumpDetectionQueue()
}

function pumpDetectionQueue(): void {
  while (activeDetectionCount < MAX_CONCURRENT_DETECTIONS && pendingDetections.length > 0) {
    const priorityIndex = pendingDetections.findIndex((entry) => entry.priority)
    const index = priorityIndex >= 0 ? priorityIndex : 0
    const [entry] = pendingDetections.splice(index, 1)
    if (!entry) continue
    const request = inflightDetections.get(entry.key)
    if (!request || request.entry !== entry || entry.epoch !== cacheEpoch) {
      entry.resolve(FALLBACK_DETECTION)
      continue
    }

    request.started = true
    activeDetectionCount += 1
    void Promise.resolve()
      .then(() => invoke<PixelArtDetection>("detect_pixel_art", { filePath: entry.filePath }))
      .then((result) => {
        if (entry.epoch === cacheEpoch) cacheResult(entry.key, result)
        entry.resolve(entry.epoch === cacheEpoch ? result : FALLBACK_DETECTION)
      })
      .catch(() => {
        // Decode/permission failures are not cached so a later mount can retry.
        entry.resolve(FALLBACK_DETECTION)
      })
      .finally(() => {
        activeDetectionCount -= 1
        if (inflightDetections.get(entry.key) === request) {
          inflightDetections.delete(entry.key)
        }
        pumpDetectionQueue()
      })
  }
}

function requestDetection(
  filePath: string,
  fileSize: number | undefined,
  priority: boolean
): DetectionLease {
  const key = cacheKey(filePath, fileSize)
  const cached = detectionCache.get(key)
  if (cached) {
    return {
      promise: Promise.resolve(cached),
      release: () => {},
      promote: () => {}
    }
  }

  const existing = inflightDetections.get(key)
  if (existing) {
    existing.consumers += 1
    if (priority) promoteRequest(existing)
    return {
      promise: existing.promise,
      release: () => releaseRequest(existing),
      promote: () => promoteRequest(existing)
    }
  }

  let resolvePromise!: (result: PixelArtDetection) => void
  const promise = new Promise<PixelArtDetection>((resolve) => {
    resolvePromise = resolve
  })
  const request = {} as SharedRequest
  const entry: QueueEntry = {
    key,
    filePath,
    epoch: cacheEpoch,
    priority,
    resolve: resolvePromise
  }
  request.key = key
  request.promise = promise
  request.consumers = 1
  request.started = false
  request.entry = entry
  inflightDetections.set(key, request)
  pendingDetections.push(entry)
  pumpDetectionQueue()

  return {
    promise,
    release: () => releaseRequest(request),
    promote: () => promoteRequest(request)
  }
}

/** 테스트와 장기 세션의 메모리 정리용. */
export function clearPixelArtDetectionCache(): void {
  cacheEpoch += 1
  detectionCache.clear()
  for (const entry of pendingDetections.splice(0)) {
    const request = inflightDetections.get(entry.key)
    if (request?.entry === entry) inflightDetections.delete(entry.key)
    entry.resolve(FALLBACK_DETECTION)
  }
  // 이미 시작된 디코드는 Tauri invoke를 취소할 수 없지만, 새 이미지가 같은
  // 경로를 다시 열 때 이전 epoch의 결과나 inflight promise를 재사용하지 않는다.
  for (const [key, request] of inflightDetections) {
    if (request.started) inflightDetections.delete(key)
  }
}

/**
 * 현재 이미지의 픽셀 아트 판별을 제한된 큐에서 백그라운드로 요청한다.
 * 감지가 늦거나 실패해도 이미지는 smooth 모드로 계속 표시된다.
 */
export function usePixelArtDetection(
  filePath: string | null,
  fileSize: number | undefined,
  enabled: boolean,
  priority = false
): PixelArtDetection | null {
  const currentKey = filePath && enabled ? cacheKey(filePath, fileSize) : null
  const [state, setState] = useState<{ key: string | null; value: PixelArtDetection | null }>(
    () => ({
      key: currentKey,
      value: currentKey ? (detectionCache.get(currentKey) ?? null) : null
    })
  )
  const detection = state.key === currentKey ? state.value : null

  useEffect(() => {
    if (!filePath || !enabled) {
      setState({ key: null, value: null })
      return
    }

    const key = cacheKey(filePath, fileSize)
    const cached = detectionCache.get(key)
    if (cached) {
      setState({ key, value: cached })
      return
    }

    let cancelled = false
    setState({ key, value: null })
    const request = requestDetection(filePath, fileSize, priority)
    if (priority) request.promote()
    void request.promise.then((result) => {
      if (!cancelled) setState({ key, value: result })
    })
    return () => {
      cancelled = true
      request.release()
    }
  }, [enabled, filePath, fileSize, priority])

  return detection
}
