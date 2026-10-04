import { useNavigate } from "@tanstack/react-router"
import { convertFileSrc } from "@tauri-apps/api/core"
import { useLayoutEffect, useRef, useState, type RefObject } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { DualPageSpread } from "@/components/DualPageSpread"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toast } from "@/components/ui/toast"
import { useGifPlayer } from "@/hooks/useGifPlayer"
import type { MultiPage } from "@/hooks/useMultiPageImages"
import { usePixelArtDetection } from "@/hooks/usePixelArtDetection"
import i18n from "@/i18n"
import { cn } from "@/lib/utils"
import { applyImageNaturalSize, closeImage, useAppStore } from "@/store/appStore"
import { useGifStore } from "@/store/gifStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ViewMode } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { classifyError } from "@/utils/appError"
import { canControlGif } from "@/utils/gifPlayback"
import {
  getPixelArtDetectionPath,
  isMagnifiedDisplay,
  isSvgImageInfo,
  resolveImageRenderingMode
} from "@/utils/imageRendering"
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
  onOpenThumbnailGrid?: (trigger?: HTMLButtonElement) => void
  onOpenArchiveFromPreview?: () => void
  /** 웹툰 연속 뷰 안의 폴더 아카이브를 만화로 연다 */
  onOpenArchive?: (path: string) => void
}

const toSrc = (info: ImageInfo) => convertFileSrc(info.file_path)
const toPageSrc = (page: MultiPage) => toSrc(page.info)

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
  onOpenThumbnailGrid,
  onOpenArchiveFromPreview,
  onOpenArchive
}: ImageContainerProps) {
  const { t } = useTranslation()
  const app = useAppStore(
    useShallow((state) => ({
      loading: state.loading,
      error: state.error,
      errorCode: state.errorCode,
      dirImages: state.dirImages,
      imageInfo: state.imageInfo,
      position: state.position,
      zoom: state.zoom,
      isDragging: state.isDragging,
      rotation: state.rotation,
      flipH: state.flipH,
      flipV: state.flipV,
      imageSize: state.imageSize,
      archivePreviewPath: state.archivePreviewPath,
      previewPath: state.previewPath
    }))
  )

  const webtoonSettings = useSettingsStore(
    useShallow((state) => ({
      viewerBackground: state.viewerBackground,
      webtoonImageGap: state.webtoonImageGap,
      webtoonPageBoundaries: state.webtoonPageBoundaries,
      webtoonFitWidth: state.webtoonFitWidth,
      webtoonShowProgress: state.webtoonShowProgress,
      webtoonThumbnailJump: state.webtoonThumbnailJump,
      imageScalingMode: state.imageScalingMode,
      autoDetectPixelArt: state.autoDetectPixelArt
    }))
  )
  const viewerBackground = webtoonSettings.viewerBackground
  const isChecker = viewerBackground === "checker"

  const imageSrc = app.imageInfo ? toSrc(app.imageInfo) : null
  const previewSrc = app.previewPath ? convertFileSrc(app.previewPath) : null
  // 풀사이즈가 실제로 로드된 src. 프리뷰는 그 전까지만 깔린다.
  const [loadedSrc, setLoadedSrc] = useState<string | null>(null)
  const fullLoaded = imageSrc !== null && loadedSrc === imageSrc
  const navigate = useNavigate()
  const classified = app.error ? classifyError({ code: app.errorCode, message: app.error }) : null
  const tx = t as unknown as (key: string) => string
  const classifiedTitle = classified ? tx(classified.titleKey) : null
  const classifiedHint = classified ? tx(classified.hintKey) : null
  /** 디코드 실패 연쇄 스킵이 폴더 전체를 훑지 않도록 상한 */
  const skipChainRef = useRef(0)

  const handleImageError = () => {
    useAppStore.setState({ previewPath: null })
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
      // 캐시 적중으로 onLoad가 이미 지나간 경우에도 프리뷰를 걷고 원본을 표시한다.
      if (imageSrc) setLoadedSrc(imageSrc)
      useAppStore.setState({ previewPath: null })
      applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
    }
  }, [imageSrc, imageRef])

  const isDual = (viewMode === "left-to-right" || viewMode === "right-to-left") && pages.length > 0
  const isWebtoon = viewMode === "webtoon" && app.dirImages.images.length > 0 && !!getOrLoadImage
  const isMulti = isDual || isWebtoon
  // 단일 보기 배율은 imageSize(표시 바이트 기준 크기)에 대한 zoom이다.
  // 축소 표시에서는 pixelated 판정을 끄고 항상 부드럽게 보간한다.
  const displayScale = app.imageSize.width > 0 ? app.zoom : null
  // 판정은 원본을 백엔드에서 한 번 더 디코드한다. 축소 표시에서는 결과를 쓰지
  // 않으므로 요청하지 않고, 확대로 바뀌면 그때 요청한다.
  const currentPixelArtDetection = usePixelArtDetection(
    app.imageInfo ? getPixelArtDetectionPath(app.imageInfo) : null,
    app.imageInfo?.file_size,
    !isMulti &&
      webtoonSettings.imageScalingMode === "auto" &&
      webtoonSettings.autoDetectPixelArt &&
      isMagnifiedDisplay(displayScale),
    !isMulti
  )
  const imageRendering = resolveImageRenderingMode(
    webtoonSettings.imageScalingMode,
    webtoonSettings.autoDetectPixelArt,
    currentPixelArtDetection,
    displayScale
  )
  const isPixelated = imageRendering === "pixelated"

  // GIF는 단일 보기에서만 캔버스로 제어한다(재생/정지/프레임 이동).
  const gifCanvasRef = useRef<HTMLCanvasElement>(null)
  const gifControllable = canControlGif(app.imageInfo, viewMode)
  const gifActive = useGifStore((state) => state.active)
  const { failed: gifFailed } = useGifPlayer(
    gifCanvasRef,
    gifControllable ? imageSrc : null,
    gifControllable,
    !isPixelated
  )
  // 디코더가 다중 프레임을 확인해 active가 되기 전에는 기존 <img>로 보여준다
  // (정지 GIF가 빈 캔버스가 되는 것을 막는다).
  const showGifCanvas = gifControllable && gifActive && !gifFailed

  const isSvgImage = isSvgImageInfo(app.imageInfo)
  // 벡터는 레이아웃 크기로 줌하고 transform에는 이동/반전/회전만 남긴다.
  // scale(zoom) 확대는 래스터를 늘려 흐릿해지지만, 레이아웃 크기면
  // 브라우저가 표시 크기에서 재래스터해 선명도가 유지된다.
  const svgSharpZoom = isSvgImage && app.imageSize.width > 0 && app.imageSize.height > 0
  // 픽셀 보존 모드(확대 시)는 transform 합성에서 다시 보간되지 않도록
  // 레이아웃 크기로 확대한다. 축소 배율에서는 transform scale(zoom)로
  // 부드럽게 줄인다.
  const layoutZoom =
    (svgSharpZoom || (isPixelated && displayScale !== null && displayScale >= 1)) &&
    app.imageSize.width > 0 &&
    app.imageSize.height > 0
  const singleImgWidth = layoutZoom
    ? app.imageSize.width * app.zoom
    : app.imageSize.width || undefined
  const singleImgHeight = layoutZoom
    ? app.imageSize.height * app.zoom
    : app.imageSize.height || undefined
  const singleImgTransform = layoutZoom
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
      onMouseDown={viewMode === "single" || isDual ? onMouseDown : undefined}
      onMouseMove={viewMode === "single" ? onMouseMove : undefined}
      onMouseUp={viewMode === "single" ? onMouseUp : undefined}
      onMouseLeave={viewMode === "single" ? onMouseUp : undefined}
      onAuxClick={(e) => {
        if (e.button === 1 && onMiddleClick) onMiddleClick(e)
      }}
    >
      {/* 큰 이미지의 캐시된 저해상 프리뷰. 풀사이즈 onLoad 전까지만 깔린다. */}
      {!isMulti && previewSrc && !fullLoaded && !showGifCanvas && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <img
            src={previewSrc}
            alt=""
            aria-hidden="true"
            className={cn(
              "block max-h-none max-w-none shrink-0 origin-center",
              imageRendering === "pixelated"
                ? "image-rendering-pixelated"
                : "image-rendering-smooth"
            )}
            style={{
              width: singleImgWidth,
              height: singleImgHeight,
              maxWidth: "none",
              maxHeight: "none",
              transform: singleImgTransform
            }}
            draggable={false}
          />
        </div>
      )}

      {/* Single mode: 기존 줌/팬 동작 */}
      {!isMulti && imageSrc && !showGifCanvas && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <img
            key={imageSrc}
            ref={imageRef}
            src={imageSrc}
            alt={app.imageInfo?.file_name}
            className={cn(
              "pointer-events-auto block max-h-none max-w-none shrink-0 origin-center will-change-transform",
              // 페이드 없이 바로 바꾼다. opacity 트랜지션을 두면 이미 로드된
              // 이미지도 전환마다 배경이 비쳤다가 나타난다.
              fullLoaded ? "opacity-100" : "opacity-0",
              imageRendering === "pixelated"
                ? "image-rendering-pixelated"
                : "image-rendering-smooth"
            )}
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
              setLoadedSrc(imageSrc)
              useAppStore.setState({ previewPath: null })
              applyImageNaturalSize(img.naturalWidth, img.naturalHeight)
            }}
            onError={handleImageError}
            onDoubleClick={onDoubleClick}
            draggable={false}
          />
        </div>
      )}

      {/* GIF 캔버스: 프레임 제어가 가능할 때만. 줌/팬 transform은 img와 동일하다. */}
      {!isMulti && showGifCanvas && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <canvas
            ref={gifCanvasRef}
            role="img"
            aria-label={app.imageInfo?.file_name}
            className={cn(
              "pointer-events-auto block max-h-none max-w-none shrink-0 origin-center will-change-transform",
              imageRendering === "pixelated"
                ? "image-rendering-pixelated"
                : "image-rendering-smooth"
            )}
            style={{
              width: singleImgWidth,
              height: singleImgHeight,
              maxWidth: "none",
              maxHeight: "none",
              transform: singleImgTransform,
              cursor: app.isDragging ? "grabbing" : "grab"
            }}
            onDoubleClick={onDoubleClick}
          />
        </div>
      )}

      {/* Dual page: LTR / RTL. 마지막 홀수 장은 단일 중앙 표시 */}
      {isDual && (
        <DualPageSpread
          key={pages.map((page) => page.path).join("\n")}
          pages={pages}
          reversed={viewMode === "right-to-left"}
          src={toPageSrc}
          scalingMode={webtoonSettings.imageScalingMode}
          autoDetectPixelArt={webtoonSettings.autoDetectPixelArt}
          onDoubleClick={onDoubleClick}
        />
      )}

      {/* Webtoon: 전 구간 연속 스크롤 */}
      {isWebtoon && getOrLoadImage && (
        <WebtoonContinuousView
          images={app.dirImages.images}
          currentIndex={app.dirImages.current_index}
          getOrLoadImage={getOrLoadImage}
          onCenterChange={(i) => onWebtoonIndexChange?.(i)}
          scrollTarget={webtoonScrollTarget ?? null}
          imageGap={webtoonSettings.webtoonImageGap}
          showPageBoundaries={webtoonSettings.webtoonPageBoundaries}
          fitWidth={webtoonSettings.webtoonFitWidth}
          imageScalingMode={webtoonSettings.imageScalingMode}
          autoDetectPixelArt={webtoonSettings.autoDetectPixelArt}
          showProgress={webtoonSettings.webtoonShowProgress}
          thumbnailJump={webtoonSettings.webtoonThumbnailJump}
          onOpenThumbnailGrid={onOpenThumbnailGrid}
          onOpenArchive={onOpenArchive}
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
              onClick={() => useAppStore.setState({ error: null, errorCode: null })}
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
