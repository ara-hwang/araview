import { BookOpen, SquaresFour } from "@phosphor-icons/react"
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
import { isArchiveFilePath } from "@/utils/archiveFile"
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

/** 요소가 관측 영역에 들어오고 나갈 때 알린다. 반환값은 관측 해제 함수다. */
type ViewportWatch = (element: Element, onChange: (inside: boolean) => void) => () => void

type ViewportWatcher = { watch: ViewportWatch; disconnect: () => void }

/**
 * 페이지들이 함께 쓰는 IntersectionObserver 하나를 만든다. 페이지마다 인스턴스를
 * 두면 수백 쪽 아카이브에서 옵저버도 수백 개가 된다.
 *
 * root는 스크롤 컨테이너여야 한다. root를 생략하면 페이지가 먼저 컨테이너의
 * 보이는 영역으로 잘려서 `rootMargin`이 효과가 없다. 컨테이너는 첫 관측 시점에야
 * DOM에 있으므로 옵저버를 그때 만든다.
 */
function createViewportWatcher(
  rootMargin: string,
  getRoot: () => Element | null
): ViewportWatcher | null {
  if (typeof IntersectionObserver === "undefined") return null
  const handlers = new Map<Element, (inside: boolean) => void>()
  let observer: IntersectionObserver | null = null
  return {
    watch: (element, onChange) => {
      observer ??= new IntersectionObserver(
        (entries) => {
          for (const entry of entries) handlers.get(entry.target)?.(entry.isIntersecting)
        },
        { root: getRoot(), rootMargin }
      )
      handlers.set(element, onChange)
      observer.observe(element)
      return () => {
        handlers.delete(element)
        observer?.unobserve(element)
      }
    },
    disconnect: () => {
      handlers.clear()
      observer?.disconnect()
      observer = null
    }
  }
}

type WebtoonWatchers = {
  /** 로드와 픽셀아트 판정을 시작하는 범위. */
  near: ViewportWatcher | null
  /** 이 범위를 벗어난 페이지는 이미지를 내려놓는다. `near`보다 넓어 경계에서 반복 로드하지 않는다. */
  keep: ViewportWatcher | null
}

type PageSize = { width: number; height: number }

export type WebtoonScrollTarget = {
  index: number
  nonce: number
} | null

function WebtoonLazyPage({
  path,
  index,
  getOrLoadImage,
  registerRef,
  watchers,
  onNearChange,
  fitWidth,
  imageScalingMode,
  autoDetectPixelArt,
  showPageBoundary,
  priority,
  isCurrent,
  onImageDoubleClick,
  onOpenArchive
}: {
  path: string
  index: number
  getOrLoadImage: GetOrLoadImage
  registerRef: (index: number, el: HTMLDivElement | null) => void
  watchers: WebtoonWatchers
  onNearChange: (index: number, near: boolean) => void
  fitWidth: boolean
  imageScalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  showPageBoundary: boolean
  priority: boolean
  isCurrent: boolean
  onImageDoubleClick?: (e: React.MouseEvent) => void
  onOpenArchive?: (path: string) => void
}) {
  const { t } = useTranslation()
  const wrapRef = useRef<HTMLDivElement>(null)
  const nearRef = useRef(false)
  const [info, setInfo] = useState<ImageInfo | null>(null)
  // 내려놓은 페이지가 자리를 그대로 차지하도록 마지막으로 본 크기를 기억한다.
  const [lastSize, setLastSize] = useState<PageSize | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isNearViewport, setIsNearViewport] = useState(false)
  const [nonce, setNonce] = useState(0)
  const name = basenameOf(path)
  // 폴더 안 아카이브(CBZ/ZIP)는 이미지로 그릴 수 없어 만화 열기 안내 카드로 보여준다.
  const isArchive = isArchiveFilePath(path)

  useEffect(() => {
    registerRef(index, wrapRef.current)
    return () => registerRef(index, null)
  }, [index, registerRef])

  useEffect(() => {
    const el = wrapRef.current
    if (!el || isArchive) return
    let cancelled = false
    let loaded = false
    // 유지 범위 안인지. 옵저버의 첫 보고 전에는 안에 있다고 본다.
    let kept = true

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
        if (cancelled) return
        if (!kept) {
          // 로드 중에 멀리 지나갔다. 그리지 않고 다음 접근 때 다시 로드한다.
          loaded = false
          return
        }
        setInfo(data)
        if (data.width && data.height) {
          setLastSize({ width: data.width, height: data.height })
        }
        setError(null)
      } catch (e) {
        loaded = false
        if (!cancelled && e !== STALE) {
          setError(errorMessage(e))
        }
      }
    }

    if (!watchers.near) {
      nearRef.current = true
      setIsNearViewport(priority)
      void load()
      return () => {
        cancelled = true
      }
    }

    const unwatchNear = watchers.near.watch(el, (near) => {
      nearRef.current = near
      setIsNearViewport(near)
      onNearChange(index, near)
      if (near) {
        // 로드 범위는 유지 범위 안이다. 두 옵저버의 보고 순서가 엇갈려도
        // 방금 받은 결과를 버리지 않게 여기서 함께 맞춘다.
        kept = true
        void load()
      }
    })
    // 멀리 지나간 페이지는 이미지를 내려놓는다. 풀해상 <img>가 본 만큼 쌓여
    // WebView 메모리를 차지하지 않게 하고, 자리는 `lastSize`로 유지한다.
    const unwatchKeep = watchers.keep?.watch(el, (inside) => {
      if (inside || nearRef.current) return
      kept = false
      loaded = false
      setInfo(null)
    })
    return () => {
      cancelled = true
      unwatchNear()
      unwatchKeep?.()
      onNearChange(index, false)
    }
  }, [path, index, getOrLoadImage, nonce, priority, isArchive, watchers, onNearChange])

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
      {isArchive ? (
        <div className="flex w-full max-w-2xl flex-col items-center gap-3 rounded-lg border border-border bg-background p-6 text-center">
          <p className="text-sm text-muted-foreground">
            {t("viewer.webtoon.archiveHint", { name })}
          </p>
          {onOpenArchive && (
            <Button
              size="sm"
              onClick={() => onOpenArchive(path)}
              title={t("viewer.archivePreview.openTitle")}
            >
              <BookOpen data-icon="inline-start" />
              {t("viewer.archivePreview.open")}
            </Button>
          )}
        </div>
      ) : info && !error ? (
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
            setLastSize(null)
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
      ) : lastSize ? (
        // 내려놓은 페이지: <img>와 같은 상자(너비, 종횡비)라 스크롤 위치가 변하지 않는다.
        <div
          aria-hidden="true"
          className={cn(
            "aspect-(--webtoon-page-ratio) max-w-full rounded bg-muted/40",
            fitWidth ? "w-full" : "w-(--webtoon-page-width)"
          )}
          style={
            {
              "--webtoon-page-width": `${lastSize.width}px`,
              "--webtoon-page-ratio": `${lastSize.width} / ${lastSize.height}`
            } as React.CSSProperties
          }
        />
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
  watchers: WebtoonWatchers
  onNearChange: (index: number, near: boolean) => void
  imageGap: number
  showPageBoundaries: boolean
  fitWidth: boolean
  imageScalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  onImageDoubleClick?: (e: React.MouseEvent) => void
  onOpenArchive?: (path: string) => void
}

const WebtoonPageList = memo(function WebtoonPageList({
  images,
  currentIndex,
  getOrLoadImage,
  registerRef,
  watchers,
  onNearChange,
  imageGap,
  showPageBoundaries,
  fitWidth,
  imageScalingMode,
  autoDetectPixelArt,
  onImageDoubleClick,
  onOpenArchive
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
          watchers={watchers}
          onNearChange={onNearChange}
          fitWidth={fitWidth}
          imageScalingMode={imageScalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          showPageBoundary={showPageBoundaries && index > 0}
          priority={index === currentIndex}
          isCurrent={index === currentIndex}
          onImageDoubleClick={onImageDoubleClick}
          onOpenArchive={onOpenArchive}
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
  onImageDoubleClick,
  onOpenArchive
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
  onOpenArchive?: (path: string) => void
}) {
  const { t } = useTranslation()
  const scrollRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef(new Map<number, HTMLDivElement>())
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const onCenterRef = useRef(onCenterChange)
  onCenterRef.current = onCenterChange
  const rafRef = useRef(0)
  const scheduleMeasureRef = useRef<(() => void) | null>(null)
  // 뷰포트 근처 페이지 인덱스. 스크롤 측정이 전체 페이지를 훑지 않게 한다.
  const nearIndexesRef = useRef(new Set<number>())
  const [watchers] = useState<WebtoonWatchers>(() => ({
    near: createViewportWatcher("200% 0px", () => scrollRef.current),
    keep: createViewportWatcher("400% 0px", () => scrollRef.current)
  }))
  useEffect(
    () => () => {
      watchers.near?.disconnect()
      watchers.keep?.disconnect()
    },
    [watchers]
  )
  const onNearChange = useRef((index: number, near: boolean) => {
    if (near) nearIndexesRef.current.add(index)
    else nearIndexesRef.current.delete(index)
    scheduleMeasureRef.current?.()
  }).current
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
      const viewportBottom = viewport.top + viewport.height
      // 근처 페이지만 잰다. 수백 쪽 목록을 프레임마다 전부 재지 않는다.
      let pages: WebtoonPageRect[] = []
      let anyVisible = false
      for (const index of nearIndexesRef.current) {
        const element = itemRefs.current.get(index)
        if (!element) continue
        const rect = element.getBoundingClientRect()
        pages.push({ index, top: rect.top, height: rect.height })
        if (rect.top + rect.height >= viewport.top && rect.top <= viewportBottom) {
          anyVisible = true
        }
      }
      // 큰 점프 직후처럼 옵저버 보고가 아직 오지 않아 보이는 페이지가 근처
      // 집합에 없으면, 그 프레임만 전체를 재서 현재 페이지를 잘못 고르지 않는다.
      if (!anyVisible) {
        pages = []
        for (const [index, element] of itemRefs.current) {
          const rect = element.getBoundingClientRect()
          pages.push({ index, top: rect.top, height: rect.height })
        }
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
    scheduleMeasureRef.current = scheduleMeasure

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
      scheduleMeasureRef.current = null
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
          watchers={watchers}
          onNearChange={onNearChange}
          imageGap={imageGap}
          showPageBoundaries={showPageBoundaries}
          fitWidth={fitWidth}
          imageScalingMode={imageScalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          onImageDoubleClick={onImageDoubleClick}
          onOpenArchive={onOpenArchive}
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
