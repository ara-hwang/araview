import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { HistogramChart } from "@/components/HistogramChart"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription
} from "@/components/ui/sheet"
import { useExifLoader } from "@/hooks/useExifLoader"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo, ImageDetails } from "@/types"
import { comicReadingDirection } from "@/utils/comicViewMode"
import { formatDimensions, formatFileSize } from "@/utils/format"
import { formatDpi, formatUnixDateTime } from "@/utils/imageDetails"
import { isSvgImageInfo } from "@/utils/imageRendering"

const EXIF_CATEGORIES: Record<string, string[]> = {
  Camera: ["Make", "Model", "LensModel", "LensMake", "BodySerialNumber"],
  Exposure: [
    "ExposureTime",
    "FNumber",
    "PhotographicSensitivity",
    "ISOSpeedRatings",
    "ExposureBiasValue",
    "ExposureProgram",
    "ExposureMode",
    "MeteringMode",
    "Flash",
    "WhiteBalance"
  ],
  Image: [
    "ImageWidth",
    "ImageLength",
    "PixelXDimension",
    "PixelYDimension",
    "Orientation",
    "ColorSpace",
    "BitsPerSample",
    "Compression"
  ],
  Lens: ["FocalLength", "FocalLengthIn35mmFilm", "MaxApertureValue"],
  DateTime: [
    "DateTime",
    "DateTimeOriginal",
    "DateTimeDigitized",
    "OffsetTime",
    "OffsetTimeOriginal"
  ],
  GPS: [
    "GPSLatitude",
    "GPSLatitudeRef",
    "GPSLongitude",
    "GPSLongitudeRef",
    "GPSAltitude",
    "GPSAltitudeRef"
  ],
  Software: ["Software", "ProcessingSoftware", "ImageDescription", "Copyright"]
}

const COLOR_LABEL_KEY: Record<string, string> = {
  rgb: "details.colorRgb",
  rgba: "details.colorRgba",
  grayscale: "details.colorGrayscale",
  "grayscale-alpha": "details.colorGrayscaleAlpha"
}

function formatTagName(tag: string): string {
  return tag.replace(/([A-Z])/g, " $1").trim()
}

type DetailRow = { label: string; value: string; key: string }

/** 파일 상세 → 표시 행. 아카이브 모드에서는 추출물 날짜를 숨긴다. */
function buildDetailRows(
  details: ImageDetails,
  pathLabel: string,
  isArchive: boolean,
  isVector: boolean,
  isArchiveFile: boolean,
  t: (key: string, vars?: Record<string, string | number>) => string,
  locale: string | undefined
): DetailRow[] {
  const rows: DetailRow[] = [
    { key: "path", label: t("details.path"), value: pathLabel },
    {
      key: "size",
      label: t("details.size"),
      value: formatFileSize(details.file_size, locale)
    }
  ]
  const dims = isArchiveFile ? null : formatDimensions(details.width, details.height)
  if (dims !== null) {
    const pixels =
      typeof details.width === "number" &&
      typeof details.height === "number" &&
      details.width > 0 &&
      details.height > 0
        ? ` · ${t("details.pixels", {
            count: new Intl.NumberFormat(locale).format(details.width * details.height)
          })}`
        : ""
    rows.push({
      key: "dimensions",
      label: t("details.dimensions"),
      value: `${dims}${pixels}`
    })
  }
  if (!isArchive) {
    const created = formatUnixDateTime(details.created_unix, locale)
    if (created !== null) {
      rows.push({ key: "created", label: t("details.created"), value: created })
    }
    const modified = formatUnixDateTime(details.modified_unix, locale)
    if (modified !== null) {
      rows.push({
        key: "modified",
        label: t("details.modified"),
        value: modified
      })
    }
  }
  if (isArchiveFile) return rows
  const colorValue = isVector
    ? t("details.colorVector")
    : (() => {
        const colorLabel = t(COLOR_LABEL_KEY[details.color_mode] ?? "details.colorUnknown")
        return typeof details.bits_per_channel === "number"
          ? `${colorLabel} · ${t("details.bitsPerChannel", {
              bits: details.bits_per_channel
            })}`
          : colorLabel
      })()
  rows.push({ key: "color", label: t("details.color"), value: colorValue })
  const dpi = formatDpi(details.dpi_x, details.dpi_y)
  if (dpi !== null) {
    rows.push({ key: "dpi", label: t("details.dpi"), value: dpi })
  }
  rows.push({
    key: "icc",
    label: t("details.icc"),
    value: describeIcc(details, t, locale)
  })
  return rows
}

function describeIcc(
  details: ImageDetails,
  t: (key: string, vars?: Record<string, string | number>) => string,
  locale: string | undefined
): string {
  if (details.icc_status === "present") {
    const size =
      typeof details.icc_bytes === "number" ? formatFileSize(details.icc_bytes, locale) : "?"
    return details.icc_name
      ? t("details.iccPresent", { name: details.icc_name, size })
      : t("details.iccPresentNoName", { size })
  }
  return t(details.icc_status === "absent" ? "details.iccAbsent" : "details.iccUnchecked")
}

function DetailRows({ rows }: { rows: DetailRow[] }) {
  return (
    <div className="space-y-1">
      {rows.map((row) => (
        <div key={row.key} className="flex items-baseline justify-between gap-2 text-sm">
          <span className="shrink-0 text-muted-foreground">{row.label}</span>
          <span className="truncate text-right font-medium tabular-nums" title={row.value}>
            {row.value}
          </span>
        </div>
      ))}
    </div>
  )
}

/** `Year`/`Month`/`Day` → `2024-03-05` 형태. 연도가 없으면 null, 월·일은 있는 만큼만 붙인다. */
function formatComicDate(comic: ComicInfo): string | null {
  if (comic.year === null) return null
  const parts = [String(comic.year)]
  if (comic.month !== null) {
    parts.push(String(comic.month).padStart(2, "0"))
    if (comic.day !== null) parts.push(String(comic.day).padStart(2, "0"))
  }
  return parts.join("-")
}

/** ComicInfo → 표시 행. 값이 있는 필드만 순서대로 담는다. */
function buildComicRows(
  comic: ComicInfo,
  t: (key: string, vars?: Record<string, string | number>) => string
): DetailRow[] {
  const rows: DetailRow[] = []
  const push = (key: string, labelKey: string, value: string | number | null) => {
    if (value === null || value === "") return
    rows.push({ key, label: t(labelKey), value: String(value) })
  }
  push("writer", "comic.writer", comic.writer)
  push("penciller", "comic.penciller", comic.penciller)
  push("inker", "comic.inker", comic.inker)
  push("colorist", "comic.colorist", comic.colorist)
  push("letterer", "comic.letterer", comic.letterer)
  push("coverArtist", "comic.coverArtist", comic.cover_artist)
  push("editor", "comic.editor", comic.editor)
  push("publisher", "comic.publisher", comic.publisher)
  push("published", "comic.published", formatComicDate(comic))
  const direction = comicReadingDirection(comic)
  push(
    "direction",
    "comic.direction",
    direction === null
      ? null
      : t(direction === "right-to-left" ? "comic.directionRtl" : "comic.directionLtr")
  )
  push("genre", "comic.genre", comic.genre)
  push("tags", "comic.tags", comic.tags)
  push("volume", "comic.volume", comic.volume)
  push("count", "comic.count", comic.count)
  push("pageCount", "comic.pageCount", comic.page_count)
  push("language", "comic.language", comic.language_iso)
  push("ageRating", "comic.ageRating", comic.age_rating)
  push("rating", "comic.rating", comic.community_rating)
  return rows
}

/** `Series #Number` 조합. 없는 부분은 건너뛰고 둘 다 없으면 빈 문자열이다. */
function comicHeadline(comic: ComicInfo): string {
  return [comic.series, comic.number ? `#${comic.number}` : null]
    .filter((part): part is string => Boolean(part))
    .join(" ")
}

export function ExifPanel() {
  const { t, i18n } = useTranslation()
  const {
    exifData,
    exifError,
    histogramData,
    imageDetails,
    imageInfo,
    archivePath,
    archivePreviewPath,
    comicInfo,
    comicInfoError,
    dirImages,
    showExifPanel
  } = useAppStore(
    useShallow((state) => ({
      exifData: state.exifData,
      exifError: state.exifError,
      histogramData: state.histogramData,
      imageDetails: state.imageDetails,
      imageInfo: state.imageInfo,
      archivePath: state.archivePath,
      archivePreviewPath: state.archivePreviewPath,
      comicInfo: state.comicInfo,
      comicInfoError: state.comicInfoError,
      dirImages: state.dirImages,
      showExifPanel: state.showExifPanel
    }))
  )
  const showComicInfo = useSettingsStore((state) => state.showComicInfo)
  const { reloadExif } = useExifLoader()
  const locale = i18n.language === "ko" ? "ko-KR" : "en-US"
  const translate = t as unknown as (key: string, vars?: Record<string, string | number>) => string

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      useAppStore.setState({ showExifPanel: false })
    }
  }

  const categorized = new Set<string>()
  const sections: { title: string; entries: [string, string][] }[] = []

  if (exifData) {
    for (const [category, tags] of Object.entries(EXIF_CATEGORIES)) {
      const entries: [string, string][] = []
      for (const tag of tags) {
        if (exifData[tag] != null) {
          entries.push([tag, exifData[tag]])
          categorized.add(tag)
        }
      }
      if (entries.length > 0) {
        sections.push({ title: category, entries })
      }
    }

    const otherEntries = Object.entries(exifData).filter(([key]) => !categorized.has(key))
    if (otherEntries.length > 0) {
      sections.push({ title: t("exif.other"), entries: otherEntries })
    }
  }

  const isArchivePanel = archivePath !== null
  const isArchiveFilePanel = archivePreviewPath !== null
  const isVectorImage = isSvgImageInfo(imageInfo)
  const panelEntryName = dirImages.images[dirImages.current_index]
  const detailPathLabel =
    imageDetails !== null && isArchivePanel && archivePath !== null && panelEntryName !== undefined
      ? `${archivePath} › ${panelEntryName}`
      : (imageDetails?.file_path ?? "")
  const detailRows =
    imageDetails === null
      ? []
      : buildDetailRows(
          imageDetails,
          detailPathLabel,
          isArchivePanel,
          isVectorImage,
          isArchiveFilePanel,
          translate,
          locale
        )
  // 아카이브 모드에서만, 설정이 켜져 있고, 읽은 메타데이터나 에러가 있을 때만 쓴다.
  const showComicSection =
    (isArchivePanel || isArchiveFilePanel) &&
    showComicInfo &&
    (comicInfo !== null || comicInfoError !== null)
  const comicRows = comicInfo === null ? [] : buildComicRows(comicInfo, translate)
  const comicHeading = comicInfo === null ? "" : comicHeadline(comicInfo)
  const hasAnyContent =
    imageDetails !== null || histogramData !== null || sections.length > 0 || showComicSection

  return (
    // modal="trap-focus": 타이틀바를 덮는 투명 전체화면 백드롭을 없앤다 (App.css 주석 참고)
    <Sheet open={showExifPanel} onOpenChange={handleOpenChange} modal="trap-focus">
      <SheetContent side="right" data-exif-panel className="flex flex-col p-0">
        <SheetHeader className="border-b px-4 py-3">
          <SheetTitle>{t("exif.title")}</SheetTitle>
          <SheetDescription>{t("exif.desc")}</SheetDescription>
        </SheetHeader>

        <ScrollArea className="flex-1 overflow-auto">
          <div className="space-y-4 p-4">
            {showComicSection && (
              <div>
                <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  {t("comic.section")}
                </h3>
                {comicInfoError !== null ? (
                  <div className="space-y-1">
                    <p className="text-sm text-muted-foreground">{t("comic.loadFail")}</p>
                    <p className="text-xs break-words text-muted-foreground">{comicInfoError}</p>
                  </div>
                ) : (
                  comicInfo !== null && (
                    <div className="space-y-3">
                      {(comicHeading !== "" || comicInfo.title !== null) && (
                        <div className="space-y-1">
                          {comicHeading !== "" && (
                            <p className="text-sm font-medium">{comicHeading}</p>
                          )}
                          {comicInfo.title !== null && (
                            <p className="text-sm text-muted-foreground">{comicInfo.title}</p>
                          )}
                        </div>
                      )}
                      {comicRows.length > 0 && <DetailRows rows={comicRows} />}
                      {comicInfo.summary !== null && (
                        <div className="space-y-1">
                          <p className="text-sm text-muted-foreground">{t("comic.summary")}</p>
                          <p className="text-sm whitespace-pre-wrap">{comicInfo.summary}</p>
                        </div>
                      )}
                    </div>
                  )
                )}
              </div>
            )}

            <div>
              <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                {t("details.title")}
              </h3>
              {imageDetails === null ? (
                <p className="text-sm text-muted-foreground">{t("details.unavailable")}</p>
              ) : (
                <DetailRows rows={detailRows} />
              )}
            </div>

            {!isArchiveFilePanel && (
              <div>
                <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  {t("histogram.title")}
                </h3>
                {histogramData === null ? (
                  <p className="text-sm text-muted-foreground">
                    {t(isVectorImage ? "histogram.vectorUnavailable" : "histogram.unavailable")}
                  </p>
                ) : (
                  <HistogramChart data={histogramData} />
                )}
              </div>
            )}

            {exifError ? (
              <div className="flex flex-col gap-2">
                <p className="text-sm text-muted-foreground">{t("exif.loadFail")}</p>
                <p className="text-xs break-words text-muted-foreground">{exifError}</p>
                <div>
                  <Button size="sm" variant="outline" onClick={() => void reloadExif()}>
                    {t("exif.retry")}
                  </Button>
                </div>
              </div>
            ) : !exifData || sections.length === 0 ? (
              !hasAnyContent && <p className="text-sm text-muted-foreground">{t("exif.empty")}</p>
            ) : (
              <div className="space-y-4">
                {sections.map((section) => (
                  <div key={section.title}>
                    <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                      {section.title}
                    </h3>
                    <DetailRows
                      rows={section.entries.map(([key, value]) => ({
                        key,
                        label: formatTagName(key),
                        value
                      }))}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  )
}
