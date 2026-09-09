import { useEffect, useState } from "react"
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
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Field } from "@/components/ui/field"
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

const FORMATS: { value: SaveOutputFormat; label: string }[] = [
  { value: "keep", label: "원본 유지" },
  { value: "png", label: "PNG" },
  { value: "jpg", label: "JPEG" },
  { value: "webp", label: "WebP" }
]

export function SaveEditsDialog({
  open,
  onSubmit,
  onClose
}: SaveEditsDialogProps) {
  const rotation = useAppStore((state) => state.rotation)
  const flipH = useAppStore((state) => state.flipH)
  const flipV = useAppStore((state) => state.flipV)
  const fileName = useAppStore((state) => state.imageInfo?.file_name) ?? ""

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
    onSubmit(
      buildSaveEditsPayload(rotation, flipH, flipV, format, overwrite, newName)
    )
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
          <DialogTitle>편집 내용 저장</DialogTitle>
          <DialogDescription>
            {fileName} · {describeTransform(rotation, flipH, flipV)}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">포맷</span>
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
          <span className="text-sm font-medium">저장 방식</span>
          <RadioGroup
            value={overwrite ? "overwrite" : "new"}
            onValueChange={(value) => setOverwrite(value === "overwrite")}
          >
            <Field orientation="horizontal">
              <RadioGroupItem value="overwrite" id="save-dest-overwrite" />
              <Label htmlFor="save-dest-overwrite">원본에 덮어쓰기</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="new" id="save-dest-new" />
              <Label htmlFor="save-dest-new">새 파일로 저장</Label>
            </Field>
          </RadioGroup>
          {!overwrite && (
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="비우면 자동으로 이름 생성 (확장자는 포맷 기준)"
              spellCheck={false}
              autoComplete="off"
              aria-label="새 파일 이름"
              className="border-input focus-visible:ring-ring flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs transition-colors outline-none focus-visible:ring-1"
            />
          )}
        </div>

        <p className="text-muted-foreground text-xs">
          포맷 변경·HEIC·GIF 등은 항상 새 파일로 저장됩니다. GIF는 첫 프레임만
          저장됩니다.
        </p>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            취소
          </Button>
          <Button onClick={submit}>저장</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
