import { useEffect, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  ArrowsHorizontal,
  ArrowsOut,
  ArrowsVertical,
  ArrowClockwise,
  ArrowCounterClockwise,
  Command,
  Copy,
  FlipHorizontal,
  FlipVertical,
  FolderOpen,
  Gear,
  House,
  Info,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Minus,
  PushPin,
  Square,
  X
} from "@phosphor-icons/react"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "./ui/button-group"
import { ButtonGroupText } from "./ui/button-group"
import { Separator } from "./ui/separator"
import { cn } from "@/lib/utils"
import {
  flipHorizontal,
  flipVertical,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  zoomIn,
  zoomOut,
  useAppStore
} from "@/store/appStore"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useCloseImage } from "@/hooks/useCloseImage"
import { useExifLoader } from "@/hooks/useExifLoader"
import { useAlwaysOnTop } from "@/hooks/useAlwaysOnTop"
import { SettingsDialog } from "@/components/SettingsDialog"
import { OPEN_SETTINGS_EVENT, usePaletteStore } from "@/hooks/useCommandPalette"
import { useSettingsStore } from "@/store/settingsStore"
import { formatShortcutDisplay } from "@/constants/shortcuts"
import { useTranslation } from "react-i18next"

// 위쪽 툴바
export default function Header() {
  const { t } = useTranslation()
  const [appWindow] = useState(() => getCurrentWindow())
  const zoom = useAppStore((state) => state.zoom)
  const hasImage = useAppStore((state) => state.imageInfo !== null)
  const shortcuts = useSettingsStore((state) => state.shortcuts)
  const withShortcut = (label: string, binding: string) =>
    binding ? `${label} (${formatShortcutDisplay(binding)})` : label
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [isMaximized, setIsMaximized] = useState(false)

  // 명령 팔레트의 "설정 열기" 실행에 반응
  useEffect(() => {
    const openSettings = () => setSettingsOpen(true)
    window.addEventListener(OPEN_SETTINGS_EVENT, openSettings)
    return () => {
      window.removeEventListener(OPEN_SETTINGS_EVENT, openSettings)
    }
  }, [])

  const { handleOpenFile } = useImageLoader()
  const handleGoHome = useCloseImage()
  const { toggleExifPanel } = useExifLoader()
  const { alwaysOnTop, toggle: toggleAlwaysOnTop } = useAlwaysOnTop()

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined

    const syncMaximized = () => {
      void appWindow
        .isMaximized()
        .then((value) => {
          if (!cancelled) setIsMaximized(value)
        })
        .catch(() => {})
    }

    syncMaximized()
    void appWindow
      .onResized(syncMaximized)
      .then((fn) => {
        unlisten = fn
      })
      .catch(() => {})

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [appWindow])

  const handleMinimize = () => {
    void appWindow.minimize()
  }

  const handleMaximize = () => {
    void (async () => {
      try {
        await appWindow.toggleMaximize()
        setIsMaximized(await appWindow.isMaximized())
      } catch {
        // jsdom/테스트 환경에서는 Tauri 창 API가 없으므로 무시
      }
    })()
  }

  const handleClose = () => {
    void appWindow.close()
  }

  return (
    <>
      <div className="drag bg-background flex justify-between gap-2 border-b p-2">
        <ButtonGroup>
          <Button
            variant="outline"
            onClick={handleGoHome}
            disabled={!hasImage}
            title={withShortcut(t("header.home"), shortcuts.closeImage)}
            aria-label={withShortcut(t("header.home"), shortcuts.closeImage)}
          >
            <House />
            <span className="hidden md:inline">{t("header.home")}</span>
          </Button>
          <Button
            variant="outline"
            onClick={handleOpenFile}
            title={withShortcut(t("header.open"), shortcuts.openFile)}
            aria-label={withShortcut(t("header.open"), shortcuts.openFile)}
          >
            <FolderOpen />
            <span className="hidden md:inline">{t("header.open")}</span>
          </Button>
        </ButtonGroup>

        <ButtonGroup>
          <ButtonGroup>
            <ButtonGroup>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("width")}
                disabled={!hasImage}
                title={withShortcut(t("header.fitWidth"), shortcuts.fitWidth)}
                aria-label={withShortcut(
                  t("header.fitWidth"),
                  shortcuts.fitWidth
                )}
              >
                <ArrowsHorizontal />
                <span className="hidden md:inline">{t("header.fitWidth")}</span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("height")}
                disabled={!hasImage}
                title={withShortcut(t("header.fitHeight"), shortcuts.fitHeight)}
                aria-label={withShortcut(
                  t("header.fitHeight"),
                  shortcuts.fitHeight
                )}
              >
                <ArrowsVertical />
                <span className="hidden md:inline">
                  {t("header.fitHeight")}
                </span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("screen")}
                disabled={!hasImage}
                title={withShortcut(t("header.fitScreen"), shortcuts.fitScreen)}
                aria-label={withShortcut(
                  t("header.fitScreen"),
                  shortcuts.fitScreen
                )}
              >
                <ArrowsOut />
                <span className="hidden md:inline">
                  {t("header.fitScreen")}
                </span>
              </Button>
            </ButtonGroup>

            <Separator orientation="vertical" />

            <ButtonGroup>
              <Button
                variant="ghost"
                onClick={zoomOut}
                disabled={!hasImage}
                title={withShortcut(t("header.zoomOut"), shortcuts.zoomOut)}
                aria-label={withShortcut(
                  t("header.zoomOut"),
                  shortcuts.zoomOut
                )}
              >
                <MagnifyingGlassMinus />
                <span className="hidden md:inline">{t("header.zoomOut")}</span>
              </Button>
              <ButtonGroupText
                className={cn("tabular-nums", !hasImage && "opacity-50")}
                aria-disabled={!hasImage}
                onClick={hasImage ? resetZoomPan : undefined}
              >
                {Math.round(zoom * 100)}%
              </ButtonGroupText>
              <Button
                variant="ghost"
                onClick={zoomIn}
                disabled={!hasImage}
                title={withShortcut(t("header.zoomIn"), shortcuts.zoomIn)}
                aria-label={withShortcut(t("header.zoomIn"), shortcuts.zoomIn)}
              >
                <MagnifyingGlassPlus />
                <span className="hidden md:inline">{t("header.zoomIn")}</span>
              </Button>
            </ButtonGroup>

            <Separator orientation="vertical" />

            <ButtonGroup>
              <Button
                variant="ghost"
                size="icon"
                onClick={rotateCCW}
                disabled={!hasImage}
                title={withShortcut(t("menu.rotateCcw"), shortcuts.rotateCCW)}
                aria-label={withShortcut(
                  t("menu.rotateCcw"),
                  shortcuts.rotateCCW
                )}
              >
                <ArrowCounterClockwise />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={rotateCW}
                disabled={!hasImage}
                title={withShortcut(t("menu.rotateCw"), shortcuts.rotateCW)}
                aria-label={withShortcut(
                  t("menu.rotateCw"),
                  shortcuts.rotateCW
                )}
              >
                <ArrowClockwise />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipHorizontal}
                disabled={!hasImage}
                title={withShortcut(t("menu.flipH"), shortcuts.flipH)}
                aria-label={withShortcut(t("menu.flipH"), shortcuts.flipH)}
              >
                <FlipHorizontal />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipVertical}
                disabled={!hasImage}
                title={withShortcut(t("menu.flipV"), shortcuts.flipV)}
                aria-label={withShortcut(t("menu.flipV"), shortcuts.flipV)}
              >
                <FlipVertical />
              </Button>
            </ButtonGroup>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              onClick={() => void toggleExifPanel()}
              disabled={!hasImage}
              title={withShortcut(t("header.info"), shortcuts.toggleExif)}
              aria-label={withShortcut(t("header.info"), shortcuts.toggleExif)}
            >
              <Info />
              <span className="hidden md:inline">{t("header.info")}</span>
            </Button>
            <Button
              variant="outline"
              onClick={() => usePaletteStore.getState().setOpen(true)}
              title={withShortcut(t("palette.open"), shortcuts.togglePalette)}
              aria-label={withShortcut(
                t("palette.open"),
                shortcuts.togglePalette
              )}
            >
              <Command />
              <span className="hidden md:inline">{t("palette.open")}</span>
            </Button>
            <Button
              variant="outline"
              onClick={() => setSettingsOpen(true)}
              title={t("header.settingsTitle")}
              aria-label={t("header.settingsTitle")}
            >
              <Gear />
              <span className="hidden md:inline">{t("header.settings")}</span>
            </Button>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              size="icon"
              onClick={() => void toggleAlwaysOnTop()}
              title={withShortcut(
                t("header.alwaysOnTop"),
                shortcuts.toggleAlwaysOnTop
              )}
              aria-label={withShortcut(
                t("header.alwaysOnTop"),
                shortcuts.toggleAlwaysOnTop
              )}
              aria-pressed={alwaysOnTop}
              className={cn(
                alwaysOnTop &&
                  "border-primary/40 bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary"
              )}
            >
              <PushPin weight={alwaysOnTop ? "fill" : "regular"} />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={handleMinimize}
              title={t("header.minimize")}
              aria-label={t("header.minimize")}
            >
              <Minus />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleMaximize}
              title={t("header.maximize")}
              aria-label={t("header.maximize")}
              aria-pressed={isMaximized}
            >
              {isMaximized ? <Copy /> : <Square />}
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleClose}
              title={t("header.close")}
              aria-label={t("header.close")}
              className="hover:border-red-600 hover:bg-red-600 hover:text-white dark:hover:border-red-600 dark:hover:bg-red-600"
            >
              <X />
            </Button>
          </ButtonGroup>
        </ButtonGroup>
      </div>
      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </>
  )
}
