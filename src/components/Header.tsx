import { useEffect, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  ArrowsHorizontal,
  ArrowsOut,
  ArrowsVertical,
  ArrowClockwise,
  ArrowCounterClockwise,
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
import { SettingsDialog } from "@/components/SettingsDialog"
import { useTranslation } from "react-i18next"

// 위쪽 툴바
export default function Header() {
  const { t } = useTranslation()
  const [appWindow] = useState(() => getCurrentWindow())
  const zoom = useAppStore((state) => state.zoom)
  const hasImage = useAppStore((state) => state.imageInfo !== null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [isMaximized, setIsMaximized] = useState(false)

  const { handleOpenFile } = useImageLoader()
  const handleGoHome = useCloseImage()
  const { toggleExifPanel } = useExifLoader()

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
            title={t("header.homeTitle")}
            aria-label={t("header.homeTitle")}
          >
            <House />
            <span className="hidden md:inline">{t("header.home")}</span>
          </Button>
          <Button
            variant="outline"
            onClick={handleOpenFile}
            title={t("header.openTitle")}
            aria-label={t("header.openTitle")}
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
                title={t("header.fitWidthTitle")}
                aria-label={t("header.fitWidthTitle")}
              >
                <ArrowsHorizontal />
                <span className="hidden md:inline">{t("header.fitWidth")}</span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("height")}
                disabled={!hasImage}
                title={t("header.fitHeightTitle")}
                aria-label={t("header.fitHeightTitle")}
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
                title={t("header.fitScreenTitle")}
                aria-label={t("header.fitScreenTitle")}
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
                title={t("header.zoomOutTitle")}
                aria-label={t("header.zoomOutTitle")}
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
                title={t("header.zoomInTitle")}
                aria-label={t("header.zoomInTitle")}
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
                title={t("header.rotateCcwTitle")}
                aria-label={t("header.rotateCcwTitle")}
              >
                <ArrowCounterClockwise />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={rotateCW}
                disabled={!hasImage}
                title={t("header.rotateCwTitle")}
                aria-label={t("header.rotateCwTitle")}
              >
                <ArrowClockwise />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipHorizontal}
                disabled={!hasImage}
                title={t("header.flipHTitle")}
                aria-label={t("header.flipHTitle")}
              >
                <FlipHorizontal />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipVertical}
                disabled={!hasImage}
                title={t("header.flipVTitle")}
                aria-label={t("header.flipVTitle")}
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
              title={t("header.infoTitle")}
              aria-label={t("header.infoTitle")}
            >
              <Info />
              <span className="hidden md:inline">{t("header.info")}</span>
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
