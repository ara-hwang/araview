import type { ImageScalingMode } from "@/store/settingsStore"
import type { ImageInfo, PixelArtDetection } from "@/types"

export type ImageRenderingMode = "smooth" | "pixelated"

const PIXEL_ART_CONFIDENCE = 0.65

export function getPixelArtDetectionPath(info: ImageInfo): string {
  return info.file_path !== info.source_path ? info.file_path : info.source_path
}

/**
 * 픽셀 보존 표시는 확대(1x 이상)에서만 의미가 있다. 축소에서 nearest 계열
 * 보간은 픽셀을 버리므로 스크린톤 같은 주기 패턴이 계단·무아레로 깨진다.
 * 배율을 못 재면(null) 기존 판정을 유지한다.
 */
export function isMagnifiedDisplay(displayScale: number | null): boolean {
  return displayScale === null || displayScale >= 1
}

export function resolveImageRenderingMode(
  mode: ImageScalingMode,
  autoDetectPixelArt: boolean,
  detection: PixelArtDetection | null,
  displayScale: number | null = null
): ImageRenderingMode {
  if (!isMagnifiedDisplay(displayScale)) return "smooth"
  if (mode === "pixelated") return "pixelated"
  if (mode === "smooth") return "smooth"
  if (
    autoDetectPixelArt &&
    detection?.classification === "pixel_art" &&
    detection.confidence >= PIXEL_ART_CONFIDENCE
  ) {
    return "pixelated"
  }
  return "smooth"
}

/** SVG 여부. MIME이 비어 오는 경우를 위해 확장자도 함께 본다. */
export function isSvgImageInfo(
  info: Pick<ImageInfo, "mime_type" | "file_name"> | null | undefined
) {
  if (!info) return false
  return info.mime_type === "image/svg+xml" || info.file_name.toLowerCase().endsWith(".svg")
}
