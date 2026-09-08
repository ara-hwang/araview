import { Minus, Square, X } from "lucide-react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "./ui/button-group"

import {
  ArrowLeftRight,
  ArrowUpDown,
  FolderOpen,
  Info,
  Maximize2,
  Settings,
  ZoomIn,
  ZoomOut
} from "lucide-react"
import { ButtonGroupText } from "./ui/button-group"
import { Separator } from "./ui/separator"
import { useNavigate } from "@tanstack/react-router"
import {
  resetZoomPan,
  setZoomToFit,
  zoomIn,
  zoomOut,
  useAppStore
} from "@/store/appStore"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useExifLoader } from "@/hooks/useExifLoader"

// 위쪽 툴바
export default function Header() {
  const appWindow = getCurrentWindow()
  const navigate = useNavigate()
  const zoom = useAppStore((state) => state.zoom)

  const { handleOpenFile } = useImageLoader()
  const { toggleExifPanel } = useExifLoader()

  const handleOpenSettings = () => {
    void navigate({ to: "/page/settings" })
  }

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
    <div className="drag flex justify-between p-2">
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
              variant="outline"
              size="icon"
              onClick={() => setZoomToFit("width")}
              title="Fit to width (1)"
            >
              <ArrowLeftRight />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={() => setZoomToFit("height")}
              title="Fit to height (2)"
            >
              <ArrowUpDown />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={() => setZoomToFit("screen")}
              title="Fit to screen (3)"
            >
              <Maximize2 />
            </Button>
          </ButtonGroup>

          <Separator orientation="vertical" />

          {/* 확대율 */}
          <ButtonGroup>
            <Button
              variant="outline"
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
              variant="outline"
              size="icon"
              onClick={zoomIn}
              title="Zoom in (+)"
            >
              <ZoomIn />
            </Button>
          </ButtonGroup>
        </ButtonGroup>

        {/* EXIF & 설정 */}
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
            onClick={handleOpenSettings}
            title="Settings"
          >
            <Settings />
          </Button>
        </ButtonGroup>

        <ButtonGroup>
          {/* 최소화 */}
          <Button
            variant="outline"
            size="icon"
            onClick={handleMinimize}
            title="Minimize"
          >
            <Minus />
          </Button>

          {/* 최대화 / 복구 */}
          <Button
            variant="outline"
            size="icon"
            onClick={handleMaximize}
            title="Maximize / Restore"
          >
            <Square />
          </Button>

          {/* 닫기 */}
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
  )
}
