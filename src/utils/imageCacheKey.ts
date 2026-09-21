/** 아카이브 엔트리명과 디스크 경로가 겹치지 않도록 캐시 키를 만든다. */
export function imageCacheKey(archivePath: string | null, pathOrEntry: string): string {
  if (!archivePath) return pathOrEntry
  return `${archivePath}\u0000${pathOrEntry}`
}
