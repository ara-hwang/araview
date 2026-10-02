import { SquaresFour } from "@phosphor-icons/react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { memo, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { PixelArtImage } from "@/components/PixelArtImage"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { cn } from "@/lib/utils"
import type { ImageScalingMode } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { errorMessage } from "@/utils/appError"
import { runLimitedImageLoad } from "@/utils/concurrencyLimit"
import { getPixelArtDetectionPath } from "@/utils/imageRendering"
import { basenameOf } from "@/utils/path"
import {
  calculateWebtoonScrollMetrics,
  type WebtoonPageRect,
  type WebtoonScrollMetrics
} from "@/utils/webtoonProgress"

// 큐에 대기하다 화면 밖으로 나간 지연 로드의 취소 신호. 실패 UI로
// 보이면 안 되므로 Error가 아닌 심볼로 구분한다.
const STALE = Symbol("webtoon-stale-load")

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

export type WebtoonScrollTarget = {
  index: number
  nonce: number
} | null

function WebtoonLazyPage({
  path,
  index,
  getOrLoadImage,
  registerRef,
  fitWidth,
  imageScalingMode,
  autoDetectPixelArt,
  showPageBoundary,
  priority,
  isCurrent,
  onImageDoubleClick
}: {
  path: string
  index: number
  getOrLoadImage: GetOrLoadImage
  registerRef: (index: number, el: HTMLDivElement | null) => void
  fitWidth: boolean
  imageScalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  showPageBoundary: boolean
  priority: boolean
  isCurrent: boolean
  onImageDoubleClick?: (e: React.MouseEvent) => void
}) {
  const { t } = useTranslation()
  const wrapRef = useRef<HTMLDivElement>(null)
  const nearRef = useRef(false)
  const [info, setInfo] = useState<ImageInfo | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isNearViewport, setIsNearViewport] = useState(false)
  const [nonce, setNonce] = useState(0)
  const name = basenameOf(path)

  useEffect(() => {
    registerRef(index, wrapRef.current)
    return () => registerRef(index, null)
  }, [index, registerRef])

  useEffect(() => {
    const el = wrapRef.current
    if (!el) return
    let cancelled = false
    let loaded = false

    const load = async () => {
      if (loaded) return
      loaded = true
      try {
        // 연속 뷰의 동시 디코드 상한을 둔다. 대기 중 화면 밖으로 나간
        // 페이지는 실행 직전에 다시 확인해 백엔드 호출을 생략한다.
        const data = await runLimitedImageLoad(() => {
          if (cancelled || !nearRef.current) {
            return Promise.reject(STALE)
          }
          return getOrLoadImage(path)
        })
        if (!cancelled) {
          setInfo(data)
          setError(null)
        }
      } catch (e) {
        loaded = false
        if (!cancelled && e !== STALE) {
          setError(errorMessage(e))
        }
      }
    }

    if (typeof IntersectionObserver === "undefined") {
      nearRef.current = true
      setIsNearViewport(priority)
      void load()
      return () => {
        cancelled = true
      }
    }

    const io = new IntersectionObserver(
      (entries) => {
        const near = entries.some((entry) => entry.isIntersecting)
        nearRef.current = near
        setIsNearViewport(near)
        if (near) {
          void load()
        }
      },
      { rootMargin: "200% 0px" }
    )
    io.observe(el)
    return () => {
      cancelled = true
      io.disconnect()
    }
  }, [path, getOrLoadImage, nonce, priority])

  return (
    <div
      ref={wrapRef}
      data-webtoon-index={index}
      aria-current={isCurrent ? "page" : undefined}
      className={cn(
        "flex w-full shrink-0 justify-center",
        showPageBoundary && "border-t border-border"
      )}
    >
      {info && !error ? (
        <PixelArtImage
          filePath={info.file_path}
          detectionPath={getPixelArtDetectionPath(info)}
          fileSize={info.file_size}
          scalingMode={imageScalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          detectionEnabled={isNearViewport || priority}
          detectionPriority={priority}
          src={convertFileSrc(info.file_path)}
          alt={name}
          width={info.width ?? undefined}
          height={info.height ?? undefined}
          className={cn("h-auto max-w-full object-contain", fitWidth && "w-full")}
          draggable={false}
          loading={priority ? "eager" : "lazy"}
          fetchPriority={priority ? "high" : undefined}
          decoding="async"
          onError={() => {
            setInfo(null)
            setError(t("error.corrupt.title"))
          }}
          onDoubleClick={onImageDoubleClick}
        />
      ) : error ? (
        <div className="flex w-full max-w-2xl flex-col items-center gap-2 rounded-lg border border-destructive/20 bg-background p-6 text-center">
          <p className="text-sm text-muted-foreground">
            {name}: {error}
          </p>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setError(null)
              setNonce((n) => n + 1)
            }}
          >
            {t("viewer.error.retry")}
          </Button>
        </div>
      ) : (
        <div
          aria-hidden="true"
          className="min-h-64 w-full max-w-2xl animate-pulse rounded bg-muted/40 motion-reduce:animate-none"
        />
      )}
    </div>
  )
}

type WebtoonPageListProps = {
  images: readonly string[]
  currentIndex: number
  getOrLoadImage: GetOrLoadImage
  registerRef: (index: number, el: HTMLDivElement | null) => void
  imageGap: number
  showPageBoundaries: boolean
  fitWidth: boolean
  imageScalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  onImageDoubleClick?: (e: React.MouseEvent) => void
}

const WebtoonPageList = memo(function WebtoonPageList({
  images,
  currentIndex,
  getOrLoadImage,
  registerRef,
  imageGap,
  showPageBoundaries,
  fitWidth,
  imageScalingMode,
  autoDetectPixelArt,
  onImageDoubleClick
}: WebtoonPageListProps) {
  return (
    <div
      className="flex flex-col items-center gap-(--webtoon-image-gap) py-2"
      style={{ "--webtoon-image-gap": `${imageGap}px` } as React.CSSProperties}
    >
      {images.map((path, index) => (
        <WebtoonLazyPage
          key={path}
          path={path}
          index={index}
          getOrLoadImage={getOrLoadImage}
          registerRef={registerRef}
          fitWidth={fitWidth}
          imageScalingMode={imageScalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          showPageBoundary={showPageBoundaries && index > 0}
          priority={index === currentIndex}
          isCurrent={index === currentIndex}
          onImageDoubleClick={onImageDoubleClick}
        />
      ))}
    </div>
  )
})

function WebtoonThumbnailButton({
  label,
  onClick,
  className
}: {
  label: string
  onClick: (trigger: HTMLButtonElement) => void
  className?: string
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      className={cn("pointer-events-auto", className)}
      onClick={(event) => onClick(event.currentTarget)}
      title={label}
      aria-label={label}
    >
      <SquaresFour />
    </Button>
  )
}

export function WebtoonContinuousView({
  images,
  currentIndex,
  getOrLoadImage,
  onCenterChange,
  scrollTarget,
  imageGap,
  showPageBoundaries,
  fitWidth,
  imageScalingMode = "auto",
  autoDetectPixelArt = true,
  showProgress,
  thumbnailJump,
  onOpenThumbnailGrid,
  onImageDoubleClick
}: {
  images: string[]
  currentIndex: number
  getOrLoadImage: GetOrLoadImage
  onCenterChange: (index: number) => void
  scrollTarget: WebtoonScrollTarget
  imageGap: number
  showPageBoundaries: boolean
  fitWidth: boolean
  imageScalingMode?: ImageScalingMode
  autoDetectPixelArt?: boolean
  showProgress: boolean
  thumbnailJump: boolean
  onOpenThumbnailGrid?: (trigger?: HTMLButtonElement) => void
  onImageDoubleClick?: (e: React.MouseEvent) => void
}) {
  const { t } = useTranslation()
  const scrollRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef(new Map<number, HTMLDivElement>())
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const onCenterRef = useRef(onCenterChange)
  onCenterRef.current = onCenterChange
  const rafRef = useRef(0)
  const initialIndex = Math.max(0, Math.min(currentIndex, images.length - 1))
  const lastReportedIndexRef = useRef(initialIndex)
  const [scrollMetrics, setScrollMetrics] = useState<WebtoonScrollMetrics>({
    currentIndex: initialIndex,
    percent: 0
  })
  const previousImagesRef = useRef(images)

  useEffect(() => {
    const datasetChanged = previousImagesRef.current !== images
    previousImagesRef.current = images
    const nextIndex = Math.max(0, Math.min(currentIndex, images.length - 1))
    lastReportedIndexRef.current = nextIndex
    setScrollMetrics((previous) => {
      if (!datasetChanged && previous.currentIndex === nextIndex) return previous
      return {
        currentIndex: nextIndex,
        percent: datasetChanged ? 0 : previous.percent
      }
    })
  }, [currentIndex, images])

  const registerRef = useRef((index: number, el: HTMLDivElement | null) => {
    const previous = itemRefs.current.get(index)
    if (previous && previous !== el) resizeObserverRef.current?.unobserve(previous)
    if (el) {
      itemRefs.current.set(index, el)
      resizeObserverRef.current?.observe(el)
    } else {
      itemRefs.current.delete(index)
    }
  }).current

  // 초기 진입과 외부 점프(썸네일, 단축키) 시 해당 인덱스로 스크롤
  useEffect(() => {
    if (!scrollTarget) return
    const el = itemRefs.current.get(scrollTarget.index)
    el?.scrollIntoView({ block: "start", behavior: "auto" })
  }, [scrollTarget])

  // 이미지 목록이 바뀌어도 현재 페이지부터 읽도록 맞춘다.
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const el = itemRefs.current.get(lastReportedIndexRef.current)
      el?.scrollIntoView({ block: "start", behavior: "auto" })
    })
    return () => cancelAnimationFrame(frame)
  }, [images])

  useEffect(() => {
    const container = scrollRef.current
    if (!container) return

    const measure = () => {
      rafRef.current = 0
      const viewport = container.getBoundingClientRect()
      const pages: WebtoonPageRect[] = []
      for (const [index, element] of itemRefs.current) {
        const rect = element.getBoundingClientRect()
        pages.push({ index, top: rect.top, height: rect.height })
      }

      const metrics = calculateWebtoonScrollMetrics({
        pages,
        viewportTop: viewport.top,
        viewportHeight: viewport.height,
        scrollTop: container.scrollTop,
        scrollHeight: container.scrollHeight,
        fallbackIndex: lastReportedIndexRef.current
      })

      setScrollMetrics((previous) =>
        previous.currentIndex === metrics.currentIndex && previous.percent === metrics.percent
          ? previous
          : metrics
      )

      if (metrics.currentIndex !== lastReportedIndexRef.current) {
        lastReportedIndexRef.current = metrics.currentIndex
        onCenterRef.current(metrics.currentIndex)
      }
    }

    const scheduleMeasure = () => {
      if (rafRef.current !== 0) return
      rafRef.current = requestAnimationFrame(measure)
    }

    const observer =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(scheduleMeasure)
    resizeObserverRef.current = observer
    observer?.observe(container)
    for (const element of itemRefs.current.values()) observer?.observe(element)

    container.addEventListener("scroll", scheduleMeasure, { passive: true })
    container.addEventListener("load", scheduleMeasure, true)
    window.addEventListener("resize", scheduleMeasure)
    scheduleMeasure()

    return () => {
      container.removeEventListener("scroll", scheduleMeasure)
      container.removeEventListener("load", scheduleMeasure, true)
      window.removeEventListener("resize", scheduleMeasure)
      observer?.disconnect()
      resizeObserverRef.current = null
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current)
      rafRef.current = 0
    }
  }, [fitWidth, imageGap, images, showPageBoundaries])

  if (images.length === 0) return null

  const displayIndex = Math.max(0, Math.min(scrollMetrics.currentIndex, images.length - 1))
  const positionText = t("viewer.webtoon.position", {
    current: displayIndex + 1,
    total: images.length,
    percent: scrollMetrics.percent
  })
  const progressLabel = t("viewer.webtoon.progressLabel", {
    current: displayIndex + 1,
    total: images.length,
    percent: scrollMetrics.percent
  })

  return (
    <>
      <div
        ref={scrollRef}
        role="region"
        tabIndex={0}
        data-webtoon-scroll-region="true"
        className="absolute inset-0 overflow-x-hidden overflow-y-auto focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        aria-label={t("viewer.webtoon.scrollRegion")}
      >
        <WebtoonPageList
          images={images}
          currentIndex={displayIndex}
          getOrLoadImage={getOrLoadImage}
          registerRef={registerRef}
          imageGap={imageGap}
          showPageBoundaries={showPageBoundaries}
          fitWidth={fitWidth}
          imageScalingMode={imageScalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          onImageDoubleClick={onImageDoubleClick}
        />
      </div>

      {showProgress && (
        <div
          data-testid="webtoon-progress"
          className="pointer-events-none absolute top-1/2 right-3 flex -translate-y-1/2 items-center gap-2 rounded-lg border border-border bg-background p-1 shadow-lg"
        >
          <span aria-hidden="true" className="pl-1 text-sm font-medium tabular-nums">
            {positionText}
          </span>
          <Progress value={scrollMetrics.percent} aria-label={progressLabel} className="w-10" />
          {thumbnailJump && onOpenThumbnailGrid && (
            <WebtoonThumbnailButton
              label={t("viewer.webtoon.openThumbnails")}
              onClick={onOpenThumbnailGrid}
            />
          )}
        </div>
      )}
      {!showProgress && thumbnailJump && onOpenThumbnailGrid && (
        <div className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 rounded-lg border border-border bg-background p-1 shadow-lg">
          <WebtoonThumbnailButton
            label={t("viewer.webtoon.openThumbnails")}
            onClick={onOpenThumbnailGrid}
          />
        </div>
      )}
    </>
  )
}
