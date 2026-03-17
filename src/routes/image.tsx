import { createFileRoute, redirect } from "@tanstack/react-router"
import { useImageViewer } from "@/hooks/useImageViewer"
import { ImageContainer } from "@/components/ImageContainer"
import { getApp } from "@/store/appStore"

export const Route = createFileRoute("/image")({
  beforeLoad: () => {
    if (!getApp().imageInfo) {
      throw redirect({ to: "/" })
    }
  },
  component: ImagePage
})

function ImagePage() {
  const {
    containerRef,
    imageRef,
    navigateImage,
    navigateToIndex,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    handleDrop,
    handleDragOver,
    handleContextMenu
  } = useImageViewer()

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
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onNavigate={navigateImage}
        onNavigateToIndex={navigateToIndex}
      />
    </div>
  )
}
