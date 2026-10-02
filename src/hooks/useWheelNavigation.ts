import { useCallback } from "react"

import { zoomInBy, zoomOutBy } from "@/store/appStore"
import { getSettings } from "@/store/settingsStore"
import { resolveWheelAction } from "@/utils/wheelAction"

export type WheelNavigationOptions = {
  /**
   * 웹툰 모드에서 수식키 없는 휠을 네이티브 스크롤에 넘긴다 (뷰어 기본).
   * 도크처럼 세로 스크롤이 없는 영역에서는 false로 두어 휠 이동을 유지한다.
   */
  webtoonPassthrough?: boolean
}

export function useWheelNavigation(
  navigateImage: (direction: "prev" | "next") => Promise<void> | void,
  options: WheelNavigationOptions = {}
) {
  const webtoonPassthrough = options.webtoonPassthrough ?? true
  return useCallback(
    (e: React.WheelEvent) => {
      const settings = getSettings()
      const action = resolveWheelAction(e, settings.wheel)
      if (!action) return
      // Webtoon은 세로 스크롤이 본문이므로 일반 휠은 네이티브 스크롤에 맡긴다.
      // Ctrl+휠 같은 수식키 조합은 줌 설정대로 동작시킨다.
      if (webtoonPassthrough && settings.viewMode === "webtoon" && !e.ctrlKey) return
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
    [navigateImage, webtoonPassthrough]
  )
}
