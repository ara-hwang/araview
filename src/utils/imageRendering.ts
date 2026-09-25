import type { ImageScalingMode } from "@/store/settingsStore"
import type { ImageInfo, PixelArtDetection } from "@/types"

export type ImageRenderingMode = "smooth" | "pixelated"

const PIXEL_ART_CONFIDENCE = 0.65

export function getPixelArtDetectionPath(info: ImageInfo): string {
  return info.file_path !== info.source_path ? info.file_path : info.source_path
}

export function resolveImageRenderingMode(
  mode: ImageScalingMode,
  autoDetectPixelArt: boolean,
  detection: PixelArtDetection | null
): ImageRenderingMode {
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
