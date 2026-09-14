import { createRootRoute, Link, Outlet } from "@tanstack/react-router"
import { useEffect, useState } from "react"
import { CaretDown } from "@phosphor-icons/react"
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
import { useOpenFileBridge } from "@/hooks/useOpenFileListener"
import { useUpdateCheckRequestListener } from "@/hooks/useUpdater"
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
  const { t } = useTranslation()
  // UI 자동 숨김은 이미지를 보고 있을 때만 적용 (홈에서는 항상 표시)
  const hasImage = useAppStore((state) => state.imageInfo !== null)
  const autoHideUI = useSettingsStore((state) => state.autoHideUI)
  const menuBarHidden = useSettingsStore((state) => state.menuBarHidden)
  const chromeHidden = useIdleHide(autoHideUI && hasImage)

  // 상단바 수동 숨김: 접힌 상태에서는 상단 호버 영역에서만 peek으로 표시
  const [menuBarPeek, setMenuBarPeek] = useState(false)
  useEffect(() => {
    if (!menuBarHidden) setMenuBarPeek(false)
  }, [menuBarHidden])
  const menuBarCollapsed = menuBarHidden && !menuBarPeek
  const menuBarOverlay = menuBarHidden && menuBarPeek

  // OS 파일 연결/두 번째 실행으로 열린 파일을 현재 라우트 로더로 전달한다.
  useOpenFileBridge()
  // 명령 팔레트의 업데이트 확인 요청을 실제 확인으로 연결한다.
  useUpdateCheckRequestListener()

  return (
    <ThemeProvider>
      {/* 앱 셸은 절대 문서 스크롤되지 않는다. 스크롤은 각 라우트 안에서 처리. */}
      <div className="relative flex h-screen w-full flex-col overflow-hidden">
        {menuBarCollapsed ? (
          <div
            data-tauri-drag-region
            onMouseEnter={() => setMenuBarPeek(true)}
            className="group absolute inset-x-0 top-0 z-40 flex h-5 items-start justify-center"
          >
            <button
              type="button"
              onClick={() => setMenuBarPeek(true)}
              title={t("header.showMenuBar")}
              aria-label={t("header.showMenuBar")}
              className="bg-background text-muted-foreground hover:text-foreground rounded-b-md border border-t-0 px-2 py-0.5 opacity-0 shadow-md transition-opacity group-hover:opacity-100 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:outline-none"
            >
              <CaretDown aria-hidden="true" />
            </button>
          </div>
        ) : menuBarOverlay ? (
          <div
            onMouseLeave={() => setMenuBarPeek(false)}
            onBlur={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
                setMenuBarPeek(false)
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") setMenuBarPeek(false)
            }}
            className="absolute inset-x-0 top-0 z-40 shadow-md"
          >
            <Header onHideMenuBar={() => setMenuBarPeek(false)} />
            <Separator />
          </div>
        ) : (
          <div
            className={cn(
              "transition-opacity duration-300",
              chromeHidden && "pointer-events-none opacity-0"
            )}
          >
            <Header onHideMenuBar={() => setMenuBarPeek(false)} />
            <Separator />
          </div>
        )}
        {/* min-h-0: flex 자식이 콘텐츠 높이만큼 커져 셸을 밀어내지 않게 한다 */}
        <div className="relative min-h-0 flex-1 overflow-hidden">
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
