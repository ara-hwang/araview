import type { RefObject } from "react"
import { cn } from "@/lib/utils"
import { Spinner } from "@/components/ui/spinner"
import { ImageNavBar } from "./ImageNavBar"
import { useAppStore } from "@/store/appStore"
import { useShallow } from "zustand/react/shallow"

type ImageContainerProps = {
  containerRef: RefObject<HTMLDivElement>
  imageRef: RefObject<HTMLImageElement>
  onWheel: (e: React.WheelEvent) => void
  onMouseDown: (e: React.MouseEvent) => void
  onMouseMove: (e: React.MouseEvent) => void
  onMouseUp: () => void
  onNavigate?: (direction: "prev" | "next") => void
  onNavigateToIndex?: (index: number) => void
}

export function ImageContainer({
  containerRef,
  imageRef,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  onNavigate,
  onNavigateToIndex
}: ImageContainerProps) {
  const app = useAppStore(
    useShallow((state) => ({
      loading: state.loading,
      error: state.error,
      dirImages: state.dirImages,
      imageInfo: state.imageInfo,
      position: state.position,
      zoom: state.zoom,
      isDragging: state.isDragging
    }))
  )

  const showNavBar = !!(
    app.dirImages.images.length > 1 &&
    onNavigate &&
    onNavigateToIndex
  )

  const imageSrc = app.imageInfo
    ? `data:${app.imageInfo.mime_type};base64,${app.imageInfo.base64}`
    : null

  return (
    <div
      ref={containerRef}
      className={cn(
        "relative flex flex-1 items-center justify-center overflow-hidden bg-[hsl(var(--background))]",
        app.isDragging && "[&]:cursor-grabbing"
      )}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
    >
      {imageSrc && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <img
            ref={imageRef}
            src={imageSrc}
            alt={app.imageInfo?.file_name}
            className="pointer-events-auto max-h-full max-w-full object-contain transition-transform duration-50 ease-out"
            style={{
              transform: `translate(${app.position.x}px, ${app.position.y}px) scale(${app.zoom})`,
              transformOrigin: "center center",
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            draggable={false}
          />
        </div>
      )}

      {app.loading && (
        <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
          <Spinner
            className="size-8 text-[hsl(var(--muted-foreground))]"
            aria-label="Loading image"
          />
        </div>
      )}

      {app.error && (
        <div className="rounded-lg border border-[hsl(var(--destructive))]/20 bg-[hsl(var(--destructive))]/10 px-6 py-4 text-sm text-[hsl(var(--destructive))]">
          {app.error}
        </div>
      )}

      {showNavBar && onNavigate && onNavigateToIndex && (
        <ImageNavBar
          onNavigate={onNavigate}
          onNavigateToIndex={onNavigateToIndex}
        />
      )}
    </div>
  )
}
