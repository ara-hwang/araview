import {
  flipHorizontal,
  flipVertical,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import { useHotkey } from "@tanstack/react-hotkeys"

type ImageViewerHotkeysParams = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onOpenFile: () => void
  onToggleExif: () => void
  onToggleSlideshow: () => void
  onToggleFullscreen: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onCycleBackground: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onToggleShuffle: () => void
  onSaveEdits: () => void
  onToggleFavorite: () => void
}

export function useImageViewerHotkeys(props: ImageViewerHotkeysParams) {
  // 방향키 / 숫자 / 조합키 등으로 각각의 액션을 바인딩
  useHotkey("ArrowLeft", props.onNavigatePrev)
  useHotkey("ArrowRight", props.onNavigateNext)
  useHotkey({ key: "+" }, zoomIn)
  useHotkey("=", zoomIn)
  useHotkey("-", zoomOut)
  useHotkey("0", resetZoomPan)
  useHotkey("1", () => setZoomToFit("width"))
  useHotkey("2", () => setZoomToFit("height"))
  useHotkey("3", () => setZoomToFit("screen"))
  useHotkey("Control+O", props.onOpenFile)
  useHotkey("I", props.onToggleExif)
  useHotkey("R", rotateCW)
  useHotkey("Shift+R", rotateCCW)
  useHotkey("H", flipHorizontal)
  useHotkey("V", flipVertical)
  useHotkey("Space", props.onToggleSlideshow)
  useHotkey("F5", props.onToggleSlideshow)
  useHotkey("F11", props.onToggleFullscreen)
  useHotkey("Control+C", props.onCopyImage)
  useHotkey("Delete", props.onTrashFile)
  useHotkey("Control+Shift+E", props.onRevealInExplorer)
  useHotkey("Control+Shift+O", props.onOpenExternal)
  useHotkey("B", props.onCycleBackground)
  useHotkey("F2", props.onRenameFile)
  useHotkey("Control+Shift+C", props.onCopyPath)
  useHotkey("S", props.onToggleShuffle)
  useHotkey("Control+S", props.onSaveEdits)
  useHotkey("F", props.onToggleFavorite)
}
