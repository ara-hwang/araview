import { useCallback } from "react"
import { zoomInBy, zoomOutBy } from "@/store/appStore"
import { getSettings } from "@/store/settingsStore"
import type { WheelSlot } from "@/constants/shortcuts"

type ZoomPanHandlers = {
  handleWheel: (e: React.WheelEvent) => void
}

function toWheelSlot(e: React.WheelEvent): WheelSlot {
  const direction = e.deltaY < 0 ? "wheelUp" : "wheelDown"
  if (e.ctrlKey) return `ctrl+${direction}` as WheelSlot
  if (e.shiftKey) return `shift+${direction}` as WheelSlot
  if (e.altKey) return `alt+${direction}` as WheelSlot
  return direction
}

export function useWheelNavigation(
  _zoomPan: ZoomPanHandlers,
  navigateImage: (direction: "prev" | "next") => Promise<void> | void
) {
  return useCallback(
    (e: React.WheelEvent) => {
      const settings = getSettings()
      if (e.deltaY === 0) return
      const slot = toWheelSlot(e)
      // Webtoon은 세로 스크롤이 본문이므로 일반 휠은 네이티브 스크롤에 맡긴다.
      // Ctrl+휠 같은 수식키 조합은 줌 설정대로 동작시킨다.
      if (settings.viewMode === "webtoon" && !slot.startsWith("ctrl+")) return
      const action = settings.wheel[slot] ?? "none"
      if (action === "none") return
      e.preventDefault()
      if (action === "prev" || action === "next") {
        void navigateImage(action)
        return
      }
      if (action === "zoomIn") {
        zoomInBy(1.1)
        return
      }
      if (action === "zoomOut") {
        zoomOutBy(1.1)
      }
    },
    [navigateImage]
  )
}
