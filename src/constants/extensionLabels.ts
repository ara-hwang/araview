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
  | "avif"
  | "heic"
  | "heif"
  | "psd"
  | "tga"
  | "dds"
  | "exr"
  | "qoi"
  | "cbz"
  | "zip"

const EXTENSION_I18N_KEYS: Record<ExtensionLabelKey, string> = {
  png: "ext.png",
  jpg: "ext.jpg",
  jpeg: "ext.jpeg",
  gif: "ext.gif",
  bmp: "ext.bmp",
  webp: "ext.webp",
  svg: "ext.svg",
  ico: "ext.ico",
  avif: "ext.avif",
  heic: "ext.heic",
  heif: "ext.heif",
  psd: "ext.psd",
  tga: "ext.tga",
  dds: "ext.dds",
  exr: "ext.exr",
  qoi: "ext.qoi",
  cbz: "ext.cbz",
  zip: "ext.zip"
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
  avif: "AVIF 이미지",
  heic: "HEIC 이미지",
  heif: "HEIF 이미지",
  psd: "PSD 이미지",
  tga: "TGA 이미지",
  dds: "DDS 텍스처",
  exr: "OpenEXR 이미지",
  qoi: "QOI 이미지",
  cbz: "만화 아카이브",
  zip: "ZIP 아카이브"
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
