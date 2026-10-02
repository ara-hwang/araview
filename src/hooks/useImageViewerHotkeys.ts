import { useEffect, useRef } from "react"

import { eventToBinding, type ShortcutActionId } from "@/constants/shortcuts"
import { runViewerCommand, type ViewerActionHandlers } from "@/hooks/viewerActions"
import { getSettings } from "@/store/settingsStore"

type ImageViewerHotkeysParams = ViewerActionHandlers & {
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

      if (action === "togglePalette") return
      // 브라우저 기본 동작(스크롤, F키, 페이지 이동)이 겹치는 바인딩은 막는다.
      // 웹툰 상하 이동은 네이티브 중복 스크롤을 막고 일정량만 이동한다.
      if (
        binding === "Space" ||
        binding.startsWith("F") ||
        binding.startsWith("Page") ||
        binding === "Home" ||
        binding === "End" ||
        ((action === "panUp" || action === "panDown") && getSettings().viewMode === "webtoon")
      ) {
        e.preventDefault()
      }
      runViewerCommand(action, propsRef.current)
    }

    window.addEventListener("keydown", onKeyDown)
    return () => {
      window.removeEventListener("keydown", onKeyDown)
    }
  }, [])
}
