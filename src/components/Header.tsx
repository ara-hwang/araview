import { useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  ArrowLeftRight,
  ArrowUpDown,
  FolderOpen,
  Info,
  Maximize2,
  Minus,
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
  resetZoomPan,
  setZoomToFit,
  zoomIn,
  zoomOut,
  useAppStore
} from "@/store/appStore"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useExifLoader } from "@/hooks/useExifLoader"
import { SettingsDialog } from "@/components/SettingsDialog"

// 위쪽 툴바
export default function Header() {
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
            title="Open file (Ctrl+O)"
          >
            <FolderOpen />
            Open
          </Button>
        </ButtonGroup>

        <ButtonGroup>
          <ButtonGroup>
            <ButtonGroup>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setZoomToFit("width")}
                title="Fit to width (1)"
              >
                <ArrowLeftRight />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setZoomToFit("height")}
                title="Fit to height (2)"
              >
                <ArrowUpDown />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setZoomToFit("screen")}
                title="Fit to screen (3)"
              >
                <Maximize2 />
              </Button>
            </ButtonGroup>

            <Separator orientation="vertical" />

            <ButtonGroup>
              <Button
                variant="ghost"
                size="icon"
                onClick={zoomOut}
                title="Zoom out (-)"
              >
                <ZoomOut />
              </Button>
              <ButtonGroupText className="tabular-nums" onClick={resetZoomPan}>
                {Math.round(zoom * 100)}%
              </ButtonGroupText>
              <Button
                variant="ghost"
                size="icon"
                onClick={zoomIn}
                title="Zoom in (+)"
              >
                <ZoomIn />
              </Button>
            </ButtonGroup>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              size="icon"
              onClick={() => void toggleExifPanel()}
              title="EXIF info (I)"
            >
              <Info />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={() => setSettingsOpen(true)}
              title="Settings"
            >
              <Settings />
            </Button>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              size="icon"
              onClick={handleMinimize}
              title="Minimize"
            >
              <Minus />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleMaximize}
              title="Maximize / Restore"
            >
              <Square />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleClose}
              title="Close"
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
