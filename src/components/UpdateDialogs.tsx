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
import { dismissUpdate, relaunchAfterUpdate, startUpdateDownload } from "@/hooks/useUpdater"
import { useUpdateStore } from "@/store/updateStore"

export function UpdateDialogs() {
  const { t } = useTranslation()
  const stage = useUpdateStore((s) => s.stage)
  const version = useUpdateStore((s) => s.version)
  const body = useUpdateStore((s) => s.body)
  const pct = useUpdateStore((s) => s.pct)
  const error = useUpdateStore((s) => s.error)

  const open = stage !== "idle"
  const downloading = stage === "downloading" && !error

  const handleOpenChange = (next: boolean) => {
    if (next) return
    if (stage === "downloading" && !error) return
    dismissUpdate(false)
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent data-testid="update-dialog" showCloseButton={!downloading}>
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
              <Button
                variant="outline"
                data-testid="update-later"
                onClick={() => dismissUpdate(false)}
              >
                {t("dialog.update.later")}
              </Button>
              <Button data-testid="update-download" onClick={() => void startUpdateDownload()}>
                {t("dialog.update.download")}
              </Button>
            </DialogFooter>
          </>
        )}

        {stage === "downloading" && (
          <>
            <DialogHeader>
              <DialogTitle>{t("dialog.update.downloadingTitle")}</DialogTitle>
              <DialogDescription>
                {t("dialog.update.downloadingDesc", {
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
                aria-label={t("dialog.update.downloadingTitle")}
                data-testid="update-progress"
                className="h-2 w-full overflow-hidden rounded-full bg-muted"
              >
                <div
                  className={
                    pct === null
                      ? "h-full w-1/3 animate-pulse rounded-full bg-primary"
                      : "h-full w-full origin-left rounded-full bg-primary transition-transform duration-200 ease-linear"
                  }
                  style={pct === null ? undefined : { transform: `scaleX(${pct / 100})` }}
                />
              </div>
              <p className="text-sm text-muted-foreground">
                {error
                  ? error
                  : pct === null
                    ? t("dialog.update.downloading")
                    : t("dialog.update.downloadingPct", { pct })}
              </p>
              {error && (
                <p role="alert" className="text-sm text-destructive">
                  {t("dialog.update.downloadFail")}
                </p>
              )}
            </div>
            {error && (
              <DialogFooter>
                <Button variant="outline" onClick={() => dismissUpdate(false)}>
                  {t("dialog.cancel")}
                </Button>
                <Button data-testid="update-retry" onClick={() => void startUpdateDownload()}>
                  {t("dialog.update.retry")}
                </Button>
              </DialogFooter>
            )}
          </>
        )}

        {stage === "ready" && (
          <>
            <DialogHeader>
              <DialogTitle>{t("dialog.update.readyTitle")}</DialogTitle>
              <DialogDescription>{t("dialog.update.readyDesc")}</DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button
                variant="outline"
                data-testid="update-apply-on-exit"
                onClick={() => dismissUpdate(true)}
              >
                {t("dialog.update.applyOnExit")}
              </Button>
              <Button data-testid="update-restart-now" onClick={() => void relaunchAfterUpdate()}>
                {t("dialog.update.restartNow")}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
