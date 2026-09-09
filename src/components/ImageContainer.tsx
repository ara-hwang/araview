import { useLayoutEffect, type RefObject } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { useNavigate } from "@tanstack/react-router"
import { cn } from "@/lib/utils"
import { Spinner } from "@/components/ui/spinner"
import { Button } from "@/components/ui/button"
import { ImageNavBar } from "./ImageNavBar"
import { applyImageNaturalSize, useAppStore } from "@/store/appStore"
import { classifyError } from "@/utils/appError"
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
  getOrLoadImage?: (filePath: string) => Promise<ImageInfo>
  viewMode?: ViewMode
  pages?: MultiPage[]
  slideshowActive?: boolean
  slideshowIntervalMs?: number
  onToggleSlideshow?: () => void
  onToggleFullscreen?: () => void
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
  getOrLoadImage,
  viewMode = "single",
  pages = [],
  slideshowActive = false,
  slideshowIntervalMs = 3000,
  onToggleSlideshow,
  onToggleFullscreen
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
      flipV: state.flipV,
      imageSize: state.imageSize
    }))
  )

  const showNavBar = !!(
    app.dirImages.images.length > 1 &&
    onNavigate &&
    onNavigateToIndex &&
    getOrLoadImage
  )

  const imageSrc = app.imageInfo ? toSrc(app.imageInfo) : null
  const navigate = useNavigate()
  const classified = app.error ? classifyError(app.error) : null

  const handleGoHome = () => {
    useAppStore.setState({ error: null })
    void navigate({ to: "/" })
  }

  useLayoutEffect(() => {
    const img = imageRef.current
    if (img?.complete && img.naturalWidth > 0) {
      applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
    }
  }, [imageSrc, imageRef])

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
            className="pointer-events-auto object-contain transition-transform duration-50 ease-out"
            style={{
              width: app.imageSize.width || undefined,
              height: app.imageSize.height || undefined,
              transform: `translate(${app.position.x}px, ${app.position.y}px) scale(${app.zoom * (app.flipH ? -1 : 1)}, ${app.zoom * (app.flipV ? -1 : 1)}) rotate(${app.rotation}deg)`,
              transformOrigin: "center center",
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            onLoad={(event) => {
              const img = event.currentTarget
              applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
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
        <div className="absolute inset-0 overflow-x-hidden overflow-y-auto">
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

      {slideshowActive && (
        <div
          className="bg-background/90 border-border absolute top-2 left-1/2 z-20 w-64 -translate-x-1/2 rounded-md border px-3 py-2 shadow-lg"
          onMouseDown={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between gap-2 text-xs">
            <span aria-live="polite">
              슬라이드쇼 {app.dirImages.current_index + 1}/
              {app.dirImages.images.length} ·{" "}
              {(slideshowIntervalMs / 1000).toFixed(1)}초
            </span>
            <span className="flex gap-1">
              {onToggleFullscreen && (
                <button
                  type="button"
                  onClick={onToggleFullscreen}
                  className="rounded px-1.5 py-0.5 hover:bg-[hsl(var(--accent))]"
                  title="전체화면 (F11)"
                  aria-label="전체화면 (F11)"
                >
                  전체화면
                </button>
              )}
              {onToggleSlideshow && (
                <button
                  type="button"
                  onClick={onToggleSlideshow}
                  className="rounded px-1.5 py-0.5 hover:bg-[hsl(var(--accent))]"
                  title="슬라이드쇼 정지 (Space)"
                  aria-label="슬라이드쇼 정지 (Space)"
                >
                  정지
                </button>
              )}
            </span>
          </div>
          <div className="bg-muted mt-1.5 h-1 overflow-hidden rounded-full">
            <div
              key={`${app.dirImages.current_index}-${slideshowIntervalMs}`}
              className="bg-primary h-full"
              style={{
                animation: `slideshow-progress ${slideshowIntervalMs}ms linear forwards`
              }}
            />
          </div>
        </div>
      )}

      {app.error && classified && (
        <div
          role="alert"
          className="bg-background max-w-md rounded-lg border border-[hsl(var(--destructive))]/20 px-6 py-4 text-sm shadow-lg"
        >
          <p className="font-medium text-[hsl(var(--destructive))]">
            {classified.title}
          </p>
          <p className="text-muted-foreground mt-1 break-words">{app.error}</p>
          <p className="text-muted-foreground mt-1">{classified.hint}</p>
          <div className="mt-3 flex gap-2">
            <Button size="sm" onClick={handleGoHome}>
              홈으로 돌아가기
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => useAppStore.setState({ error: null })}
            >
              닫기
            </Button>
          </div>
        </div>
      )}

      {showNavBar && onNavigate && onNavigateToIndex && getOrLoadImage && (
        <ImageNavBar
          onNavigate={onNavigate}
          onNavigateToIndex={onNavigateToIndex}
          getOrLoadImage={getOrLoadImage}
        />
      )}
    </div>
  )
}
