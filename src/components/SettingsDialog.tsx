import { useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { GeneralSettingsPanel } from "@/components/settings/GeneralSettingsPanel"
import { ExtensionSettingsPanel } from "@/components/settings/ExtensionSettingsPanel"
import { resetSettings } from "@/store/settingsStore"
import { cn } from "cn"
import { FileTypeIcon, SettingsIcon } from "lucide-react"

type SettingsTab = "general" | "extensions"

type SettingsDialogProps = {
  open: boolean
  onClose: () => void
}

const SIDEBAR_ITEMS: {
  id: SettingsTab
  label: string
  icon: typeof SettingsIcon
}[] = [
  { id: "general", label: "일반", icon: SettingsIcon },
  { id: "extensions", label: "확장자", icon: FileTypeIcon }
]

export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const [tab, setTab] = useState<SettingsTab>("general")

  const close = () => {
    setTab("general")
    onClose()
  }

  const handleOpenChange = (isOpen: boolean) => {
    if (!isOpen) close()
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="no-drag w-[720px] max-w-[calc(100%-2rem)] gap-0 overflow-hidden p-0 sm:max-w-[720px]">
        <div className="flex h-[min(560px,80vh)] flex-col">
          <div className="border-b px-4 py-3 pr-12">
            <DialogTitle>환경 설정</DialogTitle>
            <DialogDescription className="sr-only">
              이미지 뷰어 설정과 파일 확장자 연결을 변경합니다.
            </DialogDescription>
          </div>

          <div className="flex min-h-0 flex-1">
            <aside className="flex w-44 shrink-0 flex-col border-r">
              <nav className="flex flex-1 flex-col gap-1 p-2">
                {SIDEBAR_ITEMS.map((item) => {
                  const Icon = item.icon
                  const active = tab === item.id
                  return (
                    <Button
                      key={item.id}
                      variant="ghost"
                      className={cn(
                        "w-full justify-start",
                        active &&
                          "bg-primary/10 text-primary hover:bg-primary/10 hover:text-primary"
                      )}
                      onClick={() => setTab(item.id)}
                    >
                      <Icon data-icon="inline-start" />
                      {item.label}
                    </Button>
                  )
                })}
              </nav>
              <Separator />
              <div className="p-2">
                <Button
                  variant="ghost"
                  className="w-full"
                  onClick={() => void resetSettings()}
                >
                  초기화
                </Button>
              </div>
            </aside>

            <div className="flex min-w-0 flex-1 flex-col">
              <ScrollArea className="h-full min-h-0 flex-1">
                <div className="p-4">
                  {tab === "general" ? (
                    <>
                      <h3 className="mb-4 text-base font-medium">일반 설정</h3>
                      <GeneralSettingsPanel />
                    </>
                  ) : (
                    <ExtensionSettingsPanel
                      active={open && tab === "extensions"}
                    />
                  )}
                </div>
              </ScrollArea>
              <div className="flex justify-end border-t p-3">
                <Button onClick={close}>확인</Button>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
