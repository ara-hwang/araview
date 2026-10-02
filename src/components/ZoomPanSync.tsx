import { useZoomPanSync } from "@/hooks/useZoomPan"

/**
 * 줌/팬 상태 보정만 담는, 아무것도 그리지 않는 컴포넌트. 팬 프레임마다 바뀌는
 * position/zoom 구독을 여기에 가둬 이미지 페이지 전체(도크 포함)가 다시 렌더되지
 * 않게 한다.
 */
export function ZoomPanSync() {
  useZoomPanSync()
  return null
}
