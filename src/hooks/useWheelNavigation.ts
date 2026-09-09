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
      if (settings.viewMode === "webtoon") return
      if (e.deltaY === 0) return
      const slot = toWheelSlot(e)
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
