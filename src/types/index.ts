export type ImageInfo = {
  /** WebView가 디코드할 경로. HEIC/PSD/축소본은 sidecar일 수 있다. */
  file_path: string
  /** 사용자가 연 원본 파일 경로. 항상 sidecar가 아니다(파일 작업용). */
  source_path: string
  mime_type: string
  file_name: string
  file_size: number
  width: number | null
  height: number | null
}

export type PixelArtClassification = "pixel_art" | "continuous" | "uncertain"

export type PixelArtDetection = {
  classification: PixelArtClassification
  confidence: number
  pixel_scale: number | null
  method: "runs" | "edges" | "hybrid" | "unsupported"
}

export type FileAvailability = "local" | "cloud_only" | "unknown"

export type DirectoryImages = {
  images: string[]
  current_index: number
  /** Same order as `images`. Empty for archive listings. */
  availability: FileAvailability[]
}

export type ThumbnailInfo = {
  file_path: string
  width: number
  height: number
}

export type CacheStorageMode = "temporary" | "persistent"

export type CacheCategoryKey = "thumbnails" | "converted" | "scaled" | "archives" | "other"

export type CacheCategoryStats = {
  key: CacheCategoryKey
  bytes: number
  file_count: number
  protected_bytes: number
  protected_file_count: number
  limit_bytes: number | null
}

export type CacheStats = {
  storage_mode: CacheStorageMode
  persistent_available: boolean
  total_bytes: number
  file_count: number
  protected_bytes: number
  protected_file_count: number
  total_limit_bytes: number
  categories: CacheCategoryStats[]
}

export type CacheScope = CacheCategoryKey | "all"

export type CacheClearResult = {
  removed_bytes: number
  removed_file_count: number
  failed_file_count: number
  stats: CacheStats
}

export type ExifData = Record<string, string>

/** ComicInfo.xml의 Page 요소. `image`는 ComicRack 스키마대로 0 기반이다. */
export type ComicPage = {
  image: number
  page_type: string | null
}

/** 아카이브 안의 ComicInfo.xml 메타데이터 (읽기 전용, 표시용). */
export type ComicInfo = {
  title: string | null
  series: string | null
  number: string | null
  count: number | null
  volume: number | null
  summary: string | null
  writer: string | null
  penciller: string | null
  publisher: string | null
  genre: string | null
  tags: string | null
  language_iso: string | null
  page_count: number | null
  age_rating: string | null
  community_rating: string | null
  /** `Unknown` | `No` | `Yes` | `YesAndRightToLeft` 원문 (읽기 방향 판정용) */
  manga: string | null
  pages: ComicPage[] | null
}

export type ImageHistogram = {
  r: number[]
  g: number[]
  b: number[]
  sampled_pixels: number
}

export type IccStatus = "present" | "absent" | "unchecked"

export type ImageDetails = {
  file_path: string
  file_size: number
  width: number | null
  height: number | null
  color_mode: string
  bits_per_channel: number | null
  created_unix: number | null
  modified_unix: number | null
  dpi_x: number | null
  dpi_y: number | null
  icc_status: IccStatus
  icc_name: string | null
  icc_bytes: number | null
}

export type ArchiveState = {
  /** 현재 열린 아카이브 파일의 절대 경로 (null이면 일반 모드) */
  archivePath: string | null
}

export type LicenseDocument = {
  name: string
  content: string
}

export type LicenseFile = {
  name: string
  /** `LicenseBundle.texts`의 키 */
  id: string
}

export type LicensePackage = {
  ecosystem: "rust" | "npm"
  name: string
  version: string
  license: string
  files: LicenseFile[]
  /** 저작권자가 비어 있는 표준 템플릿 본문만 있는 패키지 */
  templated?: boolean
}

export type LicenseBundle = {
  documents: LicenseDocument[]
  packages: LicensePackage[]
  texts: Record<string, string>
}

export type FileAssociation = {
  extension: string
  associated: boolean
  current_prog_id: string | null
  needs_os_confirmation: boolean
}

export type PsdThumbStatus = {
  registered: boolean
  clsid: string
  dll_path: string
  dll_exists: boolean
}
