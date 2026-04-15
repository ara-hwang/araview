import type { RefObject } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { cn } from "@/lib/utils"
import { Spinner } from "@/components/ui/spinner"
import { ImageNavBar } from "./ImageNavBar"
import { useAppStore } from "@/store/appStore"
import { useShallow } from "zustand/react/shallow"
import type { ViewMode } from "@/store/settingsStore"
import type { MultiPage } from "@/hooks/useMultiPageImages"
import type { ImageInfo } from "@/types"

type ImageContainerProps = {
  containerRef: RefObject<HTMLDivElement>
  imageRef: RefObject<HTMLImageElement>
  onWheel: (e: React.WheelEvent) => void
  onMouseDown: (e: React.MouseEvent) => void
  onMouseMove: (e: React.MouseEvent) => void
  onMouseUp: () => void
  onNavigate?: (direction: "prev" | "next") => void
  onNavigateToIndex?: (index: number) => void
  viewMode?: ViewMode
  pages?: MultiPage[]
}

const toSrc = (info: ImageInfo) => convertFileSrc(info.file_path)

export function ImageContainer({
  containerRef,
  imageRef,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  onNavigate,
  onNavigateToIndex,
  viewMode = "single",
  pages = []
}: ImageContainerProps) {
  const app = useAppStore(
    useShallow((state) => ({
      loading: state.loading,
      error: state.error,
      dirImages: state.dirImages,
      imageInfo: state.imageInfo,
      position: state.position,
      zoom: state.zoom,
      isDragging: state.isDragging,
      rotation: state.rotation,
      flipH: state.flipH,
      flipV: state.flipV
    }))
  )

  const showNavBar = !!(
    app.dirImages.images.length > 1 &&
    onNavigate &&
    onNavigateToIndex
  )

  const imageSrc = app.imageInfo ? toSrc(app.imageInfo) : null

  const isMulti = viewMode !== "single" && pages.length > 0

  return (
    <div
      ref={containerRef}
      className={cn(
        "relative flex flex-1 items-center justify-center overflow-hidden bg-[hsl(var(--background))]",
        app.isDragging && viewMode === "single" && "[&]:cursor-grabbing"
      )}
      onWheel={onWheel}
      onMouseDown={viewMode === "single" ? onMouseDown : undefined}
      onMouseMove={viewMode === "single" ? onMouseMove : undefined}
      onMouseUp={viewMode === "single" ? onMouseUp : undefined}
      onMouseLeave={viewMode === "single" ? onMouseUp : undefined}
    >
      {/* Single mode: 기존 줌/팬 동작 */}
      {!isMulti && imageSrc && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <img
            ref={imageRef}
            src={imageSrc}
            alt={app.imageInfo?.file_name}
            className="pointer-events-auto max-h-full max-w-full object-contain transition-transform duration-50 ease-out"
            style={{
              transform: `translate(${app.position.x}px, ${app.position.y}px) scale(${app.zoom * (app.flipH ? -1 : 1)}, ${app.zoom * (app.flipV ? -1 : 1)}) rotate(${app.rotation}deg)`,
              transformOrigin: "center center",
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            draggable={false}
          />
        </div>
      )}

      {/* Dual page: LTR / RTL */}
      {isMulti &&
        (viewMode === "left-to-right" || viewMode === "right-to-left") && (
          <div
            className={cn(
              "absolute inset-0 flex items-center justify-center gap-1 p-2",
              viewMode === "right-to-left" && "flex-row-reverse"
            )}
          >
            {pages.map((page) => (
              <img
                key={page.path}
                src={toSrc(page.info)}
                alt={page.info.file_name}
                className="max-h-full max-w-[50%] object-contain"
                draggable={false}
              />
            ))}
          </div>
        )}

      {/* Webtoon: 세로 스크롤 */}
      {isMulti && viewMode === "webtoon" && (
        <div className="absolute inset-0 overflow-y-auto overflow-x-hidden">
          <div className="flex flex-col items-center">
            {pages.map((page) => (
              <img
                key={page.path}
                src={toSrc(page.info)}
                alt={page.info.file_name}
                className="max-w-full object-contain"
                draggable={false}
              />
            ))}
          </div>
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
