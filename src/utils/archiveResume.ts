/**
 * 아카이브 이어보기 시작 위치를 정한다.
 * 저장된 엔트리가 실제 목록에 있고 첫 페이지가 아니면 그 위치를, 아니면 0을 돌려준다.
 */
export function resolveArchiveStartIndex(
  images: readonly string[],
  savedEntry: string | null
): number {
  if (!savedEntry) return 0
  const index = images.indexOf(savedEntry)
  return index > 0 ? index : 0
}
