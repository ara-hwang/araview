import { useCallback, useEffect, useLayoutEffect, useRef } from "react"
import { useShallow } from "zustand/react/shallow"

import {
  useAppStore,
  zoomInBy,
  zoomOutBy,
  resetZoomPan,
  startDrag,
  moveDrag
} from "@/store/appStore"
import { getSettings } from "@/store/settingsStore"

import {
  getPositionBounds,
  clampPosition,
  getOrientedImageSize,
  isFullyContained
} from "../utils/zoomPanUtils"

function getMouseSettings() {
  return getSettings().mouse
}

// 이미지 컨테이너 내부에서 줌/팬(드래그) 상태를 관리하고
// 전역 `appStore`의 position/zoom 값을 일관되게 업데이트하는 훅

export function useZoomPan() {
  const app = useAppStore(
    useShallow((state) => ({
      imageInfo: state.imageInfo,
      position: state.position,
      zoom: state.zoom,
      isDragging: state.isDragging,
      containerSize: state.containerSize,
      imageSize: state.imageSize,
      rotation: state.rotation
    }))
  )
  const setIsDragging = useAppStore((state) => state.setIsDragging)

  const prevZoomRef = useRef(1)
  // 고빈도 mousemove를 rAF당 1회로 합쳐 zustand 갱신 폭주를 방지
  const rafRef = useRef(0)
  const pendingPosRef = useRef<{ x: number; y: number } | null>(null)

  useEffect(() => {
    return () => {
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return
      if (getMouseSettings().leftDrag !== "pan") return
      if (app.imageInfo) {
        startDrag(e.clientX, e.clientY)
      }
    },
    [app.imageInfo]
  )

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    pendingPosRef.current = { x: e.clientX, y: e.clientY }
    if (rafRef.current !== 0) return
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0
      const pending = pendingPosRef.current
      pendingPosRef.current = null
      if (pending) moveDrag(pending.x, pending.y)
    })
  }, [])

  const handleMouseUp = useCallback(() => {
    setIsDragging(false)
  }, [])

  // 휠 매핑에서 줌으로 결정된 경우에만 호출된다. 방향에 따라 확대/축소한다.
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()
    if (e.deltaY < 0) {
      zoomInBy(1.1)
    } else if (e.deltaY > 0) {
      zoomOutBy(1.1)
    }
  }, [])

  // 현재 줌 배율에서 이미지가 컨테이너 안에 완전히 들어오면 위치를 (0,0)으로 정렬
  useLayoutEffect(() => {
    if (!app.imageInfo) return
    const cw = app.containerSize.width
    const ch = app.containerSize.height
    const oriented = getOrientedImageSize(app.imageSize.width, app.imageSize.height, app.rotation)
    const iw = oriented.width
    const ih = oriented.height
    if (cw <= 0 || ch <= 0 || iw <= 0 || ih <= 0) return
    if (isFullyContained(cw, ch, iw, ih, app.zoom)) {
      useAppStore.setState({ position: { x: 0, y: 0 } })
    }
  }, [app.imageInfo, app.zoom, app.containerSize, app.imageSize, app.rotation])

  const resetView = useCallback(() => {
    resetZoomPan()
    prevZoomRef.current = 1
  }, [])

  // 줌 값이 바뀔 때 기존 position 비율을 유지하면서,
  // 컨테이너를 벗어나지 않도록 clamp 해서 position을 재계산
  useLayoutEffect(() => {
    if (prevZoomRef.current === app.zoom) return
    const ratio = app.zoom / prevZoomRef.current
    const base = app.position
    const newX = base.x * ratio
    const newY = base.y * ratio
    const cw = app.containerSize.width
    const ch = app.containerSize.height
    const oriented = getOrientedImageSize(app.imageSize.width, app.imageSize.height, app.rotation)
    const iw = oriented.width
    const ih = oriented.height
    if (cw <= 0 || ch <= 0 || iw <= 0 || ih <= 0) {
      useAppStore.setState({ position: { x: newX, y: newY } })
    } else {
      const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, app.zoom)
      useAppStore.setState({ position: clampPosition(newX, newY, maxX, maxY) })
    }
    prevZoomRef.current = app.zoom
  }, [app.zoom, app.position, app.containerSize, app.imageSize, app.rotation])

  return {
    position: app.position,
    isDragging: app.isDragging,
    resetView,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp
  }
}
