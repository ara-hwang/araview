import { CaretDown } from "@phosphor-icons/react"
import { createRootRoute, Link, Outlet } from "@tanstack/react-router"
import { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { AppTooltip } from "@/components/AppTooltip"
import { CommandPalette } from "@/components/CommandPalette"
import { ExifPanel } from "@/components/ExifPanel"
import Header from "@/components/Header"
import { SettingsDialog } from "@/components/SettingsDialog"
import { StatusBar } from "@/components/StatusBar"
import { ThemeProvider } from "@/components/theme-provider"
import { Separator } from "@/components/ui/separator"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
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

// peek 닫힘 지연. 메뉴 호버의 관례적인 히스테리시스(150ms): 의도적 이탈은
// 바로 닫히는 느낌을 유지하면서 스쳐 지나감은 애니메이션 시작 전에 흡수한다.
const PEEK_CLOSE_DELAY_MS = 150

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
  // 상단바 수동 숨김도 이미지를 볼 때만 적용한다. 빈 화면에서까지 숨기면
  // 헤더가 파일 열기/드래그 앤 드롭 동선 자체를 가려 버린다.
  const menuBarHiddenActive = menuBarHidden && hasImage
  // Snap Layouts 네이티브 오버레이는 최대화 버튼 위에만 존재한다. 헤더가
  // opacity로만 가려지는 동안은 버튼 rect가 유효해 오버레이가 남으므로,
  // 헤더가 완전히 보이지 않는 동안은 명시적으로 떼어낸다 (peek 중에는 다시 붙인다).
  const snapOverlayEnabled = !menuBarHiddenActive && !chromeHidden

  // 상단바 수동 숨김: 접힌 상태에서는 상단 호버 영역에서만 peek으로 표시
  const [menuBarPeek, setMenuBarPeek] = useState(false)
  // peek 닫힘 지연 타이머. 헤더가 내려오는 동안(히트 영역이 이동 중) 근처에서
  // 마우스를 빠르게 움직이면 leave/enter가 연속으로 발생해 애니메이션이
  // 중간에 뒤집히며 끊겨 보인다. 열기는 즉시, 닫기만 짧게 늦춰 스침은 무시한다.
  const peekCloseTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => {
    return () => {
      if (peekCloseTimer.current) clearTimeout(peekCloseTimer.current)
    }
  }, [])
  const cancelPeekClose = () => {
    if (peekCloseTimer.current) {
      clearTimeout(peekCloseTimer.current)
      peekCloseTimer.current = null
    }
  }
  const openPeek = () => {
    cancelPeekClose()
    setMenuBarPeek(true)
  }
  const schedulePeekClose = () => {
    cancelPeekClose()
    peekCloseTimer.current = setTimeout(() => {
      peekCloseTimer.current = null
      setMenuBarPeek(false)
    }, PEEK_CLOSE_DELAY_MS)
  }
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
    if (!menuBarHiddenActive) {
      if (peekCloseTimer.current) {
        clearTimeout(peekCloseTimer.current)
        peekCloseTimer.current = null
      }
      setMenuBarPeek(false)
    }
  }, [menuBarHiddenActive])
  const menuBarCollapsed = menuBarHiddenActive && !menuBarPeek
  const menuBarOverlay = menuBarHiddenActive && menuBarPeek

  // 다이얼로그/Sheet 오버레이가 헤더를 가리지 않도록 헤더 높이를 CSS
  // 변수로 노출한다. Portal이 body에 렌더되므로 셸이 아닌 documentElement에
  // 설정한다. 접힘 상태에서는 헤더가 없으므로 0px.
  const headerRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const root = document.documentElement
    const apply = () => {
      if (menuBarCollapsed) {
        root.style.setProperty("--header-height", "0px")
        return
      }
      const el = headerRef.current
      if (!el) return
      const h = el.getBoundingClientRect().height
      root.style.setProperty("--header-height", `${Math.round(h)}px`)
    }
    apply()
    const el = headerRef.current
    if (!el) return
    const ro = new ResizeObserver(apply)
    ro.observe(el)
    return () => ro.disconnect()
  }, [menuBarCollapsed, menuBarOverlay, chromeHidden])

  // OS 파일 연결/두 번째 실행으로 열린 파일을 현재 라우트 로더로 전달한다.
  useOpenFileBridge()
  // 명령 팔레트의 업데이트 확인 요청을 실제 확인으로 연결한다.
  useUpdateCheckRequestListener()

  return (
    <ThemeProvider>
      <TooltipProvider delay={300}>
        {/* 앱 셸은 절대 문서 스크롤되지 않는다. 스크롤은 각 라우트 안에서 처리. */}
        <div className="relative flex h-screen w-full flex-col overflow-hidden">
          {/* 상단바 수동 숨김: 접힘 상태에서는 상단 가장자리 호버 영역만 남긴다 */}
          {menuBarHiddenActive ? (
            <div
              data-tauri-drag-region
              onMouseEnter={openPeek}
              className="group absolute inset-x-0 top-0 z-40 flex h-5 items-start justify-center"
            >
              <AppTooltip content={t("header.showMenuBar")}>
                <button
                  type="button"
                  onClick={() => void updateSettings({ menuBarHidden: false })}
                  aria-label={t("header.showMenuBar")}
                  className="rounded-b-md border border-t-0 bg-background px-2 py-0.5 text-muted-foreground opacity-0 shadow-md transition-opacity no-drag group-hover:opacity-100 hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none [&_svg]:pointer-events-none"
                >
                  <CaretDown aria-hidden="true" />
                </button>
              </AppTooltip>
            </div>
          ) : null}

          {/* 상단바는 마운트를 유지한 채 높이(0fr)와 슬라이드로 접고 편다. 접히는
              동안 내용이 위로 빠져나가고, peek 중에는 흐름 높이 없이 읽기 영역
              위로 겹쳐 내려온다. */}
          <div
            data-header-root
            className={cn(
              "grid shrink-0 transition-[grid-template-rows] duration-200 ease-motion-out motion-reduce:transition-none",
              menuBarHiddenActive ? "grid-rows-[0fr]" : "grid-rows-[1fr]"
            )}
          >
            {/* min-h-0: 0fr 트랙에서도 내용 높이가 트랙을 밀어내지 않게 한다 */}
            <div className="min-h-0">
              <div
                ref={headerRef}
                onMouseEnter={openPeek}
                onMouseLeave={() => {
                  if (menuBarPeek) schedulePeekClose()
                }}
                onBlur={(e) => {
                  if (menuBarPeek && !e.currentTarget.contains(e.relatedTarget as Node | null)) {
                    cancelPeekClose()
                    setMenuBarPeek(false)
                  }
                }}
                inert={menuBarCollapsed}
                className={cn(
                  "transition-[translate,opacity] duration-200 ease-motion-out motion-reduce:transition-none",
                  menuBarCollapsed && "pointer-events-none -translate-y-full opacity-0",
                  menuBarOverlay && "relative z-40 shadow-md",
                  !menuBarHiddenActive && chromeHidden && "pointer-events-none opacity-0"
                )}
              >
                <Header
                  onHideMenuBar={() => setMenuBarPeek(false)}
                  snapOverlayEnabled={snapOverlayEnabled}
                />
                <Separator />
              </div>
            </div>
          </div>
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
      </TooltipProvider>
    </ThemeProvider>
  )
}
