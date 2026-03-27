import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription
} from "@/components/ui/sheet"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useAppStore } from "@/store/appStore"
import { useShallow } from "zustand/react/shallow"

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

function formatTagName(tag: string): string {
  return tag.replace(/([A-Z])/g, " $1").trim()
}

export function ExifPanel() {
  const { exifData, showExifPanel } = useAppStore(
    useShallow((state) => ({
      exifData: state.exifData,
      showExifPanel: state.showExifPanel
    }))
  )

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

    const otherEntries = Object.entries(exifData).filter(
      ([key]) => !categorized.has(key)
    )
    if (otherEntries.length > 0) {
      sections.push({ title: "Other", entries: otherEntries })
    }
  }

  return (
    <Sheet open={showExifPanel} onOpenChange={handleOpenChange}>
      <SheetContent side="right" className="flex flex-col p-0">
        <SheetHeader className="border-b px-4 py-3">
          <SheetTitle>EXIF Information</SheetTitle>
          <SheetDescription>
            Image metadata and camera settings
          </SheetDescription>
        </SheetHeader>

        <ScrollArea className="flex-1 overflow-auto">
          <div className="p-4">
            {!exifData || sections.length === 0 ? (
              <p className="text-muted-foreground text-sm">
                No EXIF data available for this image.
              </p>
            ) : (
              <div className="space-y-4">
                {sections.map((section) => (
                  <div key={section.title}>
                    <h3 className="text-muted-foreground mb-2 text-xs font-semibold tracking-wider uppercase">
                      {section.title}
                    </h3>
                    <div className="space-y-1">
                      {section.entries.map(([key, value]) => (
                        <div
                          key={key}
                          className="flex items-baseline justify-between gap-2 text-sm"
                        >
                          <span className="text-muted-foreground shrink-0">
                            {formatTagName(key)}
                          </span>
                          <span className="truncate text-right font-medium">
                            {value}
                          </span>
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
