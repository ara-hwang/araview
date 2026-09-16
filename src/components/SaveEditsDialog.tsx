import { useEffect, useState } from "react"
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
import { Field } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { useAppStore } from "@/store/appStore"
import {
  buildSaveEditsPayload,
  describeTransform,
  type SaveEditsPayload,
  type SaveOutputFormat
} from "@/utils/imageEdits"

type SaveEditsDialogProps = {
  open: boolean
  onSubmit: (payload: SaveEditsPayload) => void
  onClose: () => void
}

export function SaveEditsDialog({ open, onSubmit, onClose }: SaveEditsDialogProps) {
  const { t } = useTranslation()
  const rotation = useAppStore((state) => state.rotation)
  const flipH = useAppStore((state) => state.flipH)
  const flipV = useAppStore((state) => state.flipV)
  const fileName = useAppStore((state) => state.imageInfo?.file_name) ?? ""

  const FORMATS: { value: SaveOutputFormat; label: string }[] = [
    { value: "keep", label: t("dialog.save.keep") },
    { value: "png", label: "PNG" },
    { value: "jpg", label: "JPEG" },
    { value: "webp", label: "WebP" }
  ]

  const [format, setFormat] = useState<SaveOutputFormat>("keep")
  const [overwrite, setOverwrite] = useState(true)
  const [newName, setNewName] = useState("")

  useEffect(() => {
    if (!open) return
    setFormat("keep")
    setOverwrite(true)
    setNewName("")
  }, [open])

  const submit = () => {
    onSubmit(buildSaveEditsPayload(rotation, flipH, flipV, format, overwrite, newName))
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose()
      }}
    >
      <DialogContent className="no-drag">
        <DialogHeader>
          <DialogTitle>{t("dialog.save.title")}</DialogTitle>
          <DialogDescription>
            {fileName} ·{" "}
            {describeTransform(
              rotation,
              flipH,
              flipV,
              t as unknown as (key: string, vars?: Record<string, string | number>) => string
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">{t("dialog.save.format")}</span>
          <RadioGroup
            value={format}
            onValueChange={(value) => setFormat(value as SaveOutputFormat)}
          >
            {FORMATS.map((f) => (
              <Field key={f.value} orientation="horizontal">
                <RadioGroupItem value={f.value} id={`save-fmt-${f.value}`} />
                <Label htmlFor={`save-fmt-${f.value}`}>{f.label}</Label>
              </Field>
            ))}
          </RadioGroup>
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">{t("dialog.save.saveAs")}</span>
          <RadioGroup
            value={overwrite ? "overwrite" : "new"}
            onValueChange={(value) => setOverwrite(value === "overwrite")}
          >
            <Field orientation="horizontal">
              <RadioGroupItem value="overwrite" id="save-dest-overwrite" />
              <Label htmlFor="save-dest-overwrite">{t("dialog.save.overwrite")}</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="new" id="save-dest-new" />
              <Label htmlFor="save-dest-new">{t("dialog.save.newFile")}</Label>
            </Field>
          </RadioGroup>
          {!overwrite && (
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder={t("dialog.save.newNamePlaceholder")}
              spellCheck={false}
              autoComplete="off"
              aria-label={t("dialog.save.newNameAria")}
              className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs transition-colors outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
          )}
        </div>

        <p className="text-xs text-muted-foreground">{t("dialog.save.note")}</p>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("dialog.cancel")}
          </Button>
          <Button onClick={submit}>{t("dialog.save.submit")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
