import type { ViewMode } from "@/store/settingsStore"
import type { ComicInfo } from "@/types"

/**
 * ComicInfo `Manga` 값에서 읽기 방향을 읽는다.
 * `YesAndRightToLeft`만 우→좌, `No`는 좌→우로 확정하고 나머지(`Yes`, `Unknown`, 없음)는
 * 방향을 알 수 없으므로 null이다.
 */
export function comicReadingDirection(
  comicInfo: ComicInfo | null
): "left-to-right" | "right-to-left" | null {
  const manga = comicInfo?.manga?.trim().toLowerCase()
  if (manga === "yesandrighttoleft") return "right-to-left"
  if (manga === "no") return "left-to-right"
  return null
}

/**
 * 아카이브를 열 때 쓸 보기 모드. 자동 양쪽 보기가 꺼져 있거나 웹툰을 고른 경우는
 * null(= 설정값 그대로)이다. 방향은 ComicInfo > 설정의 양쪽 방향 > 좌→우 순으로 정한다.
 */
export function resolveComicViewMode(
  base: ViewMode,
  autoDualView: boolean,
  comicInfo: ComicInfo | null
): ViewMode | null {
  if (!autoDualView || base === "webtoon") return null
  const fallback = base === "right-to-left" ? "right-to-left" : "left-to-right"
  return comicReadingDirection(comicInfo) ?? fallback
}
