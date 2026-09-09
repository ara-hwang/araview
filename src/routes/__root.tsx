import { createRootRoute, Outlet } from "@tanstack/react-router"
import { Toaster } from "@/components/ui/sonner"
import { ThemeProvider } from "@/components/theme-provider"
import Header from "@/components/Header"
import { Separator } from "@/components/ui/separator"
import { StatusBar } from "@/components/StatusBar"
import { ExifPanel } from "@/components/ExifPanel"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { useIdleHide } from "@/hooks/useIdleHide"
import { cn } from "@/lib/utils"

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: () => <div>Not Found</div>
})

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
      <Toaster position="bottom-right" richColors closeButton />
    </ThemeProvider>
  )
}
