import { useCallback, useEffect, useRef, useState, type ImgHTMLAttributes } from "react"

import { usePixelArtDetection } from "@/hooks/usePixelArtDetection"
import { cn } from "@/lib/utils"
import type { ImageScalingMode } from "@/store/settingsStore"
import { isMagnifiedDisplay, resolveImageRenderingMode } from "@/utils/imageRendering"

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

/** 로드/측정 전 기본값. 확대 여부를 모를 때는 기존 판정을 유지한다. */
const UNMEASURED_DISPLAY_SCALE = null

/**
 * <img>의 실제 표시 배율(기기 픽셀 기준)을 측정한다. 웹툰 fitWidth나
 * max-w-full 제약은 레이아웃에서 이미지를 줄일 수 있고, info.width/height는
 * 표시 바이트 기준이라 최종 배율은 렌더된 요소에서만 정확하다.
 */
export function useMeasuredDisplayScale(): [
  (el: HTMLImageElement | null) => void,
  () => void,
  number | null
] {
  const [displayScale, setDisplayScale] = useState<number | null>(UNMEASURED_DISPLAY_SCALE)
  const elementRef = useRef<HTMLImageElement | null>(null)

  const measure = useCallback(() => {
    const el = elementRef.current
    if (!el || !el.complete || el.naturalWidth <= 0) return
    const rect = el.getBoundingClientRect()
    if (rect.width <= 0) return
    const dpr = window.devicePixelRatio || 1
    setDisplayScale((rect.width * dpr) / el.naturalWidth)
  }, [])

  const ref = useCallback(
    (el: HTMLImageElement | null) => {
      elementRef.current = el
      if (!el) return
      measure()
    },
    [measure]
  )

  useEffect(() => {
    const el = elementRef.current
    if (!el) return
    // jsdom 등 ResizeObserver가 없는 환경에서도 resize로 재측정되게 한다.
    const observer =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => measure())
    observer?.observe(el)
    window.addEventListener("resize", measure)
    return () => {
      observer?.disconnect()
      window.removeEventListener("resize", measure)
    }
  }, [measure])

  return [ref, measure, displayScale]
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
  const [imgRef, measure, displayScale] = useMeasuredDisplayScale()
  // 판정은 원본을 백엔드에서 한 번 더 디코드한다. 결과는 확대 표시에서만
  // 쓰이므로 실측 배율이 1x 이상인 이미지만 요청한다. 실측 전에는 배율을
  // 모르므로 기다린다(양쪽 보기의 만화 페이지는 대개 축소 표시다).
  const detection = usePixelArtDetection(
    detectionPath ?? filePath,
    fileSize,
    scalingMode === "auto" &&
      autoDetectPixelArt &&
      detectionEnabled &&
      displayScale !== null &&
      isMagnifiedDisplay(displayScale),
    detectionPriority
  )
  // 축소 배율에서는 nearest 보간이 스크린톤 같은 주기 패턴을 깨뜨리므로
  // resolveImageRenderingMode가 smooth로 강제한다.
  const imageRendering = resolveImageRenderingMode(
    scalingMode,
    autoDetectPixelArt,
    detection,
    displayScale
  )

  return (
    <img
      {...props}
      ref={imgRef}
      onLoad={(event) => {
        measure()
        props.onLoad?.(event)
      }}
      className={cn(
        className,
        imageRendering === "pixelated" ? "image-rendering-pixelated" : "image-rendering-smooth"
      )}
      style={style}
    />
  )
}
