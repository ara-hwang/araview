/** `\`와 `/`를 모두 구분자로 쓰는 Windows 경로 분할. 빈 조각 제거. */
function splitPathSegments(path: string): string[] {
  return path.split(/[\\/]+/).filter((seg) => seg.length > 0)
}

/** 경로의 마지막 조각. 없으면 빈 문자열. */
export function basenameOf(path: string): string {
  const segs = splitPathSegments(path)
  return segs.length > 0 ? (segs[segs.length - 1] as string) : ""
}

/** 경로의 부모 폴더명. 없으면 null. */
export function parentFolderNameOf(path: string): string | null {
  const segs = splitPathSegments(path)
  if (segs.length < 2) return null
  return segs[segs.length - 2] as string
}
