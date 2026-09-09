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
          <DialogTitle>이름 변경</DialogTitle>
          <DialogDescription>
            같은 폴더 안에서 확장자 포함 전체 파일명을 입력하세요.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <Label htmlFor="rename-input">파일 이름</Label>
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
            취소
          </Button>
          <Button
            onClick={submit}
            disabled={!name.trim() || name.trim() === initialName}
          >
            변경
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
