import { useCallback } from "react"

import { getEffectiveViewMode } from "@/hooks/useEffectiveViewMode"
import { useAppStore, zoomInBy, zoomOutBy, type ZoomAnchor } from "@/store/appStore"
import { getSettings } from "@/store/settingsStore"
import { resolveWheelAction } from "@/utils/wheelAction"

export type WheelNavigationOptions = {
  /**
   * 웹툰 모드에서 수식키 없는 휠을 네이티브 스크롤에 넘긴다 (뷰어 기본).
   * 도크처럼 세로 스크롤이 없는 영역에서는 false로 두어 휠 이동을 유지한다.
   */
  webtoonPassthrough?: boolean
}

/**
 * 휠 줌의 기준점. 단일 보기에서 커서가 이미지 컨테이너 위에 있으면 커서 위치를,
 * 도크처럼 컨테이너 밖이거나 팬이 없는 보기 모드면 undefined(컨테이너 중심)를 돌려준다.
 */
function getCursorZoomAnchor(clientX: number, clientY: number): ZoomAnchor | undefined {
  if (getEffectiveViewMode() !== "single") return undefined
  const rect = useAppStore.getState().containerElement?.getBoundingClientRect()
  if (!rect || rect.width <= 0 || rect.height <= 0) return undefined
  if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
    return undefined
  }
  return {
    x: clientX - (rect.left + rect.width / 2),
    y: clientY - (rect.top + rect.height / 2)
  }
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
        zoomInBy(1.1, getCursorZoomAnchor(e.clientX, e.clientY))
        return
      }
      if (action === "zoomOut") {
        zoomOutBy(1.1, getCursorZoomAnchor(e.clientX, e.clientY))
      }
    },
    [navigateImage, webtoonPassthrough]
  )
}
