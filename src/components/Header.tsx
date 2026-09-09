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
            title="파일 열기 (Ctrl+O)"
            aria-label="파일 열기 (Ctrl+O)"
          >
            <FolderOpen />
            <span className="hidden md:inline">열기</span>
          </Button>
        </ButtonGroup>

        <ButtonGroup>
          <ButtonGroup>
            <ButtonGroup>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("width")}
                title="너비에 맞춤 (1)"
                aria-label="너비에 맞춤 (1)"
              >
                <ArrowLeftRight />
                <span className="hidden md:inline">너비</span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("height")}
                title="높이에 맞춤 (2)"
                aria-label="높이에 맞춤 (2)"
              >
                <ArrowUpDown />
                <span className="hidden md:inline">높이</span>
              </Button>
              <Button
                variant="ghost"
                onClick={() => setZoomToFit("screen")}
                title="화면에 맞춤 (3)"
                aria-label="화면에 맞춤 (3)"
              >
                <Maximize2 />
                <span className="hidden md:inline">화면</span>
              </Button>
            </ButtonGroup>

            <Separator orientation="vertical" />

            <ButtonGroup>
              <Button
                variant="ghost"
                onClick={zoomOut}
                title="축소 (-)"
                aria-label="축소 (-)"
              >
                <ZoomOut />
                <span className="hidden md:inline">축소</span>
              </Button>
              <ButtonGroupText className="tabular-nums" onClick={resetZoomPan}>
                {Math.round(zoom * 100)}%
              </ButtonGroupText>
              <Button
                variant="ghost"
                onClick={zoomIn}
                title="확대 (+)"
                aria-label="확대 (+)"
              >
                <ZoomIn />
                <span className="hidden md:inline">확대</span>
              </Button>
            </ButtonGroup>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              onClick={() => void toggleExifPanel()}
              title="EXIF 정보 (I)"
              aria-label="EXIF 정보 (I)"
            >
              <Info />
              <span className="hidden md:inline">정보</span>
            </Button>
            <Button
              variant="outline"
              onClick={() => setSettingsOpen(true)}
              title="설정"
              aria-label="설정"
            >
              <Settings />
              <span className="hidden md:inline">설정</span>
            </Button>
          </ButtonGroup>

          <ButtonGroup>
            <Button
              variant="outline"
              size="icon"
              onClick={handleMinimize}
              title="최소화"
              aria-label="최소화"
            >
              <Minus />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleMaximize}
              title="최대화 / 복원"
              aria-label="최대화 / 복원"
            >
              <Square />
            </Button>

            <Button
              variant="outline"
              size="icon"
              onClick={handleClose}
              title="닫기"
              aria-label="닫기"
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
