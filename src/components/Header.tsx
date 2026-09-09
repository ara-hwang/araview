import { useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  ArrowLeftRight,
  ArrowUpDown,
  FlipHorizontal2,
  FlipVertical2,
  FolderOpen,
  Info,
  Maximize2,
  Minus,
  RotateCcw,
  RotateCw,
  Settings,
  Square,
  X,
  ZoomIn,
  ZoomOut
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "./ui/button-group"
import { ButtonGroupText } from "./ui/button-group"
import { Separator } from "./ui/separator"
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
import { useExifLoader } from "@/hooks/useExifLoader"
import { SettingsDialog } from "@/components/SettingsDialog"
import { useTranslation } from "react-i18next"

// 위쪽 툴바
export default function Header() {
  const { t } = useTranslation()
  const appWindow = getCurrentWindow()
  const zoom = useAppStore((state) => state.zoom)
  const [settingsOpen, setSettingsOpen] = useState(false)

  const { handleOpenFile } = useImageLoader()
  const { toggleExifPanel } = useExifLoader()

  const handleMinimize = () => {
    void appWindow.minimize()
  }

  const handleMaximize = () => {
    void appWindow.toggleMaximize()
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
                title={t("header.fitWidthTitle")}
                aria-label={t("header.fitWidthTitle")}
              >
                <ArrowLeftRight />
                <span className="hidden md:inline">{t("header.fitWidth")}</span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("height")}
                title={t("header.fitHeightTitle")}
                aria-label={t("header.fitHeightTitle")}
              >
                <ArrowUpDown />
                <span className="hidden md:inline">
                  {t("header.fitHeight")}
                </span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("screen")}
                title={t("header.fitScreenTitle")}
                aria-label={t("header.fitScreenTitle")}
              >
                <Maximize2 />
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
                title={t("header.zoomOutTitle")}
                aria-label={t("header.zoomOutTitle")}
              >
                <ZoomOut />
                <span className="hidden md:inline">{t("header.zoomOut")}</span>
              </Button>
              <ButtonGroupText className="tabular-nums" onClick={resetZoomPan}>
                {Math.round(zoom * 100)}%
              </ButtonGroupText>
              <Button
                variant="ghost"
                onClick={zoomIn}
                title={t("header.zoomInTitle")}
                aria-label={t("header.zoomInTitle")}
              >
                <ZoomIn />
                <span className="hidden md:inline">{t("header.zoomIn")}</span>
              </Button>
            </ButtonGroup>

            <Separator orientation="vertical" />

            <ButtonGroup>
              <Button
                variant="ghost"
                size="icon"
                onClick={rotateCCW}
                title={t("header.rotateCcwTitle")}
                aria-label={t("header.rotateCcwTitle")}
              >
                <RotateCcw />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={rotateCW}
                title={t("header.rotateCwTitle")}
                aria-label={t("header.rotateCwTitle")}
              >
                <RotateCw />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipHorizontal}
                title={t("header.flipHTitle")}
                aria-label={t("header.flipHTitle")}
              >
                <FlipHorizontal2 />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={flipVertical}
                title={t("header.flipVTitle")}
                aria-label={t("header.flipVTitle")}
              >
                <FlipVertical2 />
              </Button>
            </ButtonGroup>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              onClick={() => void toggleExifPanel()}
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
              <Settings />
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
            >
              <Square />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleClose}
              title={t("header.close")}
              aria-label={t("header.close")}
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
