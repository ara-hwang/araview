import type { CacheMode } from "@/store/settingsStore"

export function getCacheLimit(mode: CacheMode): number {
  switch (mode) {
    case "off":
      return 1
    case "memory-1gb":
    case "memory-2gb":
      return Number.MAX_SAFE_INTEGER
    case "extended":
      return 64
    case "nearby":
    default:
      return 24
  }
}

export function getCacheByteLimit(mode: CacheMode): number {
  switch (mode) {
    case "memory-1gb":
      return 1_024 * 1_024 * 1_024
    case "memory-2gb":
      return 2_048 * 1_024 * 1_024
    default:
      return Infinity
  }
}

export function getPrefetchDistance(mode: CacheMode): number {
  switch (mode) {
    case "off":
      return 0
    case "memory-2gb":
      return 3
    case "memory-1gb":
      return 2
    case "extended":
      return 3
    case "nearby":
    default:
      return 1
  }
}
