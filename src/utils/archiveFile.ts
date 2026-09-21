const ARCHIVE_EXTENSIONS = new Set(["cbz", "cb7", "cbr", "rar", "zip", "7z", "cbt"])

/** 로컬 아카이브 파일 경로 여부 (폴더 목록·썸네일 분기용) */
export function isArchiveFilePath(filePath: string): boolean {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? ""
  return ARCHIVE_EXTENSIONS.has(ext)
}
