import { getCurrentWindow } from "@tauri-apps/api/window"
import { useCallback, useEffect, useLayoutEffect, useRef } from "react"
import { useShallow } from "zustand/react/shallow"

import { getEffectiveViewMode, useEffectiveViewMode } from "@/hooks/useEffectiveViewMode"
import {
  applyRememberedFit,
  useAppStore,
  zoomInBy,
  zoomOutBy,
  startDrag,
  moveDrag
} from "@/store/appStore"
import { getSettings, useSettingsStore } from "@/store/settingsStore"

import {
  getPositionBounds,
  clampPosition,
  getOrientedImageSize,
  getZoomForFitMode,
  isFullyContained
} from "../utils/zoomPanUtils"

function getMouseSettings() {
  return getSettings().mouse
}

// 이미지 컨테이너의 줌/팬(드래그) 입력을 전역 `appStore`의 position/zoom으로 옮기는 훅.
// 고빈도로 바뀌는 position/zoom을 구독하지 않으므로, 이 훅을 쓰는 이미지 페이지가
// 팬 프레임마다 다시 렌더되지 않는다. 상태 보정 effect는 `useZoomPanSync`가 맡는다.

export function useZoomPan() {
  // 고빈도 mousemove를 rAF당 1회로 합쳐 zustand 갱신 폭주를 방지
  const rafRef = useRef(0)
  const pendingPosRef = useRef<{ x: number; y: number } | null>(null)

  useEffect(() => {
    return () => {
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return
    if (getMouseSettings().leftDrag !== "pan") return
    const { imageInfo, containerSize, imageSize, rotation, zoom } = useAppStore.getState()
    if (!imageInfo) return
    // 양쪽 보기는 팬이 없으므로 항상 창을 끌어 이동한다.
    const viewMode = getEffectiveViewMode()
    if (viewMode === "left-to-right" || viewMode === "right-to-left") {
      void getCurrentWindow()
        .startDragging()
        .catch(() => {})
      return
    }
    // 이미지가 컨테이너에 모두 들어와 팬할 여지가 없으면 창을 끌어 이동한다.
    const oriented = getOrientedImageSize(imageSize.width, imageSize.height, rotation)
    if (
      containerSize.width > 0 &&
      containerSize.height > 0 &&
      oriented.width > 0 &&
      oriented.height > 0 &&
      isFullyContained(
        containerSize.width,
        containerSize.height,
        oriented.width,
        oriented.height,
        zoom
      )
    ) {
      void getCurrentWindow()
        .startDragging()
        .catch(() => {})
      return
    }
    startDrag(e.clientX, e.clientY)
  }, [])

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
    // 아직 그리지 않은 마지막 이동을 놓기 전에 반영한다. 다음 프레임으로
    // 미루면 드래그가 이미 끝나 그 이동이 버려진다.
    if (rafRef.current !== 0) {
      cancelAnimationFrame(rafRef.current)
      rafRef.current = 0
    }
    const pending = pendingPosRef.current
    pendingPosRef.current = null
    if (pending) moveDrag(pending.x, pending.y)
    useAppStore.getState().setIsDragging(false)
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

  // 이미지 전환 시 기억된 맞춤 모드를 다시 적용한다. fitMode는 바꾸지 않는다.
  const resetView = useCallback(() => {
    applyRememberedFit()
  }, [])

  return {
    resetView,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp
  }
}

/**
 * 줌/컨테이너 변화에 맞춰 position/zoom을 보정하는 effect 모음. position/zoom을
 * 구독하므로 팬 프레임마다 다시 렌더된다. 큰 컴포넌트가 아니라 `ZoomPanSync`처럼
 * 아무것도 그리지 않는 컴포넌트에서 호출한다.
 */
export function useZoomPanSync() {
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
  const isFitLocked = useAppStore((state) => state.isFitLocked)
  const viewMode = useEffectiveViewMode()
  const fitMode = useSettingsStore((state) => state.fitMode)

  const prevZoomRef = useRef(1)
  // 컨테이너 변화 감지용. 이미지 전환 경로는 applyRememberedFit이 처리하므로
  // 이 effect는 컨테이너 크기가 실제로 바뀐 경우에만 핏을 다시 계산한다.
  const prevContainerRef = useRef({ width: 0, height: 0 })

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

  // 윈도우 리사이즈(컨테이너 크기 변화) 시 핏 잠금이 유지 중이면 기억된 맞춤 모드를
  // 다시 적용한다. 수동 줌으로 잠금이 풀렸거나 single 모드가 아니면 기존 clamp만 유지한다.
  useLayoutEffect(() => {
    if (!app.imageInfo) return
    const cw = app.containerSize.width
    const ch = app.containerSize.height
    if (prevContainerRef.current.width === cw && prevContainerRef.current.height === ch) {
      return
    }
    prevContainerRef.current = { width: cw, height: ch }
    if (viewMode !== "single") return
    if (!isFitLocked) return
    if (app.isDragging) return
    const oriented = getOrientedImageSize(app.imageSize.width, app.imageSize.height, app.rotation)
    const iw = oriented.width
    const ih = oriented.height
    if (cw <= 0 || ch <= 0 || iw <= 0 || ih <= 0) return
    const zoom = getZoomForFitMode(fitMode, cw, ch, iw, ih)
    if (!Number.isFinite(zoom) || zoom <= 0) return
    if (Math.abs(zoom - app.zoom) < 1e-9) return
    prevZoomRef.current = zoom
    useAppStore.setState({ zoom, position: { x: 0, y: 0 } })
  }, [
    app.imageInfo,
    app.containerSize,
    app.imageSize,
    app.rotation,
    app.zoom,
    app.isDragging,
    viewMode,
    fitMode,
    isFitLocked
  ])

  // 줌 값이 바뀔 때 기존 position 비율을 유지하면서,
  // 컨테이너를 벗어나지 않도록 clamp 해서 position을 재계산
  useLayoutEffect(() => {
    if (prevZoomRef.current === app.zoom) return
    const ratio = app.zoom / prevZoomRef.current
    prevZoomRef.current = app.zoom
    const base = app.position
    const newX = base.x * ratio
    const newY = base.y * ratio
    const cw = app.containerSize.width
    const ch = app.containerSize.height
    const oriented = getOrientedImageSize(app.imageSize.width, app.imageSize.height, app.rotation)
    const iw = oriented.width
    const ih = oriented.height
    let next = { x: newX, y: newY }
    if (cw > 0 && ch > 0 && iw > 0 && ih > 0) {
      const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, app.zoom)
      next = clampPosition(newX, newY, maxX, maxY)
    }
    // 이미지 전환처럼 위치가 이미 (0,0)이면 같은 값을 다시 넣어 렌더를 늘리지 않는다.
    if (next.x === base.x && next.y === base.y) return
    useAppStore.setState({ position: next })
  }, [app.zoom, app.position, app.containerSize, app.imageSize, app.rotation])
}
