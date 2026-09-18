/**
 * 90°/270° 회전 시 가로/세로를 교체한 이미지 크기를 반환합니다.
 */
import type { FitMode } from "@/store/settingsStore"

export function getOrientedImageSize(
  imgW: number,
  imgH: number,
  rotation: 0 | 90 | 180 | 270
): { width: number; height: number } {
  return rotation === 90 || rotation === 270
    ? { width: imgH, height: imgW }
    : { width: imgW, height: imgH }
}

/**
 * 컨테이너·이미지 크기와 줌으로 이동 가능한 최대 오프셋(절반)을 계산합니다.
 */
export function getPositionBounds(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number,
  zoom: number
): { maxX: number; maxY: number } {
  const scaledW = imgW * zoom
  const scaledH = imgH * zoom
  return {
    maxX: scaledW >= containerW ? (scaledW - containerW) / 2 : 0,
    maxY: scaledH >= containerH ? (scaledH - containerH) / 2 : 0
  }
}

/**
 * 위치를 maxX, maxY 기준으로 클램프합니다.
 */
export function clampPosition(
  x: number,
  y: number,
  maxX: number,
  maxY: number
): { x: number; y: number } {
  return {
    x: maxX > 0 ? Math.max(-maxX, Math.min(maxX, x)) : 0,
    y: maxY > 0 ? Math.max(-maxY, Math.min(maxY, y)) : 0
  }
}

/**
 * 원본 이미지 픽셀을 컨테이너에 맞출 때의 배율.
 * 크기를 아직 모르면 1을 반환합니다.
 */
export function getFitZoomFromSizes(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number
): number {
  if (containerW <= 0 || containerH <= 0 || imgW <= 0 || imgH <= 0) return 1
  const fit = Math.min(containerW / imgW, containerH / imgH)
  if (!Number.isFinite(fit) || fit <= 0) return 1
  return fit
}

/**
 * 기억된 맞춤 모드에 따른 줌 배율.
 * width/height/screen은 컨테이너에 꽉 맞추고(작은 이미지도 확대),
 * auto는 큰 이미지만 맞추고 작은 이미지는 100%로 둔다(기존 기본 동작).
 */
export function getZoomForFitMode(
  mode: FitMode,
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number
): number {
  if (containerW <= 0 || containerH <= 0 || imgW <= 0 || imgH <= 0) return 1
  let zoom: number
  switch (mode) {
    case "width":
      zoom = containerW / imgW
      break
    case "height":
      zoom = containerH / imgH
      break
    case "screen":
      zoom = Math.min(containerW / imgW, containerH / imgH)
      break
    case "auto":
    default:
      zoom = Math.min(1, Math.min(containerW / imgW, containerH / imgH))
      break
  }
  if (!Number.isFinite(zoom) || zoom <= 0) return 1
  return zoom
}

/**
 * 축소 하한. 작은 이미지는 100%까지, 큰 이미지는 화면 맞춤 배율까지 허용합니다.
 * 화면 맞춤(>100%)을 하한으로 두면 한 번 확대한 뒤 원래 크기로 돌아가지 못합니다.
 */
export function getMinZoom(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number
): number {
  return Math.min(1, getFitZoomFromSizes(containerW, containerH, imgW, imgH))
}

/**
 * 이미지가 컨테이너에 완전히 들어가는지 여부
 */
export function isFullyContained(
  containerW: number,
  containerH: number,
  imgW: number,
  imgH: number,
  zoom: number
): boolean {
  const scaledW = imgW * zoom
  const scaledH = imgH * zoom
  return scaledW <= containerW && scaledH <= containerH
}
