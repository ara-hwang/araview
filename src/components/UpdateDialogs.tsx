import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import { dismissUpdate, startUpdateDownload } from "@/hooks/useUpdater"
import { useUpdateStore } from "@/store/updateStore"
import { formatFileSize } from "@/utils/format"

export function UpdateDialogs() {
  const { t } = useTranslation()
  const stage = useUpdateStore((s) => s.stage)
  const version = useUpdateStore((s) => s.version)
  const body = useUpdateStore((s) => s.body)
  const pct = useUpdateStore((s) => s.pct)
  const downloadedBytes = useUpdateStore((s) => s.downloadedBytes)
  const totalBytes = useUpdateStore((s) => s.totalBytes)
  const error = useUpdateStore((s) => s.error)

  const open = stage !== "idle"
  const installing = stage === "installing"
  const busy = stage === "downloading" || installing
  // 다운로드/설치 인계 중에는 에러가 없으면 닫기를 막는다.
  const dismissBlocked = busy && !error

  const handleOpenChange = (next: boolean) => {
    if (next) return
    if (dismissBlocked) return
    dismissUpdate()
  }

  const busyTitle = installing
    ? t("dialog.update.installingTitle")
    : t("dialog.update.downloadingTitle")

  const bytesLine =
    totalBytes === null || pct === null
      ? t("dialog.update.downloadedBytes", { size: formatFileSize(downloadedBytes) })
      : t("dialog.update.downloadedOfTotal", {
          downloaded: formatFileSize(downloadedBytes),
          total: formatFileSize(totalBytes),
          pct
        })

  return (
    // modal="trap-focus": 타이틀바를 덮는 투명 전체화면 백드롭을 없앤다 (App.css 주석 참고)
    <Dialog open={open} onOpenChange={handleOpenChange} modal="trap-focus">
      <DialogContent data-testid="update-dialog" showCloseButton={!dismissBlocked}>
        {stage === "available" && (
          <>
            <DialogHeader>
              <DialogTitle>
                {t("dialog.update.availableTitle", { version: version ?? "" })}
              </DialogTitle>
              <DialogDescription>{t("dialog.update.availableDesc")}</DialogDescription>
            </DialogHeader>
            <div className="rounded-md border">
              <ScrollArea className="max-h-40">
                <p className="p-3 text-sm whitespace-pre-wrap text-muted-foreground">
                  {body?.trim() ? body : t("dialog.update.noNotes")}
                </p>
              </ScrollArea>
            </div>
            <DialogFooter>
              <Button variant="outline" data-testid="update-later" onClick={() => dismissUpdate()}>
                {t("dialog.update.later")}
              </Button>
              <Button data-testid="update-download" onClick={() => void startUpdateDownload()}>
                {t("dialog.update.download")}
              </Button>
            </DialogFooter>
          </>
        )}

        {busy && (
          <>
            <DialogHeader>
              <DialogTitle>{busyTitle}</DialogTitle>
              <DialogDescription>
                {installing
                  ? t("dialog.update.installingDesc")
                  : t("dialog.update.downloadingDesc", {
                      version: version ?? ""
                    })}
              </DialogDescription>
            </DialogHeader>
            <div className="flex flex-col gap-2">
              <div
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={pct ?? undefined}
                aria-label={busyTitle}
                data-testid="update-progress"
                className="h-2 w-full overflow-hidden rounded-full bg-muted"
              >
                <div
                  className={
                    pct === null
                      ? "h-full w-1/3 animate-progress-indeterminate rounded-full bg-primary"
                      : "h-full w-full origin-left rounded-full bg-primary transition-transform duration-200 ease-linear"
                  }
                  style={pct === null ? undefined : { transform: `scaleX(${pct / 100})` }}
                />
              </div>
              {error ? (
                <p className="text-sm text-muted-foreground">{error}</p>
              ) : (
                !installing && (
                  <p className="text-sm text-muted-foreground">{t("dialog.update.downloading")}</p>
                )
              )}
              {!error && !installing && downloadedBytes > 0 && (
                <p
                  data-testid="update-bytes"
                  className="text-sm text-muted-foreground tabular-nums"
                >
                  {bytesLine}
                </p>
              )}
              {error && (
                <p role="alert" className="text-sm text-destructive">
                  {installing ? t("dialog.update.installFail") : t("dialog.update.downloadFail")}
                </p>
              )}
            </div>
            {error && (
              <DialogFooter>
                <Button variant="outline" onClick={() => dismissUpdate()}>
                  {t("dialog.cancel")}
                </Button>
                <Button data-testid="update-retry" onClick={() => void startUpdateDownload()}>
                  {t("dialog.update.retry")}
                </Button>
              </DialogFooter>
            )}
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
