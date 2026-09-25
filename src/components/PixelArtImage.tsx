import type { ImgHTMLAttributes } from "react"

import { usePixelArtDetection } from "@/hooks/usePixelArtDetection"
import { cn } from "@/lib/utils"
import type { ImageScalingMode } from "@/store/settingsStore"
import { resolveImageRenderingMode } from "@/utils/imageRendering"

type PixelArtImageProps = ImgHTMLAttributes<HTMLImageElement> & {
  src: string
  filePath: string
  detectionPath?: string
  fileSize?: number
  scalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  detectionEnabled?: boolean
  detectionPriority?: boolean
}

export function PixelArtImage({
  filePath,
  detectionPath,
  fileSize,
  scalingMode,
  autoDetectPixelArt,
  detectionEnabled = true,
  detectionPriority = false,
  className,
  style,
  ...props
}: PixelArtImageProps) {
  const detection = usePixelArtDetection(
    detectionPath ?? filePath,
    fileSize,
    scalingMode === "auto" && autoDetectPixelArt && detectionEnabled,
    detectionPriority
  )
  const imageRendering = resolveImageRenderingMode(scalingMode, autoDetectPixelArt, detection)

  return (
    <img
      {...props}
      className={cn(
        className,
        imageRendering === "pixelated" ? "image-rendering-pixelated" : "image-rendering-smooth"
      )}
      style={style}
    />
  )
}
