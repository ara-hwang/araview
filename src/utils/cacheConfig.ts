import type { CacheMode } from "@/store/settingsStore"

export function getCacheLimit(mode: CacheMode): number {
  switch (mode) {
    case "off":
      return 1
    case "extended":
      return 64
    case "nearby":
    default:
      return 24
  }
}

export function getPrefetchDistance(mode: CacheMode): number {
  switch (mode) {
    case "off":
      return 0
    case "extended":
      return 3
    case "nearby":
    default:
      return 1
  }
}
