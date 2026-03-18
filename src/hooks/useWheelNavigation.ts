import { useCallback } from "react"

type ZoomPanHandlers = {
  handleWheel: (e: React.WheelEvent) => void
}

export function useWheelNavigation(
  zoomPan: ZoomPanHandlers,
  navigateImage: (direction: "prev" | "next") => Promise<void> | void
) {
  return useCallback(
    (e: React.WheelEvent) => {
      if (e.ctrlKey) {
        zoomPan.handleWheel(e)
      } else {
        void navigateImage(e.deltaY < 0 ? "prev" : "next")
      }
    },
    [navigateImage, zoomPan]
  )
}
