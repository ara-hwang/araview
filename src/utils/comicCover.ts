import type { ComicInfo } from "@/types"

/** ComicInfo `Page Type`의 표지 값 (대소문자 무시 비교). */
export const FRONT_COVER_PAGE_TYPE = "frontcover"

/**
 * ComicInfo의 FrontCover 페이지를 양면 보기 표지 인덱스로 변환한다.
 *
 * 스키마는 0 기반이지만 1 기반으로 쓴 파일이 있고, 목록 길이와 PageCount가
 * 어긋나는 파일도 있다. `image`가 목록 범위를 벗어나면 0번으로 폴백한다.
 * 우선순위: ComicInfo FrontCover > 0번.
 */
export function resolveCoverIndex(comicInfo: ComicInfo | null, total: number): number {
  if (total <= 0) return 0
  const pages = comicInfo?.pages
  if (!pages || pages.length === 0) return 0
  const cover = pages.find((page) => page.page_type?.trim().toLowerCase() === FRONT_COVER_PAGE_TYPE)
  if (!cover) return 0
  const index = Math.trunc(cover.image)
  if (!Number.isFinite(index) || index < 0 || index >= total) return 0
  return index
}
