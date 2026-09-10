import { createRootRoute, Link, Outlet } from "@tanstack/react-router"
import { Toaster } from "@/components/ui/sonner"
import { ThemeProvider } from "@/components/theme-provider"
import Header from "@/components/Header"
import { Separator } from "@/components/ui/separator"
import { StatusBar } from "@/components/StatusBar"
import { ExifPanel } from "@/components/ExifPanel"
import { CommandPalette } from "@/components/CommandPalette"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { useIdleHide } from "@/hooks/useIdleHide"
import { cn } from "@/lib/utils"
import { useTranslation } from "react-i18next"

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: NotFound
})

function NotFound() {
  const { t } = useTranslation()
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 p-8 text-center">
      <p className="text-sm font-medium">{t("notFound.title")}</p>
      <p className="text-muted-foreground text-sm">{t("notFound.desc")}</p>
      <Link
        to="/"
        className="bg-primary text-primary-foreground mt-2 rounded-md px-3 py-1.5 text-sm focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:outline-none"
      >
        {t("notFound.home")}
      </Link>
    </div>
  )
}

function RootLayout() {
  // UI 자동 숨김은 이미지를 보고 있을 때만 적용 (홈에서는 항상 표시)
  const hasImage = useAppStore((state) => state.imageInfo !== null)
  const autoHideUI = useSettingsStore((state) => state.autoHideUI)
  const chromeHidden = useIdleHide(autoHideUI && hasImage)

  return (
    <ThemeProvider>
      <div className="flex h-screen w-screen flex-col">
        <div
          className={cn(
            "transition-opacity duration-300",
            chromeHidden && "pointer-events-none opacity-0"
          )}
        >
          <Header />
          <Separator />
        </div>
        <div className="relative flex-1">
          <Outlet />
          <div
            className={cn(
              "transition-opacity duration-300",
              chromeHidden && "pointer-events-none opacity-0"
            )}
          >
            <StatusBar />
          </div>
        </div>
      </div>
      <ExifPanel />
      <CommandPalette />
      <Toaster position="bottom-right" richColors closeButton />
    </ThemeProvider>
  )
}
