import type { ImageScalingMode } from "@/store/settingsStore"
import type { ImageInfo, PixelArtDetection } from "@/types"

export type ImageRenderingMode = "smooth" | "pixelated"

const PIXEL_ART_CONFIDENCE = 0.65

/**
 * 자동 모드에서 픽셀 보존으로 전환하는 최소 표시 배율. 이 배율 이상이면
 * 원본 픽셀이 화면에 드러나므로, 자동 감지 결과와 무관하게 보간 없이
 * 픽셀을 그대로 보여준다(확대 시 bilinear 블러 대신 픽셀).
 */
export const AUTO_PIXELATED_MIN_SCALE = 2

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
  // 자동 모드에서 배율 2x 이상 확대는 자동 감지와 무관하게 픽셀을 보존한다.
  // 감지 결과는 1x~2x 확대 구간에서만 판정에 들어간다.
  if (displayScale !== null && displayScale >= AUTO_PIXELATED_MIN_SCALE) {
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
