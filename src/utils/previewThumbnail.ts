import type { ImageInfo } from "@/types"

/** 프리뷰 썸네일을 쓸 최소 픽셀 수 (약 2MP) */
export const PREVIEW_MIN_PIXELS = 2_000_000
/** 프리뷰 썸네일을 쓸 최소 파일 크기 (약 1.5MB) */
export const PREVIEW_MIN_BYTES = 1_500_000

/**
 * 풀사이즈 첫 페인트가 오래 걸릴 만한 이미지인지 판정한다. 캐시된 저해상
 * 썸네일을 먼저 깔아 체감 로딩을 줄이는 대상만 고르며, GIF는 프레임 제어
 * 캔버스가 따로 처리하므로 제외한다.
 */
export function shouldUsePreviewThumbnail(info: ImageInfo): boolean {
  if (info.mime_type === "image/gif") return false
  const pixels = (info.width ?? 0) * (info.height ?? 0)
  return pixels >= PREVIEW_MIN_PIXELS || info.file_size >= PREVIEW_MIN_BYTES
}
