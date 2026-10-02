import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { useAppStore } from "@/store/appStore"
import { formatDimensions, formatFileSize } from "@/utils/format"
import { buildStatusModel } from "@/utils/statusBar"

/** "폴더명 /" 앞부분. 좁은 창(sm 미만)에서는 숨긴다. */
function FolderSegment({ name, maxWidth }: { name: string; maxWidth: "max-w-32" | "max-w-40" }) {
  return (
    <>
      <span
        className={`hidden ${maxWidth} min-w-0 shrink truncate text-muted-foreground sm:inline`}
      >
        {name}
      </span>
      <span aria-hidden="true" className="hidden shrink-0 text-muted-foreground sm:inline">
        /
      </span>
    </>
  )
}

export function StatusBar() {
  const { t } = useTranslation()
  const { imageInfo, zoom, imageSize, dirImages, archivePath } = useAppStore(
    useShallow((state) => ({
      imageInfo: state.imageInfo,
      zoom: state.zoom,
      imageSize: state.imageSize,
      dirImages: state.dirImages,
      archivePath: state.archivePath
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

  // temp 경로(sidecar/추출물) 노출 방지: 폴더/파일명은 dirImages와
  // archivePath에서만 도출하고 imageInfo.file_path는 쓰지 않는다.
  const model = buildStatusModel({
    archivePath,
    currentEntry: dirImages.images[dirImages.current_index],
    fallbackFileName: imageInfo?.file_name ?? null
  })

  // 빈 화면(홈)에서는 상태바 문구가 불필요하므로 바 자체를 렌더하지 않는다.
  if (model.kind === "empty") {
    return null
  }

  return (
    <div
      className="absolute bottom-0 flex h-6 w-full items-center justify-between gap-2 border-t bg-background px-2 text-xs"
      role="status"
      aria-label={t("status.barLabel")}
    >
      <span
        className="flex min-w-0 flex-1 items-baseline gap-1 overflow-hidden whitespace-nowrap"
        title={model.tooltip}
      >
        {model.kind === "archive" ? (
          <>
            {model.folderName && <FolderSegment name={model.folderName} maxWidth="max-w-32" />}
            <span className="max-w-48 min-w-0 shrink-0 truncate font-medium">
              {model.archiveName}
            </span>
            {model.entryName && (
              <>
                <span aria-hidden="true" className="shrink-0 text-muted-foreground">
                  ›
                </span>
                <span className="min-w-0 flex-1 truncate">{model.entryName}</span>
              </>
            )}
          </>
        ) : (
          <>
            {model.folderName && <FolderSegment name={model.folderName} maxWidth="max-w-40" />}
            <span className="min-w-0 flex-1 truncate font-medium">{model.fileName}</span>
          </>
        )}
      </span>
      <span className="shrink-0 font-mono tabular-nums">{tokens.join(" | ")}</span>
    </div>
  )
}
