import {
  SUPPORTED_IMAGE_EXTENSIONS,
  type SupportedImageExtension
} from "@/constants/imageExtensions"

export const EXTENSION_LABELS: Record<SupportedImageExtension, string> = {
  png: "PNG 이미지",
  jpg: "JPEG 이미지",
  jpeg: "JPEG 이미지",
  gif: "GIF 이미지",
  bmp: "BMP 이미지",
  webp: "WebP 이미지",
  svg: "SVG 이미지",
  ico: "아이콘",
  tiff: "TIFF 이미지",
  tif: "TIFF 이미지",
  avif: "AVIF 이미지",
  heic: "HEIC 이미지",
  heif: "HEIF 이미지",
  cbz: "만화 아카이브"
}

export function extensionLabel(ext: string): string {
  if (isSupportedExtension(ext)) {
    return EXTENSION_LABELS[ext]
  }
  return ext.toUpperCase()
}

export function isSupportedExtension(
  ext: string
): ext is SupportedImageExtension {
  return (SUPPORTED_IMAGE_EXTENSIONS as readonly string[]).includes(ext)
}
