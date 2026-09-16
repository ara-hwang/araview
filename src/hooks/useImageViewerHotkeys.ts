import { useEffect, useRef } from "react"

import { eventToBinding, type ShortcutActionId } from "@/constants/shortcuts"
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
import { getSettings } from "@/store/settingsStore"

type ImageViewerHotkeysParams = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onJumpPrev10: () => void
  onJumpNext10: () => void
  onJumpFirst: () => void
  onJumpLast: () => void
  onOpenFile: () => void
  onCloseImage: () => void
  onToggleExif: () => void
  onToggleSlideshow: () => void
  onToggleFullscreen: () => void
  onToggleAlwaysOnTop: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onCycleBackground: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onToggleShuffle: () => void
  onSaveEdits: () => void
  onToggleGrid: () => void
  /** 그리드 등 오버레이가 열려 있을 때 뷰어 단축키 전체를 막는다 */
  disabled?: boolean
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.closest('[role="dialog"], [data-slot="dialog-content"]')) {
    return true
  }
  const tag = target.tagName
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true
  if (target.isContentEditable) return true
  return false
}

/** 웹툰 연속 스크롤 컨테이너를 키보드로 스크롤한다. 포커스 위치와 무관하게 동작. */
export function scrollWebtoonBy(dy: number): boolean {
  if (typeof document === "undefined") return false
  const el = document.querySelector('[aria-label="webtoon-scroll"]')
  if (!(el instanceof HTMLElement)) return false
  el.scrollBy({ top: dy, behavior: "auto" })
  return true
}

export const WEBTOON_KEY_SCROLL_PX = 240

export function useImageViewerHotkeys(props: ImageViewerHotkeysParams) {
  const propsRef = useRef(props)
  propsRef.current = props

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return
      if (propsRef.current.disabled) return
      const binding = eventToBinding(e)
      if (!binding) return

      const shortcuts = getSettings().shortcuts
      let action: ShortcutActionId | null = null
      for (const [id, value] of Object.entries(shortcuts)) {
        if (value === binding) {
          action = id as ShortcutActionId
          break
        }
      }
      if (!action) return

      // 다이얼로그 내부 포커스에서는 뷰어 단축키를 막는다. 단, EXIF 패널이
      // 열린 상태에서 패널 내부 포커스라면 I 토글 닫기는 허용한다.
      if (isEditableTarget(e.target)) {
        const inExifPanel =
          action === "toggleExif" &&
          e.target instanceof HTMLElement &&
          e.target.closest("[data-exif-panel]") !== null
        if (!inExifPanel) return
      }

      const p = propsRef.current
      const run = (fn: () => void) => {
        if (
          binding === "Space" ||
          binding.startsWith("F") ||
          binding.startsWith("Page") ||
          binding === "Home" ||
          binding === "End"
        ) {
          e.preventDefault()
        }
        fn()
      }

      switch (action) {
        case "navigatePrev":
          run(p.onNavigatePrev)
          break
        case "navigateNext":
          run(p.onNavigateNext)
          break
        case "panLeft":
          // Webtoon은 transform 팬이 없어 좌우는 이전/다음 이미지로 스크롤
          if (getSettings().viewMode === "webtoon") run(p.onNavigatePrev)
          else run(panLeft)
          break
        case "panRight":
          if (getSettings().viewMode === "webtoon") run(p.onNavigateNext)
          else run(panRight)
          break
        case "panUp":
        case "panDown": {
          // Webtoon 상하는 연속 스크롤 컨테이너를 직접 스크롤한다.
          // 네이티브 중복 스크롤을 막고 일정량 이동.
          if (getSettings().viewMode === "webtoon") {
            const dy = action === "panUp" ? -WEBTOON_KEY_SCROLL_PX : WEBTOON_KEY_SCROLL_PX
            e.preventDefault()
            run(() => {
              scrollWebtoonBy(dy)
            })
            break
          }
          run(action === "panUp" ? panUp : panDown)
          break
        }
        case "jumpPrev10":
          run(p.onJumpPrev10)
          break
        case "jumpNext10":
          run(p.onJumpNext10)
          break
        case "jumpFirst":
          run(p.onJumpFirst)
          break
        case "jumpLast":
          run(p.onJumpLast)
          break
        case "zoomIn":
          run(zoomIn)
          break
        case "zoomOut":
          run(zoomOut)
          break
        case "resetView":
          run(resetZoomPan)
          break
        case "fitWidth":
          run(() => setZoomToFit("width"))
          break
        case "fitHeight":
          run(() => setZoomToFit("height"))
          break
        case "fitScreen":
          run(() => setZoomToFit("screen"))
          break
        case "openFile":
          run(p.onOpenFile)
          break
        case "closeImage":
          run(p.onCloseImage)
          break
        case "toggleExif":
          run(p.onToggleExif)
          break
        case "rotateCW":
          run(rotateCW)
          break
        case "rotateCCW":
          run(rotateCCW)
          break
        case "flipH":
          run(flipHorizontal)
          break
        case "flipV":
          run(flipVertical)
          break
        case "toggleSlideshow":
          run(p.onToggleSlideshow)
          break
        case "toggleFullscreen":
          run(p.onToggleFullscreen)
          break
        case "toggleAlwaysOnTop":
          run(p.onToggleAlwaysOnTop)
          break
        case "copyImage":
          run(p.onCopyImage)
          break
        case "trashFile":
          run(p.onTrashFile)
          break
        case "revealInExplorer":
          run(p.onRevealInExplorer)
          break
        case "openExternal":
          run(p.onOpenExternal)
          break
        case "cycleBackground":
          run(p.onCycleBackground)
          break
        case "renameFile":
          run(p.onRenameFile)
          break
        case "copyPath":
          run(p.onCopyPath)
          break
        case "toggleShuffle":
          run(p.onToggleShuffle)
          break
        case "saveEdits":
          run(p.onSaveEdits)
          break
        case "toggleGrid":
          run(p.onToggleGrid)
          break
        default:
          break
      }
    }

    window.addEventListener("keydown", onKeyDown)
    return () => {
      window.removeEventListener("keydown", onKeyDown)
    }
  }, [])
}
