import { useEffect, useRef } from "react"
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
import { getSettings } from "@/store/settingsStore"
import { eventToBinding, type ShortcutActionId } from "@/constants/shortcuts"

type ImageViewerHotkeysParams = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
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
  onToggleFavorite: () => void
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

export function useImageViewerHotkeys(props: ImageViewerHotkeysParams) {
  const propsRef = useRef(props)
  propsRef.current = props

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return
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
        if (binding === "Space" || binding.startsWith("F")) {
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
        case "toggleFavorite":
          run(p.onToggleFavorite)
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
