import { useEffect, useRef, useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { useTranslation } from "react-i18next"

type RenameDialogProps = {
  open: boolean
  initialName: string
  onSubmit: (newName: string) => void
  onClose: () => void
}

export function RenameDialog({
  open,
  initialName,
  onSubmit,
  onClose
}: RenameDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState(initialName)
  const inputRef = useRef<HTMLInputElement>(null)

  // 열릴 때마다 현재 파일명으로 초기화 + 확장자 제외 선택
  useEffect(() => {
    if (!open) return
    setName(initialName)
    const dot = initialName.lastIndexOf(".")
    requestAnimationFrame(() => {
      inputRef.current?.focus()
      inputRef.current?.setSelectionRange(0, dot > 0 ? dot : initialName.length)
    })
  }, [open, initialName])

  const submit = () => {
    const trimmed = name.trim()
    if (!trimmed || trimmed === initialName) {
      onClose()
      return
    }
    onSubmit(trimmed)
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
          <DialogTitle>{t("dialog.rename.title")}</DialogTitle>
          <DialogDescription>{t("dialog.rename.desc")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <Label htmlFor="rename-input">{t("dialog.rename.label")}</Label>
          <input
            ref={inputRef}
            id="rename-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit()
            }}
            spellCheck={false}
            autoComplete="off"
            className="border-input focus-visible:ring-ring flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs transition-colors outline-none focus-visible:ring-1"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("dialog.cancel")}
          </Button>
          <Button
            onClick={submit}
            disabled={!name.trim() || name.trim() === initialName}
          >
            {t("dialog.rename.submit")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
