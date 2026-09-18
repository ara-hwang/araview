import { CaretDown } from "@phosphor-icons/react"
import { createRootRoute, Link, Outlet } from "@tanstack/react-router"
import { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { CommandPalette } from "@/components/CommandPalette"
import { ExifPanel } from "@/components/ExifPanel"
import Header from "@/components/Header"
import { SettingsDialog } from "@/components/SettingsDialog"
import { StatusBar } from "@/components/StatusBar"
import { ThemeProvider } from "@/components/theme-provider"
import { Separator } from "@/components/ui/separator"
import { Toaster } from "@/components/ui/toast"
import { UpdateDialogs } from "@/components/UpdateDialogs"
import { OPEN_SETTINGS_EVENT } from "@/hooks/useCommandPalette"
import { useIdleHide } from "@/hooks/useIdleHide"
import { useOpenFileBridge } from "@/hooks/useOpenFileListener"
import { useUpdateCheckRequestListener } from "@/hooks/useUpdater"
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore, updateSettings } from "@/store/settingsStore"

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: NotFound
})

function NotFound() {
  const { t } = useTranslation()
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 p-8 text-center">
      <p className="text-sm font-medium">{t("notFound.title")}</p>
      <p className="text-sm text-muted-foreground">{t("notFound.desc")}</p>
      <Link
        to="/"
        className="mt-2 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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
  // 설정 다이얼로그는 헤더 안에 두면 상단바 숨김과 함께 언마운트되어
  // 닫힘 + 재오픈 불가 버그가 생기므로 루트에서 마운트한다.
  const [settingsOpen, setSettingsOpen] = useState(false)
  useEffect(() => {
    const openSettings = () => setSettingsOpen(true)
    window.addEventListener(OPEN_SETTINGS_EVENT, openSettings)
    return () => {
      window.removeEventListener(OPEN_SETTINGS_EVENT, openSettings)
    }
  }, [])
  useEffect(() => {
    if (!menuBarHidden) setMenuBarPeek(false)
  }, [menuBarHidden])
  const menuBarCollapsed = menuBarHidden && !menuBarPeek
  const menuBarOverlay = menuBarHidden && menuBarPeek

  // 다이얼로그/Sheet 오버레이가 헤더를 가리지 않도록 헤더 높이를 CSS
  // 변수로 노출한다. Portal이 body에 렌더되므로 셸이 아닌 documentElement에
  // 설정한다. 접힘 상태에서는 헤더가 없으므로 0px.
  const headerRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    if (menuBarCollapsed) {
      document.documentElement.style.setProperty("--header-height", "0px")
      return
    }
    const el = headerRef.current
    if (!el) return
    const update = () => {
      const h = el.getBoundingClientRect().height
      document.documentElement.style.setProperty("--header-height", `${Math.round(h)}px`)
    }
    update()
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [menuBarCollapsed, menuBarOverlay, chromeHidden])

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
            ref={headerRef}
            data-header-root
            data-tauri-drag-region
            onMouseEnter={() => setMenuBarPeek(true)}
            className="group absolute inset-x-0 top-0 z-40 flex h-5 items-start justify-center"
          >
            <button
              type="button"
              onClick={() => void updateSettings({ menuBarHidden: false })}
              title={t("header.showMenuBar")}
              aria-label={t("header.showMenuBar")}
              className="rounded-b-md border border-t-0 bg-background px-2 py-0.5 text-muted-foreground opacity-0 shadow-md transition-opacity no-drag group-hover:opacity-100 hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none [&_svg]:pointer-events-none"
            >
              <CaretDown aria-hidden="true" />
            </button>
          </div>
        ) : menuBarOverlay ? (
          <div
            ref={headerRef}
            data-header-root
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
            ref={headerRef}
            data-header-root
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
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <UpdateDialogs />
      <Toaster />
    </ThemeProvider>
  )
}
