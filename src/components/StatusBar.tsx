import { useAppStore } from "@/store/appStore"
import { formatDimensions, formatFileSize } from "../utils/format"
import { useShallow } from "zustand/react/shallow"
import { useTranslation } from "react-i18next"

export function StatusBar() {
  const { t } = useTranslation()
  const { imageInfo, zoom, imageSize, dirImages } = useAppStore(
    useShallow((state) => ({
      imageInfo: state.imageInfo,
      zoom: state.zoom,
      imageSize: state.imageSize,
      dirImages: state.dirImages
    }))
  )

  const backendDims = formatDimensions(imageInfo?.width, imageInfo?.height)
  const fallbackDims =
    imageSize.width > 0 && imageSize.height > 0
      ? formatDimensions(imageSize.width, imageSize.height)
      : null
  const dims = backendDims ?? fallbackDims

  const tokens: string[] = []
  if (dims) tokens.push(dims)
  if (imageInfo) tokens.push(`${Math.round(zoom * 100)}%`)
  if (dirImages.images.length > 0) {
    tokens.push(`${dirImages.current_index + 1}/${dirImages.images.length}`)
  }
  if (imageInfo) tokens.push(formatFileSize(imageInfo.file_size))

  return (
    <div
      className="bg-background absolute bottom-0 flex h-6 w-full items-center justify-between gap-2 border-t px-2 text-xs"
      role="status"
      aria-label={t("status.barLabel")}
    >
      <span className="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
        {imageInfo?.file_name ?? t("status.empty")}
      </span>
      <span className="shrink-0 font-mono tabular-nums">
        {tokens.length > 0 ? tokens.join(" | ") : t("status.unknown")}
      </span>
    </div>
  )
}
