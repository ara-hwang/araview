import type { ImageInfo } from "@/types"
import { imageCacheKey } from "@/utils/imageCacheKey"

type CacheState = {
  entries: Map<string, ImageInfo>
  bytes: Map<string, number>
  totalBytes: number
  inflight: Map<string, Promise<ImageInfo>>
  generation: number
}

const state: CacheState = {
  entries: new Map(),
  bytes: new Map(),
  totalBytes: 0,
  inflight: new Map(),
  generation: 0
}

export function getCachedImageInfo(
  archivePath: string | null,
  pathOrEntry: string
): ImageInfo | undefined {
  return state.entries.get(imageCacheKey(archivePath, pathOrEntry))
}

export function getInflightImageLoad(
  archivePath: string | null,
  pathOrEntry: string
): Promise<ImageInfo> | undefined {
  return state.inflight.get(imageCacheKey(archivePath, pathOrEntry))
}

export function setInflightImageLoad(
  archivePath: string | null,
  pathOrEntry: string,
  promise: Promise<ImageInfo>
): void {
  state.inflight.set(imageCacheKey(archivePath, pathOrEntry), promise)
}

export function deleteInflightImageLoad(archivePath: string | null, pathOrEntry: string): void {
  state.inflight.delete(imageCacheKey(archivePath, pathOrEntry))
}

export function deleteCachedImage(key: string): void {
  const prevBytes = state.bytes.get(key) ?? 0
  state.entries.delete(key)
  state.bytes.delete(key)
  state.totalBytes = Math.max(0, state.totalBytes - prevBytes)
}

export function setCachedImage(
  archivePath: string | null,
  pathOrEntry: string,
  imgInfo: ImageInfo,
  byteSize: number,
  trim: (limitCount: number, limitBytes: number) => void,
  limitCount: number,
  limitBytes: number,
  expectedGeneration: number
): void {
  if (state.generation !== expectedGeneration) return
  const key = imageCacheKey(archivePath, pathOrEntry)
  if (state.entries.has(key)) deleteCachedImage(key)
  state.entries.set(key, imgInfo)
  state.bytes.set(key, byteSize)
  state.totalBytes += byteSize
  trim(limitCount, limitBytes)
}

export function trimImageMetaCache(limitCount: number, limitBytes: number): void {
  while (state.entries.size > limitCount || state.totalBytes > limitBytes) {
    const oldestKey = state.entries.keys().next().value
    if (!oldestKey) break
    deleteCachedImage(oldestKey)
  }
}

export function getImageMetaCacheGeneration(): number {
  return state.generation
}

/** 아카이브 전환·캐시 삭제 시 캐시를 비우고 이전 비동기 결과의 재삽입을 막는다. */
export function clearImageMetaCache(): void {
  state.generation += 1
  state.entries.clear()
  state.bytes.clear()
  state.totalBytes = 0
  state.inflight.clear()
}

/** 테스트 전용 */
export function getImageMetaCacheSize(): number {
  return state.entries.size
}
