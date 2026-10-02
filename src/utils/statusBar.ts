import { basenameOf, parentFolderNameOf } from "@/utils/path"

export type StatusBarModel =
  | { kind: "empty" }
  | {
      kind: "file"
      folderName: string | null
      fileName: string
      tooltip: string
    }
  | {
      kind: "archive"
      folderName: string | null
      archiveName: string
      entryName: string | null
      tooltip: string
    }

type BuildStatusInput = {
  archivePath: string | null
  /** 일반 모드면 현재 이미지 절대 경로, 아카이브 모드면 현재 엔트리 상대 경로 */
  currentEntry: string | undefined
  fallbackFileName: string | null
}

/**
 * 상태바 표시 모델을 만든다.
 * 원본 경로(`dirImages` 항목, `archivePath`)만 쓰고
 * HEIC sidecar/아카이브 추출물 같은 temp 경로는 절대 쓰지 않는다.
 */
export function buildStatusModel(input: BuildStatusInput): StatusBarModel {
  const { archivePath, currentEntry, fallbackFileName } = input

  if (archivePath) {
    const archiveName = basenameOf(archivePath) || archivePath
    const folderName = parentFolderNameOf(archivePath)
    if (!currentEntry) {
      return {
        kind: "archive",
        folderName,
        archiveName,
        entryName: null,
        tooltip: archivePath
      }
    }
    return {
      kind: "archive",
      folderName,
      archiveName,
      entryName: currentEntry,
      tooltip: `${archivePath} › ${currentEntry}`
    }
  }

  if (currentEntry) {
    const fileName = basenameOf(currentEntry) || currentEntry
    return {
      kind: "file",
      folderName: parentFolderNameOf(currentEntry),
      fileName,
      tooltip: currentEntry
    }
  }

  if (fallbackFileName) {
    return {
      kind: "file",
      folderName: null,
      fileName: fallbackFileName,
      tooltip: fallbackFileName
    }
  }

  return { kind: "empty" }
}
