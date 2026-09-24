import type { MaxResolution } from "@/store/settingsStore"

/** 표시 해상도 상한의 긴 변(px). null이면 원본 그대로 렌더한다. */
export function maxSideForResolution(mode: MaxResolution): number | null {
  switch (mode) {
    case "4k":
      return 3840
    case "1080p":
      return 1920
    case "original":
    default:
      return null
  }
}
