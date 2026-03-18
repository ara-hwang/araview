import {
  useState,
  useCallback,
  useLayoutEffect,
  useRef,
  type RefObject
} from "react"
import {
  getPositionBounds,
  clampPosition,
  isFullyContained
} from "../utils/zoomPanUtils"
import {
  setDragging,
  setPosition,
  setZoom,
  useAppStore
} from "@/store/appStore"

// 이미지 컨테이너 내부에서 줌/팬(드래그) 상태를 관리하고
// 전역 `appStore`의 position/zoom 값을 일관되게 업데이트하는 훅

export function useZoomPan(
  containerRef: RefObject<HTMLDivElement | null>,
  imageRef: RefObject<HTMLImageElement | null>
) {
  const app = useAppStore()

  const [dragStart, setDragStart] = useState({ x: 0, y: 0 })
  const prevZoomRef = useRef(1)

  const getFitZoom = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return 1
    const cw = containerRef.current.offsetWidth
    const ch = containerRef.current.offsetHeight
    const iw = imageRef.current.offsetWidth
    const ih = imageRef.current.offsetHeight
    if (iw <= 0 || ih <= 0) return 1
    const fit = Math.min(cw / iw, ch / ih)
    return fit * (1 + 1e-6)
  }, [containerRef, imageRef])

  const handleZoomIn = useCallback(() => {
    setZoom(Math.min(app.zoom * 1.25, 10))
  }, [app.zoom])

  const handleZoomOut = useCallback(() => {
    const minZoom = getFitZoom()
    setZoom(Math.max(app.zoom / 1.25, minZoom))
  }, [getFitZoom, app.zoom])

  const handleResetZoom = useCallback(() => {
    setZoom(1)
    setPosition({ x: 0, y: 0 })
  }, [])

  const handleFitWidth = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return
    const containerW = containerRef.current.offsetWidth
    const imgW = imageRef.current.offsetWidth
    if (imgW === 0) return
    setZoom(containerW / imgW)
    setPosition({ x: 0, y: 0 })
  }, [containerRef, imageRef])

  const handleFitHeight = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return
    const containerH = containerRef.current.offsetHeight
    const imgH = imageRef.current.offsetHeight
    if (imgH === 0) return
    setZoom(containerH / imgH)
    setPosition({ x: 0, y: 0 })
  }, [containerRef, imageRef])

  const handleFitScreen = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return
    const cw = containerRef.current.offsetWidth
    const ch = containerRef.current.offsetHeight
    const iw = imageRef.current.offsetWidth
    const ih = imageRef.current.offsetHeight
    if (iw === 0 || ih === 0) return
    setZoom(Math.min(cw / iw, ch / ih))
    setPosition({ x: 0, y: 0 })
  }, [containerRef, imageRef])

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (app.imageInfo) {
        setDragging(true)
        setDragStart({
          x: e.clientX - app.position.x,
          y: e.clientY - app.position.y
        })
      }
    },
    [app.imageInfo, app.position]
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!app.isDragging) return
      const newX = e.clientX - dragStart.x
      const newY = e.clientY - dragStart.y
      if (containerRef.current && imageRef.current) {
        const cw = containerRef.current.offsetWidth
        const ch = containerRef.current.offsetHeight
        const iw = imageRef.current.offsetWidth
        const ih = imageRef.current.offsetHeight
        if (isFullyContained(cw, ch, iw, ih, app.zoom)) {
          setPosition({ x: 0, y: 0 })
          return
        }
        const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, app.zoom)
        setPosition(clampPosition(newX, newY, maxX, maxY))
      } else {
        setPosition({ x: newX, y: newY })
      }
    },
    [app.isDragging, dragStart, app.zoom, containerRef, imageRef]
  )

  const handleMouseUp = useCallback(() => {
    setDragging(false)
  }, [])

  // Ctrl + 휠로만 줌을 처리하고, 스크롤 이동은 상위 훅(useImageViewer)에서 처리
  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault()
      if (e.ctrlKey) {
        const minZoom = getFitZoom()
        if (e.deltaY < 0) {
          setZoom(Math.min(app.zoom * 1.1, 10))
        } else {
          setZoom(Math.max(app.zoom / 1.1, minZoom))
        }
      }
    },
    [getFitZoom, app.zoom]
  )

  // 현재 줌 배율에서 이미지가 컨테이너 안에 완전히 들어오면 위치를 (0,0)으로 정렬
  useLayoutEffect(() => {
    if (!app.imageInfo || !containerRef.current || !imageRef.current) return
    const cw = containerRef.current.offsetWidth
    const ch = containerRef.current.offsetHeight
    const iw = imageRef.current.offsetWidth
    const ih = imageRef.current.offsetHeight
    if (isFullyContained(cw, ch, iw, ih, app.zoom)) {
      setPosition({ x: 0, y: 0 })
    }
  }, [app.imageInfo, app.zoom, containerRef, imageRef])

  const resetView = useCallback(() => {
    setZoom(1)
    setPosition({ x: 0, y: 0 })
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
    if (!containerRef.current || !imageRef.current) {
      setPosition({ x: newX, y: newY })
    } else {
      const cw = containerRef.current.offsetWidth
      const ch = containerRef.current.offsetHeight
      const iw = imageRef.current.offsetWidth
      const ih = imageRef.current.offsetHeight
      const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, app.zoom)
      setPosition(clampPosition(newX, newY, maxX, maxY))
    }
    prevZoomRef.current = app.zoom
  }, [app.zoom, app.position, containerRef, imageRef])

  return {
    position: app.position,
    isDragging: app.isDragging,
    resetView,
    getFitZoom,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleFitWidth,
    handleFitHeight,
    handleFitScreen,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp
  }
}
