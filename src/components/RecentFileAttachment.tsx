import { FileArchive, FileImage, FileX, X } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"

import {
  Attachment,
  AttachmentAction,
  AttachmentActions,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle,
  AttachmentTrigger
} from "@/components/ui/attachment"
import { Spinner } from "@/components/ui/spinner"
import type { RecentFileStatus } from "@/hooks/useRecentFileDetails"
import type { ImageInfo } from "@/types"
import { formatDimensions, formatFileSize } from "@/utils/format"

type RecentFileAttachmentProps = {
  path: string
  info?: ImageInfo
  src?: string
  status: RecentFileStatus
  onOpen: (path: string) => void
  onRemove: (path: string) => void
}

export function parentDir(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "")
  const idx = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"))
  return idx > 0 ? normalized.slice(0, idx) : normalized
}

export function extOf(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path
  const dot = base.lastIndexOf(".")
  if (dot < 0 || dot === base.length - 1) return ""
  return base.slice(dot + 1).toUpperCase()
}

export function RecentFileAttachment({
  path,
  info,
  src,
  status,
  onOpen,
  onRemove
}: RecentFileAttachmentProps) {
  const { t, i18n } = useTranslation()
  const name = info?.file_name ?? path.split(/[\\/]/).pop() ?? path
  const isArchive = info ? info.mime_type.startsWith("application/") : false
  const showImage = status === "done" && !!src && !isArchive

  const meta =
    status === "done" && info
      ? [
          extOf(path),
          formatFileSize(info.file_size, i18n.language),
          formatDimensions(info.width, info.height)
        ]
          .filter((part): part is string => !!part)
          .join(" · ")
      : status === "loading"
        ? t("home.list.loading")
        : t("home.list.unavailable")

  return (
    <Attachment state={status === "error" ? "error" : status === "loading" ? "processing" : "done"}>
      <AttachmentMedia variant={showImage ? "image" : "icon"}>
        {showImage && src ? (
          <img
            src={src}
            alt=""
            loading="lazy"
            draggable={false}
            onError={(e) => {
              ;(e.currentTarget as HTMLImageElement).style.visibility = "hidden"
            }}
          />
        ) : status === "loading" ? (
          <Spinner />
        ) : status === "error" ? (
          <FileX />
        ) : isArchive ? (
          <FileArchive />
        ) : (
          <FileImage />
        )}
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle title={path}>{name}</AttachmentTitle>
        <AttachmentDescription>{meta}</AttachmentDescription>
        <AttachmentDescription className="text-muted-foreground/70">
          <span title={path} className="block truncate">
            {parentDir(path)}
          </span>
        </AttachmentDescription>
      </AttachmentContent>
      <AttachmentActions>
        <AttachmentAction
          aria-label={`${t("home.card.remove")}, ${name}`}
          title={t("home.card.remove")}
          onClick={() => onRemove(path)}
        >
          <X />
        </AttachmentAction>
      </AttachmentActions>
      <AttachmentTrigger
        aria-label={t("home.list.open", { name })}
        title={path}
        onClick={() => onOpen(path)}
      />
    </Attachment>
  )
}
