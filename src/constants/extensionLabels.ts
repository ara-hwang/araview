import {
  SUPPORTED_IMAGE_EXTENSIONS,
  type SupportedImageExtension
} from "@/constants/imageExtensions"
import i18n from "@/i18n"

type ExtensionLabelKey =
  | "png"
  | "jpg"
  | "jpeg"
  | "gif"
  | "bmp"
  | "webp"
  | "svg"
  | "ico"
  | "tiff"
  | "tif"
  | "avif"
  | "heic"
  | "heif"
  | "psd"
  | "cbz"
  | "cb7"
  | "cbr"
  | "rar"
  | "zip"
  | "7z"
  | "cbt"

const EXTENSION_I18N_KEYS: Record<ExtensionLabelKey, string> = {
  png: "ext.png",
  jpg: "ext.jpg",
  jpeg: "ext.jpeg",
  gif: "ext.gif",
  bmp: "ext.bmp",
  webp: "ext.webp",
  svg: "ext.svg",
  ico: "ext.ico",
  tiff: "ext.tiff",
  tif: "ext.tif",
  avif: "ext.avif",
  heic: "ext.heic",
  heif: "ext.heif",
  psd: "ext.psd",
  cbz: "ext.cbz",
  cb7: "ext.cb7",
  cbr: "ext.cbr",
  rar: "ext.rar",
  zip: "ext.zip",
  "7z": "ext.7z",
  cbt: "ext.cbt"
}

/** @deprecated Use `extensionLabel()` which follows the active language. */
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
  psd: "PSD 이미지",
  cbz: "만화 아카이브",
  cb7: "만화 아카이브",
  cbr: "만화 아카이브",
  rar: "RAR 아카이브",
  zip: "ZIP 아카이브",
  "7z": "7z 아카이브",
  cbt: "만화 아카이브"
}

export function extensionLabel(ext: string): string {
  if (isSupportedExtension(ext)) {
    const tx = i18n.t as unknown as (key: string) => string
    return tx(EXTENSION_I18N_KEYS[ext])
  }
  return ext.toUpperCase()
}

export function isSupportedExtension(ext: string): ext is SupportedImageExtension {
  return (SUPPORTED_IMAGE_EXTENSIONS as readonly string[]).includes(ext)
}
