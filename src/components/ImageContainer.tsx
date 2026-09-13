import { useLayoutEffect, useRef, type RefObject } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { useNavigate } from "@tanstack/react-router"
import { toast } from "sonner"
import { cn } from "@/lib/utils"
import { Spinner } from "@/components/ui/spinner"
import { Button } from "@/components/ui/button"
import { ImageNavBar } from "./ImageNavBar"
import {
  applyImageNaturalSize,
  closeImage,
  useAppStore
} from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { classifyError } from "@/utils/appError"
import { MAX_SKIP_ATTEMPTS, findSkipTarget } from "@/utils/skipBroken"
import i18n from "@/i18n"
import { useShallow } from "zustand/react/shallow"
import { useTranslation } from "react-i18next"
import type { ViewMode } from "@/store/settingsStore"
import type { MultiPage } from "@/hooks/useMultiPageImages"
import type { ImageInfo } from "@/types"

import {
  WebtoonContinuousView,
  type WebtoonScrollTarget
} from "./WebtoonContinuousView"

type ImageContainerProps = {
  containerRef: RefObject<HTMLDivElement>
  imageRef: RefObject<HTMLImageElement>
  onWheel: (e: React.WheelEvent) => void
  onMouseDown: (e: React.MouseEvent) => void
  onMouseMove: (e: React.MouseEvent) => void
  onMouseUp: () => void
  onDoubleClick?: (e: React.MouseEvent) => void
  onMiddleClick?: (e: React.MouseEvent) => void
  onNavigate?: (direction: "prev" | "next") => void
  onNavigateToIndex?: (index: number) => void
  onToggleGrid?: () => void
  gridActive?: boolean
  getOrLoadImage?: (filePath: string) => Promise<ImageInfo>
  viewMode?: ViewMode
  pages?: MultiPage[]
  slideshowActive?: boolean
  slideshowIntervalMs?: number
  onToggleSlideshow?: () => void
  onToggleFullscreen?: () => void
  /** UI 자동 숨김(프레젠테이션) 시 하단 내비게이션 바 숨김 */
  chromeHidden?: boolean
  onRetry?: () => void
  onWebtoonIndexChange?: (index: number) => void
  webtoonScrollTarget?: WebtoonScrollTarget
}

const toSrc = (info: ImageInfo) => convertFileSrc(info.file_path)

export function ImageContainer({
  containerRef,
  imageRef,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  onDoubleClick,
  onMiddleClick,
  onNavigate,
  onNavigateToIndex,
  onToggleGrid,
  gridActive = false,
  getOrLoadImage,
  viewMode = "single",
  pages = [],
  slideshowActive = false,
  slideshowIntervalMs = 3000,
  onToggleSlideshow,
  onToggleFullscreen,
  chromeHidden = false,
  onRetry,
  onWebtoonIndexChange,
  webtoonScrollTarget
}: ImageContainerProps) {
  const { t } = useTranslation()
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

  const viewerBackground = useSettingsStore((state) => state.viewerBackground)
  const isChecker = viewerBackground === "checker"

  const imageSrc = app.imageInfo ? toSrc(app.imageInfo) : null
  const navigate = useNavigate()
  const classified = app.error ? classifyError(app.error) : null
  const tx = t as unknown as (key: string) => string
  const classifiedTitle = classified ? tx(classified.titleKey) : null
  const classifiedHint = classified ? tx(classified.hintKey) : null
  /** 디코드 실패 연쇄 스킵이 폴더 전체를 훑지 않도록 상한 */
  const skipChainRef = useRef(0)

  const handleImageError = () => {
    const st = useAppStore.getState()
    const { dirImages, imageInfo } = st
    if (!imageInfo) return
    const failedKey =
      dirImages.images[dirImages.current_index] ?? imageInfo.file_path
    st.addFailedPath(failedKey)
    if (!useSettingsStore.getState().skipBrokenFiles) return
    if (!onNavigateToIndex || dirImages.images.length <= 1) return
    if (skipChainRef.current >= MAX_SKIP_ATTEMPTS) return
    const settings = useSettingsStore.getState()
    const target = findSkipTarget(
      dirImages.images,
      dirImages.current_index,
      new Set(useAppStore.getState().failedPaths),
      settings.loopNavigation
    )
    if (target === null) return
    skipChainRef.current += 1
    toast.info(i18n.t("toast.load.skipped"), {
      description: imageInfo.file_name
    })
    void onNavigateToIndex(target)
  }

  const handleGoHome = () => {
    closeImage()
    void navigate({ to: "/" })
  }

  useLayoutEffect(() => {
    const img = imageRef.current
    if (img?.complete && img.naturalWidth > 0) {
      applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
    }
  }, [imageSrc, imageRef])

  const isDual =
    (viewMode === "left-to-right" || viewMode === "right-to-left") &&
    pages.length > 0
  const isWebtoon =
    viewMode === "webtoon" &&
    app.dirImages.images.length > 0 &&
    !!getOrLoadImage
  const isMulti = isDual || isWebtoon

  return (
    <div
      ref={containerRef}
      className={cn(
        "relative flex flex-1 items-center justify-center overflow-hidden transition-colors",
        viewerBackground === "theme" && "bg-[hsl(var(--background))]",
        viewerBackground === "black" && "bg-black",
        viewerBackground === "white" && "bg-white",
        isChecker && "bg-white",
        app.isDragging && viewMode === "single" && "[&]:cursor-grabbing"
      )}
      style={
        isChecker
          ? {
              backgroundImage:
                "conic-gradient(#c7c7c7 25%, #ffffff 0 50%, #c7c7c7 0 75%, #ffffff 0)",
              backgroundSize: "20px 20px"
            }
          : undefined
      }
      onWheel={onWheel}
      onMouseDown={viewMode === "single" ? onMouseDown : undefined}
      onMouseMove={viewMode === "single" ? onMouseMove : undefined}
      onMouseUp={viewMode === "single" ? onMouseUp : undefined}
      onMouseLeave={viewMode === "single" ? onMouseUp : undefined}
      onDoubleClick={onDoubleClick}
      onAuxClick={(e) => {
        if (e.button === 1 && onMiddleClick) onMiddleClick(e)
      }}
    >
      {/* Single mode: 기존 줌/팬 동작 */}
      {!isMulti && imageSrc && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <img
            key={imageSrc}
            ref={imageRef}
            src={imageSrc}
            alt={app.imageInfo?.file_name}
            className="pointer-events-auto block max-h-none max-w-none shrink-0"
            style={{
              width: app.imageSize.width || undefined,
              height: app.imageSize.height || undefined,
              maxWidth: "none",
              maxHeight: "none",
              transform: `translate3d(${app.position.x}px, ${app.position.y}px, 0) scale(${app.zoom * (app.flipH ? -1 : 1)}, ${app.zoom * (app.flipV ? -1 : 1)}) rotate(${app.rotation}deg)`,
              transformOrigin: "center center",
              willChange: "transform",
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            onLoad={(event) => {
              const img = event.currentTarget
              skipChainRef.current = 0
              applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
            }}
            onError={handleImageError}
            draggable={false}
          />
        </div>
      )}

      {/* Dual page: LTR / RTL. 마지막 홀수 장은 단일 중앙 표시 */}
      {isDual && (
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
              className={
                pages.length === 1
                  ? "max-h-full max-w-full object-contain"
                  : "max-h-full max-w-[50%] object-contain"
              }
              draggable={false}
            />
          ))}
        </div>
      )}

      {/* Webtoon: 전 구간 연속 스크롤 */}
      {isWebtoon && getOrLoadImage && (
        <WebtoonContinuousView
          images={app.dirImages.images}
          currentIndex={app.dirImages.current_index}
          getOrLoadImage={getOrLoadImage}
          onCenterChange={(i) => onWebtoonIndexChange?.(i)}
          scrollTarget={webtoonScrollTarget ?? null}
        />
      )}

      {app.loading && (
        <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
          <Spinner
            className="size-8 text-[hsl(var(--muted-foreground))]"
            aria-label={t("viewer.loading")}
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
              {t("viewer.slideshow.label", {
                index: app.dirImages.current_index + 1,
                total: app.dirImages.images.length,
                sec: (slideshowIntervalMs / 1000).toFixed(1)
              })}
            </span>
            <span className="flex gap-1">
              {onToggleFullscreen && (
                <button
                  type="button"
                  onClick={onToggleFullscreen}
                  className="rounded px-1.5 py-0.5 hover:bg-[hsl(var(--accent))] focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:outline-none"
                  title={t("viewer.slideshow.fullscreenTitle")}
                  aria-label={t("viewer.slideshow.fullscreenTitle")}
                >
                  {t("viewer.slideshow.fullscreen")}
                </button>
              )}
              {onToggleSlideshow && (
                <button
                  type="button"
                  onClick={onToggleSlideshow}
                  className="rounded px-1.5 py-0.5 hover:bg-[hsl(var(--accent))] focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:outline-none"
                  title={t("viewer.slideshow.stopTitle")}
                  aria-label={t("viewer.slideshow.stopTitle")}
                >
                  {t("viewer.slideshow.stop")}
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
            {classifiedTitle}
          </p>
          <p className="text-muted-foreground mt-1 break-words">{app.error}</p>
          <p className="text-muted-foreground mt-1">{classifiedHint}</p>
          <div className="mt-3 flex gap-2">
            {onRetry && (
              <Button size="sm" onClick={onRetry}>
                {t("viewer.error.retry")}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={handleGoHome}>
              {t("viewer.error.home")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => useAppStore.setState({ error: null })}
            >
              {t("viewer.error.dismiss")}
            </Button>
          </div>
        </div>
      )}

      {showNavBar && onNavigate && onNavigateToIndex && getOrLoadImage && (
        <ImageNavBar
          onNavigate={onNavigate}
          onNavigateToIndex={onNavigateToIndex}
          getOrLoadImage={getOrLoadImage}
          onToggleGrid={onToggleGrid}
          gridActive={gridActive}
          hidden={chromeHidden}
        />
      )}
    </div>
  )
}
