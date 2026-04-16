// 파일 연결(tauri.conf.json fileAssociations) 및 백엔드(image.rs)와 동기화할 것.

export const SUPPORTED_IMAGE_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "bmp",
  "webp",
  "svg",
  "ico",
  "tiff",
  "tif",
  "avif",
  "cbz"
] as const

export type SupportedImageExtension =
  (typeof SUPPORTED_IMAGE_EXTENSIONS)[number]
