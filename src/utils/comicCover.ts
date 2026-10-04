import type { ComicInfo, ComicPage } from "@/types"
import type { SoloIndices } from "@/utils/dirNavigation"

/** ComicInfo `Page Type`의 표지 값 (대소문자 무시 비교). */
export const FRONT_COVER_PAGE_TYPE = "frontcover"

/** 양쪽 보기의 표지 배치. 표지는 `coverIndex`부터 `coverCount`장 연속이다. */
export type CoverLayout = {
  /** 표지를 단독 화면으로 보여줄지 */
  coverAlone: boolean
  coverIndex: number
  coverCount: number
}

const isFrontCover = (page: ComicPage) =>
  page.page_type?.trim().toLowerCase() === FRONT_COVER_PAGE_TYPE

/** `index`를 포함해 끊기지 않고 이어지는 구간만 남긴다. */
function contiguousRun(indices: readonly number[], start: number): number[] {
  const marked = new Set(indices)
  const run: number[] = []
  for (let i = start; marked.has(i); i++) run.push(i)
  return run
}

/**
 * ComicInfo에 명시된 표지 인덱스. `FrontCover`가 여러 장이면 가장 앞에서부터
 * 연속으로 이어지는 구간만 표지로 본다(변형 표지 등). 목록 범위를 벗어난
 * `image`(1 기반으로 적은 파일 등)는 버린다.
 */
export function explicitCoverPages(comicInfo: ComicInfo | null, total: number): number[] {
  const marked = (comicInfo?.pages ?? [])
    .filter(isFrontCover)
    .map((page) => Math.trunc(page.image))
    .filter((index) => Number.isFinite(index) && index >= 0 && index < total)
  if (marked.length === 0) return []
  return contiguousRun(marked, Math.min(...marked))
}

/**
 * 아카이브의 표지 배치를 정한다. 아카이브 지정이 전역 설정보다 우선한다.
 *
 * - `FrontCover`가 있으면 그 구간을 단독으로 보여준다.
 * - `FrontCover`가 없고 0번에 다른 `Type`이 명시돼 있으면 "표지 없음"이라 쌍으로 본다.
 * - 아무 지정이 없으면(또는 `FrontCover`가 범위 밖이면) 전역 설정대로 0번을 표지로 본다.
 */
export function resolveCoverLayout(
  comicInfo: ComicInfo | null,
  total: number,
  defaultCoverAlone: boolean
): CoverLayout {
  const fallback = { coverAlone: defaultCoverAlone, coverIndex: 0, coverCount: 1 }
  if (total <= 0) return fallback
  const covers = explicitCoverPages(comicInfo, total)
  if (covers.length > 0) {
    return { coverAlone: true, coverIndex: covers[0], coverCount: covers.length }
  }
  const pages = comicInfo?.pages ?? []
  if (pages.some(isFrontCover)) return fallback
  const first = pages.find((page) => Math.trunc(page.image) === 0)
  if (first?.page_type?.trim()) return { coverAlone: false, coverIndex: 0, coverCount: 1 }
  return fallback
}

/** 지금 단독 표지로 보이는 인덱스 목록 (전역 설정에 따른 기본 0번 포함). */
export function coverPagesOf(layout: CoverLayout): number[] {
  if (!layout.coverAlone) return []
  return Array.from({ length: layout.coverCount }, (_, k) => layout.coverIndex + k)
}

/**
 * 표지가 여러 장이면 각 장이 혼자 한 화면이 되도록 단독 인덱스에 더한다.
 * 한 장일 때는 `coverIndex`만으로 배치가 정해지므로 그대로 돌려준다.
 */
export function withCoverSolo(layout: CoverLayout, solo: SoloIndices): SoloIndices {
  if (!layout.coverAlone || layout.coverCount <= 1) return solo
  return new Set([...solo, ...coverPagesOf(layout)])
}

/**
 * `index` 페이지의 표지 지정을 뒤집은 새 표지 목록.
 * 표지는 연속 구간만 허용한다. 구간에 붙은 페이지를 지정하면 구간이 늘고,
 * 떨어진 페이지를 지정하면 그 페이지만 표지가 된다. 구간 중간을 해제하면
 * 앞쪽 구간만 남는다.
 */
export function toggleCoverPage(covers: readonly number[], index: number): number[] {
  if (covers.includes(index)) {
    const rest = covers.filter((cover) => cover !== index)
    return rest.length === 0 ? [] : contiguousRun(rest, Math.min(...rest))
  }
  const adjacent = covers.includes(index - 1) || covers.includes(index + 1)
  return adjacent ? [...covers, index].sort((a, b) => a - b) : [index]
}
