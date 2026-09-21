import { useNavigate } from "@tanstack/react-router"
import { convertFileSrc } from "@tauri-apps/api/core"
import { useLayoutEffect, useRef, type RefObject } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toast } from "@/components/ui/toast"
import type { MultiPage } from "@/hooks/useMultiPageImages"
import i18n from "@/i18n"
import { cn } from "@/lib/utils"
import { applyImageNaturalSize, closeImage, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ViewMode } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { classifyError } from "@/utils/appError"
import { MAX_SKIP_ATTEMPTS, findSkipTarget } from "@/utils/skipBroken"

import { ArchivePreviewCallout } from "./ArchivePreviewCallout"
import { WebtoonContinuousView, type WebtoonScrollTarget } from "./WebtoonContinuousView"

type ImageContainerProps = {
  containerRef: RefObject<HTMLDivElement | null>
  imageRef: RefObject<HTMLImageElement | null>
  onWheel: (e: React.WheelEvent) => void
  onMouseDown: (e: React.MouseEvent) => void
  onMouseMove: (e: React.MouseEvent) => void
  onMouseUp: () => void
  onDoubleClick?: (e: React.MouseEvent) => void
  onMiddleClick?: (e: React.MouseEvent) => void
  onNavigateToIndex?: (index: number) => void
  getOrLoadImage?: (filePath: string) => Promise<ImageInfo>
  viewMode?: ViewMode
  pages?: MultiPage[]
  onRetry?: () => void
  onWebtoonIndexChange?: (index: number) => void
  webtoonScrollTarget?: WebtoonScrollTarget
  onOpenArchiveFromPreview?: () => void
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
  onNavigateToIndex,
  getOrLoadImage,
  viewMode = "single",
  pages = [],
  onRetry,
  onWebtoonIndexChange,
  webtoonScrollTarget,
  onOpenArchiveFromPreview
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
      imageSize: state.imageSize,
      archivePreviewPath: state.archivePreviewPath
    }))
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
    const failedKey = dirImages.images[dirImages.current_index] ?? imageInfo.file_path
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

  const isDual = (viewMode === "left-to-right" || viewMode === "right-to-left") && pages.length > 0
  const isWebtoon = viewMode === "webtoon" && app.dirImages.images.length > 0 && !!getOrLoadImage
  const isMulti = isDual || isWebtoon

  const isSvgImage =
    app.imageInfo?.mime_type === "image/svg+xml" ||
    (app.imageInfo?.file_name.toLowerCase().endsWith(".svg") ?? false)
  // 벡터는 레이아웃 크기로 줌하고 transform에는 이동/반전/회전만 남긴다.
  // scale(zoom) 확대는 래스터를 늘려 흐릿해지지만, 레이아웃 크기면
  // 브라우저가 표시 크기에서 재래스터해 선명도가 유지된다.
  const svgSharpZoom = isSvgImage && app.imageSize.width > 0 && app.imageSize.height > 0
  const singleImgWidth = svgSharpZoom
    ? app.imageSize.width * app.zoom
    : app.imageSize.width || undefined
  const singleImgHeight = svgSharpZoom
    ? app.imageSize.height * app.zoom
    : app.imageSize.height || undefined
  const singleImgTransform = svgSharpZoom
    ? `translate3d(${app.position.x}px, ${app.position.y}px, 0) scale(${app.flipH ? -1 : 1}, ${app.flipV ? -1 : 1}) rotate(${app.rotation}deg)`
    : `translate3d(${app.position.x}px, ${app.position.y}px, 0) scale(${app.zoom * (app.flipH ? -1 : 1)}, ${app.zoom * (app.flipV ? -1 : 1)}) rotate(${app.rotation}deg)`

  return (
    <div
      ref={containerRef}
      className={cn(
        "relative flex flex-1 items-center justify-center overflow-hidden transition-colors",
        viewerBackground === "theme" && "bg-background",
        viewerBackground === "black" && "bg-black",
        viewerBackground === "white" && "bg-white",
        isChecker && "checkerboard",
        app.isDragging && viewMode === "single" && "[&]:cursor-grabbing"
      )}
      onWheel={onWheel}
      onMouseDown={viewMode === "single" ? onMouseDown : undefined}
      onMouseMove={viewMode === "single" ? onMouseMove : undefined}
      onMouseUp={viewMode === "single" ? onMouseUp : undefined}
      onMouseLeave={viewMode === "single" ? onMouseUp : undefined}
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
            className="pointer-events-auto block max-h-none max-w-none shrink-0 origin-center will-change-transform"
            style={{
              width: singleImgWidth,
              height: singleImgHeight,
              maxWidth: "none",
              maxHeight: "none",
              transform: singleImgTransform,
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            onLoad={(event) => {
              const img = event.currentTarget
              skipChainRef.current = 0
              applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
            }}
            onError={handleImageError}
            onDoubleClick={onDoubleClick}
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
              onDoubleClick={onDoubleClick}
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
          onImageDoubleClick={onDoubleClick}
        />
      )}

      {app.loading && (
        <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
          <Spinner className="size-8 text-muted-foreground" aria-label={t("viewer.loading")} />
        </div>
      )}

      {app.error && classified && (
        <div
          role="alert"
          className="max-w-md rounded-lg border border-destructive/20 bg-background px-6 py-4 text-sm shadow-lg"
        >
          <p className="font-medium text-destructive">{classifiedTitle}</p>
          <p className="mt-1 break-words text-muted-foreground">{app.error}</p>
          <p className="mt-1 text-muted-foreground">{classifiedHint}</p>
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

      {app.archivePreviewPath && onOpenArchiveFromPreview && (
        <ArchivePreviewCallout
          archivePath={app.archivePreviewPath}
          onOpenArchive={onOpenArchiveFromPreview}
        />
      )}
    </div>
  )
}
