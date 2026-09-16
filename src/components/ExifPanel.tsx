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
import type { ImageDetails } from "@/types"
import { formatDimensions, formatFileSize } from "@/utils/format"
import { formatDpi, formatUnixDateTime } from "@/utils/imageDetails"

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
  const dims = formatDimensions(details.width, details.height)
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
  const colorLabel = t(COLOR_LABEL_KEY[details.color_mode] ?? "details.colorUnknown")
  const colorValue =
    typeof details.bits_per_channel === "number"
      ? `${colorLabel} · ${t("details.bitsPerChannel", {
          bits: details.bits_per_channel
        })}`
      : colorLabel
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
          <span className="truncate text-right font-medium" title={row.value}>
            {row.value}
          </span>
        </div>
      ))}
    </div>
  )
}

export function ExifPanel() {
  const { t, i18n } = useTranslation()
  const {
    exifData,
    exifError,
    histogramData,
    imageDetails,
    archivePath,
    dirImages,
    showExifPanel
  } = useAppStore(
    useShallow((state) => ({
      exifData: state.exifData,
      exifError: state.exifError,
      histogramData: state.histogramData,
      imageDetails: state.imageDetails,
      archivePath: state.archivePath,
      dirImages: state.dirImages,
      showExifPanel: state.showExifPanel
    }))
  )
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
  const panelEntryName = dirImages.images[dirImages.current_index]
  const detailPathLabel =
    imageDetails !== null && isArchivePanel && archivePath !== null && panelEntryName !== undefined
      ? `${archivePath} › ${panelEntryName}`
      : (imageDetails?.file_path ?? "")
  const detailRows =
    imageDetails === null
      ? []
      : buildDetailRows(imageDetails, detailPathLabel, isArchivePanel, translate, locale)
  const hasAnyContent = imageDetails !== null || histogramData !== null || sections.length > 0

  return (
    <Sheet open={showExifPanel} onOpenChange={handleOpenChange}>
      <SheetContent side="right" data-exif-panel className="flex flex-col p-0">
        <SheetHeader className="border-b px-4 py-3">
          <SheetTitle>{t("exif.title")}</SheetTitle>
          <SheetDescription>{t("exif.desc")}</SheetDescription>
        </SheetHeader>

        <ScrollArea className="flex-1 overflow-auto">
          <div className="space-y-4 p-4">
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

            <div>
              <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                {t("histogram.title")}
              </h3>
              {histogramData === null ? (
                <p className="text-sm text-muted-foreground">{t("histogram.unavailable")}</p>
              ) : (
                <HistogramChart data={histogramData} />
              )}
            </div>

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
                    <div className="space-y-1">
                      {section.entries.map(([key, value]) => (
                        <div
                          key={key}
                          className="flex items-baseline justify-between gap-2 text-sm"
                        >
                          <span className="shrink-0 text-muted-foreground">
                            {formatTagName(key)}
                          </span>
                          <span className="truncate text-right font-medium">{value}</span>
                        </div>
                      ))}
                    </div>
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
