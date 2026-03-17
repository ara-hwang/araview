import { Minus, Square, X } from "lucide-react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "./ui/button-group"

import {
  ArrowLeftRight,
  ArrowUpDown,
  FolderOpen,
  Maximize2,
  Settings,
  ZoomIn,
  ZoomOut
} from "lucide-react"
import { ButtonGroupText } from "./ui/button-group"
import { Separator } from "./ui/separator"
import { useImageViewer } from "@/hooks/useImageViewer"
import { useNavigate } from "@tanstack/react-router"
import { getApp } from "@/store/appStore"

// 위쪽 툴바
export default function Header() {
  const appWindow = getCurrentWindow()
  const navigate = useNavigate()

  const imageViewer = useImageViewer()

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
          onClick={imageViewer.handleOpenFile}
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
              onClick={imageViewer.handleFitWidth}
              title="Fit to width (1)"
            >
              <ArrowLeftRight />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={imageViewer.handleFitHeight}
              title="Fit to height (2)"
            >
              <ArrowUpDown />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={imageViewer.handleFitScreen}
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
              onClick={imageViewer.handleZoomOut}
              title="Zoom out (-)"
            >
              <ZoomOut />
            </Button>
            <ButtonGroupText
              className="tabular-nums"
              onClick={imageViewer.handleResetZoom}
            >
              {Math.round(getApp().zoom * 100)}%
            </ButtonGroupText>
            <Button
              variant="outline"
              size="icon"
              onClick={imageViewer.handleZoomIn}
              title="Zoom in (+)"
            >
              <ZoomIn />
            </Button>
          </ButtonGroup>
        </ButtonGroup>

        {/* 설정 */}
        <ButtonGroup>
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
