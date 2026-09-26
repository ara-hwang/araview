import {
  ArrowCounterClockwise,
  ArrowsDownUp,
  Eye,
  FileText,
  Gauge,
  Gear,
  Keyboard
} from "@phosphor-icons/react"
import { confirm } from "@tauri-apps/plugin-dialog"
import { cn } from "cn"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import { ExtensionSettingsPanel } from "@/components/settings/ExtensionSettingsPanel"
import { GeneralTabPanel } from "@/components/settings/GeneralTabPanel"
import { ListTabPanel } from "@/components/settings/ListTabPanel"
import { PerformanceTabPanel } from "@/components/settings/PerformanceTabPanel"
import { ShortcutsTabPanel } from "@/components/settings/ShortcutsTabPanel"
import { ViewTabPanel } from "@/components/settings/ViewTabPanel"
import { useTheme } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { resetSettings } from "@/store/settingsStore"

type SettingsTab = "general" | "view" | "list" | "performance" | "shortcuts" | "extensions"

type SettingsDialogProps = {
  open: boolean
  onClose: () => void
}

export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const { t } = useTranslation()
  const [tab, setTab] = useState<SettingsTab>("general")
  const { setTheme } = useTheme()

  const SIDEBAR_ITEMS: {
    id: SettingsTab
    label: string
    icon: typeof Gear
  }[] = [
    { id: "general", label: t("settings.tabs.general"), icon: Gear },
    { id: "view", label: t("settings.tabs.view"), icon: Eye },
    { id: "list", label: t("settings.tabs.list"), icon: ArrowsDownUp },
    {
      id: "performance",
      label: t("settings.tabs.performance"),
      icon: Gauge
    },
    {
      id: "shortcuts",
      label: t("settings.tabs.shortcuts"),
      icon: Keyboard
    },
    {
      id: "extensions",
      label: t("settings.tabs.extensions"),
      icon: FileText
    }
  ]

  const TAB_TITLES: Record<Exclude<SettingsTab, "extensions">, string> = {
    general: t("settings.general.title"),
    view: t("settings.viewTab.title"),
    list: t("settings.listTab.title"),
    performance: t("settings.performanceTab.title"),
    shortcuts: t("settings.shortcutsTab.title")
  }

  const close = () => {
    setTab("general")
    onClose()
  }

  const handleOpenChange = (isOpen: boolean) => {
    if (!isOpen) close()
  }

  const handleReset = async () => {
    const ok = await confirm(t("confirm.resetSettings.message"), {
      title: t("confirm.resetSettings.title"),
      kind: "warning"
    }).catch(() => false)
    if (!ok) return
    await resetSettings()
    setTheme("system")
  }

  return (
    // modal="trap-focus": Base UI의 투명 전체화면 백드롭이 타이틀바까지 덮어
    // 창 제어(최소화/최대화/닫기)를 가로채는 것을 막는다. 포커스 트랩과 dimmed
    // 영역 클릭 닫기는 유지된다 (App.css 하단 주석 참고).
    <Dialog open={open} onOpenChange={handleOpenChange} modal="trap-focus">
      <DialogContent className="w-[840px] max-w-[calc(100vw-2rem)] gap-0 overflow-hidden p-0 no-drag sm:max-w-[calc(100vw-2rem)]">
        <div className="flex h-[min(640px,86vh)] flex-col">
          <div className="border-b px-4 py-3 pr-12">
            <DialogTitle>{t("settings.title")}</DialogTitle>
            <DialogDescription className="sr-only">{t("settings.desc")}</DialogDescription>
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
                  className="w-full justify-start hover:bg-destructive/10 hover:text-destructive dark:hover:bg-destructive/20"
                  onClick={() => void handleReset()}
                >
                  <ArrowCounterClockwise data-icon="inline-start" />
                  {t("settings.reset")}
                </Button>
              </div>
            </aside>

            <div className="flex min-w-0 flex-1 flex-col">
              <ScrollArea key={tab} className="h-full min-h-0 flex-1">
                <div className="p-4">
                  {tab === "extensions" ? (
                    <ExtensionSettingsPanel active={open && tab === "extensions"} />
                  ) : (
                    <>
                      <h3 className="mb-4 text-base font-medium">{TAB_TITLES[tab]}</h3>
                      {tab === "general" ? (
                        <GeneralTabPanel />
                      ) : tab === "view" ? (
                        <ViewTabPanel />
                      ) : tab === "list" ? (
                        <ListTabPanel />
                      ) : tab === "shortcuts" ? (
                        <ShortcutsTabPanel />
                      ) : (
                        <PerformanceTabPanel />
                      )}
                    </>
                  )}
                </div>
              </ScrollArea>
              <div className="flex justify-end border-t p-3">
                <Button onClick={close}>{t("settings.confirm")}</Button>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
