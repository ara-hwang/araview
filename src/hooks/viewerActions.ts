import type { CommandId } from "@/constants/commands"
import { requestUpdateCheck } from "@/hooks/useUpdater"
import {
  flipHorizontal,
  flipVertical,
  panDown,
  panLeft,
  panRight,
  panUp,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import { stepGifFrameBy, toggleGifPlayback } from "@/store/gifStore"
import { cycleViewerBackground, getSettings } from "@/store/settingsStore"

/**
 * 뷰어 페이지(또는 팔레트 호스트)가 제공하는 동작. 단축키, 명령 팔레트,
 * 컨텍스트 메뉴가 같은 맵으로 `runViewerCommand`를 부른다.
 */
export type ViewerActionHandlers = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onJumpPrev10: () => void
  onJumpNext10: () => void
  onJumpFirst: () => void
  onJumpLast: () => void
  onOpenFile: () => void
  onCloseImage: () => void
  onToggleExif: () => void
  onToggleFullscreen: () => void
  onToggleAlwaysOnTop: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onToggleGrid: () => void
  onToggleDock: () => void
  /** 현재 페이지의 표지 지정(ComicInfo `FrontCover`)을 뒤집는다. */
  onToggleComicCover: () => void
}

export const OPEN_SETTINGS_EVENT = "tiv:open-settings"

export function requestOpenSettings() {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT))
}

export const WEBTOON_KEY_SCROLL_PX = 240

/** 웹툰 연속 스크롤 컨테이너를 키보드로 스크롤한다. 포커스 위치와 무관하게 동작. */
export function scrollWebtoonBy(dy: number): boolean {
  if (typeof document === "undefined") return false
  const el = document.querySelector('[data-webtoon-scroll-region="true"]')
  if (!(el instanceof HTMLElement)) return false
  el.scrollBy({ top: dy, behavior: "auto" })
  return true
}

/**
 * 명령 하나를 실행한다. 명령 id에서 동작으로 가는 매핑의 단일 출처.
 * 팔레트 토글처럼 여기서 다루지 않는 명령은 false를 돌려준다.
 */
export function runViewerCommand(id: CommandId, h: ViewerActionHandlers): boolean {
  // Webtoon은 transform 팬이 없어 좌우는 이전/다음 이미지로, 상하는 연속
  // 스크롤 컨테이너를 직접 스크롤한다.
  const webtoon = getSettings().viewMode === "webtoon"
  switch (id) {
    case "openFile":
      h.onOpenFile()
      return true
    case "closeImage":
      h.onCloseImage()
      return true
    case "navigatePrev":
      h.onNavigatePrev()
      return true
    case "navigateNext":
      h.onNavigateNext()
      return true
    case "jumpPrev10":
      h.onJumpPrev10()
      return true
    case "jumpNext10":
      h.onJumpNext10()
      return true
    case "jumpFirst":
      h.onJumpFirst()
      return true
    case "jumpLast":
      h.onJumpLast()
      return true
    case "zoomIn":
      zoomIn()
      return true
    case "zoomOut":
      zoomOut()
      return true
    case "panLeft":
      if (webtoon) h.onNavigatePrev()
      else panLeft()
      return true
    case "panRight":
      if (webtoon) h.onNavigateNext()
      else panRight()
      return true
    case "panUp":
      if (webtoon) scrollWebtoonBy(-WEBTOON_KEY_SCROLL_PX)
      else panUp()
      return true
    case "panDown":
      if (webtoon) scrollWebtoonBy(WEBTOON_KEY_SCROLL_PX)
      else panDown()
      return true
    case "resetView":
      resetZoomPan()
      return true
    case "fitWidth":
      setZoomToFit("width")
      return true
    case "fitHeight":
      setZoomToFit("height")
      return true
    case "fitScreen":
      setZoomToFit("screen")
      return true
    case "rotateCW":
      rotateCW()
      return true
    case "rotateCCW":
      rotateCCW()
      return true
    case "flipH":
      flipHorizontal()
      return true
    case "flipV":
      flipVertical()
      return true
    case "toggleExif":
      h.onToggleExif()
      return true
    case "toggleFullscreen":
      h.onToggleFullscreen()
      return true
    case "toggleAlwaysOnTop":
      h.onToggleAlwaysOnTop()
      return true
    case "copyImage":
      h.onCopyImage()
      return true
    case "trashFile":
      h.onTrashFile()
      return true
    case "revealInExplorer":
      h.onRevealInExplorer()
      return true
    case "openExternal":
      h.onOpenExternal()
      return true
    case "cycleBackground":
      cycleViewerBackground()
      return true
    case "renameFile":
      h.onRenameFile()
      return true
    case "copyPath":
      h.onCopyPath()
      return true
    case "toggleGrid":
      h.onToggleGrid()
      return true
    case "toggleDock":
      h.onToggleDock()
      return true
    case "toggleComicCover":
      h.onToggleComicCover()
      return true
    case "toggleGifPlayback":
      toggleGifPlayback()
      return true
    case "gifPrevFrame":
      stepGifFrameBy(-1)
      return true
    case "gifNextFrame":
      stepGifFrameBy(1)
      return true
    case "openSettings":
      requestOpenSettings()
      return true
    case "checkForUpdates":
      requestUpdateCheck()
      return true
    case "togglePalette":
      return false
  }
}
