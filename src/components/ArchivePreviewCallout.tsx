import { BookOpen } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { basenameOf } from "@/utils/statusBar"

type ArchivePreviewCalloutProps = {
  archivePath: string
  onOpenArchive: () => void
}

/** 폴더 탐색 중 아카이브 미리보기 안내. 캔버스 클릭·방향키는 막지 않는다. */
export function ArchivePreviewCallout({ archivePath, onOpenArchive }: ArchivePreviewCalloutProps) {
  const { t } = useTranslation()
  const name = basenameOf(archivePath)

  return (
    <div
      className="pointer-events-none absolute inset-x-0 bottom-28 z-20 flex justify-center px-4"
      aria-live="polite"
    >
      <div className="pointer-events-auto flex max-w-md flex-col gap-2 rounded-md border border-border bg-background/95 px-3 py-2 shadow-lg backdrop-blur-sm sm:flex-row sm:items-center sm:gap-3">
        <p className="text-sm text-muted-foreground">{t("viewer.archivePreview.hint", { name })}</p>
        <Button
          type="button"
          size="sm"
          className="shrink-0"
          onClick={onOpenArchive}
          title={t("viewer.archivePreview.openTitle")}
        >
          <BookOpen data-icon="inline-start" />
          {t("viewer.archivePreview.open")}
        </Button>
      </div>
    </div>
  )
}
