export type ImageInfo = {
  file_path: string
  mime_type: string
  file_name: string
  file_size: number
  width: number | null
  height: number | null
}

export type DirectoryImages = {
  images: string[]
  current_index: number
}

export type ThumbnailInfo = {
  file_path: string
  width: number
  height: number
}

export type ExifData = Record<string, string>

export type ArchiveState = {
  /** 현재 열린 아카이브 파일의 절대 경로 (null이면 일반 모드) */
  archivePath: string | null
}

export type FileAssociation = {
  extension: string
  associated: boolean
  current_prog_id: string | null
  needs_os_confirmation: boolean
}
