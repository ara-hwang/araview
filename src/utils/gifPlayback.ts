import type { ViewMode } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"

export const GIF_MIME = "image/gif"

export function isGifImage(info: ImageInfo | null | undefined): boolean {
  if (!info) return false
  return info.mime_type === GIF_MIME || info.file_name.toLowerCase().endsWith(".gif")
}

/** WebCodecs ImageDecoder 지원 여부. 미지원이면 네이티브 `<img>` 애니메이션만 쓴다. */
export function supportsAnimationControl(): boolean {
  return typeof ImageDecoder !== "undefined"
}

/** 단일 보기 + GIF + 디코더 지원일 때만 캔버스 재생/정지/프레임 이동을 제공한다. */
export function canControlGif(info: ImageInfo | null | undefined, viewMode: ViewMode): boolean {
  return viewMode === "single" && isGifImage(info) && supportsAnimationControl()
}

/** 프레임 인덱스를 [0, frameCount-1]로 자른다. */
export function clampGifFrame(frame: number, frameCount: number): number {
  if (frameCount <= 0) return 0
  if (!Number.isFinite(frame)) return 0
  return Math.max(0, Math.min(Math.trunc(frame), frameCount - 1))
}

/** 프레임 단위 이동. 끝에서는 감싸지 않고 멈춘다(재생과 달리 이동은 경계 고정). */
export function stepGifFrame(frame: number, frameCount: number, delta: number): number {
  if (frameCount <= 0) return 0
  return clampGifFrame(clampGifFrame(frame, frameCount) + delta, frameCount)
}
