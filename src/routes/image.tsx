import { createFileRoute, redirect } from "@tanstack/react-router"
import { ImageContainer } from "@/components/ImageContainer"
import { getApp, useAppStore } from "@/store/appStore"
import { useDirectoryNavigation } from "@/hooks/useDirectoryNavigation"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useImageViewerContextMenu } from "@/hooks/useContextMenu"
import { useImageViewerHotkeys } from "@/hooks/useImageViewerHotkeys"
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { useViewerElements } from "@/hooks/useViewerElements"
import { useWheelNavigation } from "@/hooks/useWheelNavigation"
import { useZoomPan } from "@/hooks/useZoomPan"
import {
  fitToHeight,
  fitToScreen,
  fitToWidth,
  resetZoomPan,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import { useCallback } from "react"

export const Route = createFileRoute("/image")({
  beforeLoad: () => {
    if (!getApp().imageInfo) {
      throw redirect({ to: "/" })
    }
  },
  component: ImagePage
})

function ImagePage() {
  const { containerRef, imageRef } = useViewerElements()
  const zoomPan = useZoomPan()
  const { loadImage, handleOpenFile, handleDrop, handleDragOver } =
    useImageLoader()
  const loadImageAndReset = useCallback(
    (filePath: string, options?: { refreshDirectory?: boolean }) =>
      loadImage(filePath, { ...options, onAfterLoad: zoomPan.resetView }),
    [loadImage, zoomPan.resetView]
  )
  const { navigateImage, navigateToIndex } =
    useDirectoryNavigation(loadImageAndReset)
  const handleWheel = useWheelNavigation(zoomPan, navigateImage)

  useOpenFileListener(loadImageAndReset)
  const app = useAppStore()

  const handleContextMenu = useImageViewerContextMenu(app.dirImages, {
    onOpenFile: handleOpenFile,
    onNavigatePrev: () => void navigateImage("prev"),
    onNavigateNext: () => void navigateImage("next"),
    onZoomIn: zoomIn,
    onZoomOut: zoomOut,
    onResetZoom: resetZoomPan,
    onFitWidth: fitToWidth,
    onFitHeight: fitToHeight,
    onFitScreen: fitToScreen
  })

  useImageViewerHotkeys({
    onNavigatePrev: () => void navigateImage("prev"),
    onNavigateNext: () => void navigateImage("next"),
    onZoomIn: zoomIn,
    onZoomOut: zoomOut,
    onResetZoom: resetZoomPan,
    onFitWidth: fitToWidth,
    onFitHeight: fitToHeight,
    onFitScreen: fitToScreen,
    onOpenFile: handleOpenFile
  })

  return (
    <div
      className="flex h-full w-full flex-col"
      onContextMenu={handleContextMenu}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
    >
      <ImageContainer
        containerRef={containerRef}
        imageRef={imageRef}
        onWheel={handleWheel}
        onMouseDown={zoomPan.handleMouseDown}
        onMouseMove={zoomPan.handleMouseMove}
        onMouseUp={zoomPan.handleMouseUp}
        onNavigate={navigateImage}
        onNavigateToIndex={navigateToIndex}
      />
    </div>
  )
}
