import { ArrowClockwise, Cloud, MagnifyingGlass, WarningCircle, X } from "@phosphor-icons/react"
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { AppIcon } from "@/components/AppIcon"
import { Button } from "@/components/ui/button"
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { useCurrentPageIndices } from "@/hooks/useCurrentPageIndices"
import { useThumbnailSrcs } from "@/hooks/useThumbnailSrcs"
import { cn } from "@/lib/utils"
import type { DirectoryImages, ImageInfo } from "@/types"
import { cloudOnlyPathSet } from "@/utils/fileAvailability"
import {
  computeGridLayout,
  computeGridWindow,
  gridOffsetForIndex,
  gridScrollTopToCenter,
  gridScrollTopToReveal
} from "@/utils/gridWindow"

type ThumbnailGridProps = {
  dirImages: DirectoryImages
  archivePath: string | null
  getOrLoadImage: (filePath: string) => Promise<ImageInfo>
  failedPaths: string[]
  onNavigateToIndex: (index: number) => void
  onClose: () => void
}

const CELL_MIN_WIDTH = 148
const LABEL_HEIGHT = 22
const GAP = 8
const PADDING = 16
const OVERSCAN_ROWS = 2
const THUMB_MAX_SIDE = 256
/** 스크롤 중 창을 재계산하는 간격. 연속 스크롤에서도 ~10fps로 창을 갱신한다. */
const SCROLL_SETTLE_MS = 100

const basename = (path: string) => path.split(/[\\/]/).pop() ?? path

export function ThumbnailGrid({
  dirImages,
  archivePath,
  getOrLoadImage,
  failedPaths,
  onNavigateToIndex,
  onClose
}: ThumbnailGridProps) {
  const { t } = useTranslation()
  const scrollRef = useRef<HTMLDivElement>(null)
  const timerRef = useRef(0)
  const [query, setQuery] = useState("")
  const [scrollTop, setScrollTop] = useState(0)
  const [viewport, setViewport] = useState({ width: 0, height: 0 })
  // 화면에 떠 있는 페이지들. 양쪽 보기는 쌍 두 장이 함께 떠 있어 둘 다 강조한다.
  const currentIndices = useCurrentPageIndices()

  const items = useMemo(
    () => dirImages.images.map((path, index) => ({ path, index })),
    [dirImages.images]
  )
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (needle === "") return items
    return items.filter((item) => basename(item.path).toLowerCase().includes(needle))
  }, [items, query])

  const failedSet = useMemo(() => new Set(failedPaths), [failedPaths])

  const layout = useMemo(() => {
    const { columns, cellWidth } = computeGridLayout({
      viewportWidth: viewport.width,
      itemCount: filtered.length,
      minCellWidth: CELL_MIN_WIDTH,
      cellHeight: 0,
      gap: GAP,
      padding: PADDING
    })
    const thumbSide = Math.max(0, Math.round(cellWidth))
    const cellHeight = thumbSide + LABEL_HEIGHT
    return {
      columns,
      cellWidth,
      thumbSide,
      cellHeight,
      rowStride: cellHeight + GAP
    }
  }, [viewport.width, filtered.length])

  const gridWindow = useMemo(
    () =>
      computeGridWindow({
        scrollTop,
        viewportHeight: viewport.height,
        itemCount: filtered.length,
        columns: layout.columns,
        cellHeight: layout.cellHeight,
        gap: GAP,
        padding: PADDING,
        overscanRows: OVERSCAN_ROWS
      }),
    [scrollTop, viewport.height, filtered.length, layout]
  )

  const initialScrollDone = useRef(false)
  // 처음에는 필터가 비어 있으므로 선택 위치는 현재 인덱스와 같다.
  const [selectedPos, setSelectedPos] = useState(() =>
    Math.max(0, Math.min(dirImages.current_index, dirImages.images.length - 1))
  )

  // 열리면 그리드에 포커스를 둬 방향키 탐색을 바로 쓸 수 있게 한다.
  useEffect(() => {
    scrollRef.current?.focus({ preventScroll: true })
  }, [])

  // 컨테이너 크기 추적 (열 수와 창 계산의 기준)
  useLayoutEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const update = () => {
      setViewport((prev) =>
        prev.width === el.clientWidth && prev.height === el.clientHeight
          ? prev
          : { width: el.clientWidth, height: el.clientHeight }
      )
    }
    update()
    if (typeof ResizeObserver === "undefined") return
    const observer = new ResizeObserver(update)
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  // 선택 셀을 항상 보이게 스크롤. 처음 열 때는 현재 이미지를 중앙에 둔다.
  useEffect(() => {
    const scroller = scrollRef.current
    if (!scroller || viewport.height === 0 || filtered.length === 0) return
    const top = initialScrollDone.current
      ? gridScrollTopToReveal({
          index: selectedPos,
          scrollTop: scroller.scrollTop,
          viewportHeight: viewport.height,
          columns: layout.columns,
          cellHeight: layout.cellHeight,
          gap: GAP,
          padding: PADDING
        })
      : gridScrollTopToCenter({
          index: selectedPos,
          viewportHeight: viewport.height,
          columns: layout.columns,
          cellHeight: layout.cellHeight,
          gap: GAP,
          padding: PADDING
        })
    initialScrollDone.current = true
    if (top === scroller.scrollTop) return
    scroller.scrollTop = top
    setScrollTop(top)
  }, [selectedPos, viewport.height, layout, filtered.length])

  useEffect(() => {
    return () => window.clearTimeout(timerRef.current)
  }, [])

  const handleScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el || timerRef.current !== 0) return
    timerRef.current = window.setTimeout(() => {
      timerRef.current = 0
      setScrollTop(scrollRef.current?.scrollTop ?? 0)
    }, SCROLL_SETTLE_MS)
  }, [])

  const selectPos = useCallback(
    (next: number) => {
      if (filtered.length === 0) return
      setSelectedPos(Math.max(0, Math.min(next, filtered.length - 1)))
    },
    [filtered.length]
  )

  const jump = useCallback(
    (originalIndex: number) => {
      onNavigateToIndex(originalIndex)
      onClose()
    },
    [onNavigateToIndex, onClose]
  )

  const handleQueryChange = useCallback(
    (value: string) => {
      setQuery(value)
      const needle = value.trim().toLowerCase()
      const nextList =
        needle === ""
          ? items
          : items.filter((item) => basename(item.path).toLowerCase().includes(needle))
      const pos = nextList.findIndex((item) => item.index === dirImages.current_index)
      setSelectedPos(pos >= 0 ? pos : 0)
    },
    [items, dirImages.current_index]
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const target = e.target
      const inField = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement
      if (inField && e.key !== "Escape") return

      const pageRows = Math.max(1, Math.floor(viewport.height / layout.rowStride))
      const move = (delta: number) => {
        e.preventDefault()
        e.stopPropagation()
        selectPos(selectedPos + delta)
      }

      switch (e.key) {
        case "ArrowLeft":
          move(-1)
          break
        case "ArrowRight":
          move(1)
          break
        case "ArrowUp":
          move(-layout.columns)
          break
        case "ArrowDown":
          move(layout.columns)
          break
        case "Home":
          e.preventDefault()
          e.stopPropagation()
          selectPos(0)
          break
        case "End":
          e.preventDefault()
          e.stopPropagation()
          selectPos(filtered.length - 1)
          break
        case "PageUp":
          move(-pageRows * layout.columns)
          break
        case "PageDown":
          move(pageRows * layout.columns)
          break
        case "Enter": {
          const item = filtered[selectedPos]
          if (item) jump(item.index)
          break
        }
        case "Escape":
        case "g":
        case "G":
          e.preventDefault()
          e.stopPropagation()
          onClose()
          break
        default:
          break
      }
    },
    [
      filtered,
      jump,
      layout.columns,
      layout.rowStride,
      onClose,
      selectPos,
      selectedPos,
      viewport.height
    ]
  )

  const visiblePaths = useMemo(
    () => filtered.slice(gridWindow.startIndex, gridWindow.endIndex).map((item) => item.path),
    [filtered, gridWindow.startIndex, gridWindow.endIndex]
  )
  const cloudOnlySet = useMemo(() => cloudOnlyPathSet(dirImages), [dirImages])
  const thumbnailOptions = useMemo(
    () => ({
      maxSide: THUMB_MAX_SIDE,
      archivePath,
      skipThumbnailPaths: cloudOnlySet
    }),
    [archivePath, cloudOnlySet]
  )
  const {
    urls,
    failed: thumbFailed,
    retry
  } = useThumbnailSrcs(visiblePaths, getOrLoadImage, thumbnailOptions)

  const locationName = useMemo(() => {
    if (archivePath) return basename(archivePath)
    const first = dirImages.images[0]
    if (!first) return ""
    const parts = first.split(/[\\/]/)
    return parts.length >= 2 ? parts[parts.length - 2] : first
  }, [archivePath, dirImages.images])

  const activeId = filtered[selectedPos] ? `grid-item-${filtered[selectedPos].index}` : undefined

  // scroll-fade는 스크롤 여지가 없을 때 위쪽 페이드가 남는 WebView2 동작이
  // 있어, 오버플로가 없으면 문서가 제공하는 scroll-fade-none으로 끈다.
  const canScroll = gridWindow.totalHeight > viewport.height + 1

  return (
    <div
      className="absolute inset-0 z-40 flex flex-col bg-background"
      // 필터 입력에서도 Esc/G가 동작하도록 루트에서 키를 받는다.
      onKeyDown={handleKeyDown}
      // 그리드 위에서는 뷰어 우클릭 메뉴를 열지 않는다.
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
      }}
    >
      <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border px-4 py-2">
        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold">{t("viewer.grid.title")}</p>
          <p className="truncate text-xs text-muted-foreground">{locationName}</p>
        </div>
        <span className="text-xs text-muted-foreground tabular-nums">
          {t("viewer.grid.count", {
            current: dirImages.current_index + 1,
            total: dirImages.images.length
          })}
        </span>
        <div className="relative">
          <MagnifyingGlass
            aria-hidden="true"
            className="absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
          />
          <input
            type="text"
            value={query}
            onChange={(e) => handleQueryChange(e.target.value)}
            placeholder={t("viewer.grid.filter")}
            aria-label={t("viewer.grid.filter")}
            className="h-8 w-48 rounded-md border border-border bg-background pr-2 pl-7 text-xs focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          />
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClose}
          title={t("viewer.grid.close")}
          aria-label={t("viewer.grid.close")}
        >
          <X />
        </Button>
      </header>

      <div
        ref={scrollRef}
        role="listbox"
        aria-label={t("viewer.grid.aria")}
        aria-activedescendant={activeId}
        tabIndex={0}
        onScroll={handleScroll}
        // shadcn scroll-fade: 스크롤 위치를 따라가는 가장자리 마스크 (JS 불필요)
        className={cn(
          "relative flex-1 scroll-fade overflow-y-auto outline-none",
          !canScroll && "scroll-fade-none"
        )}
      >
        {filtered.length === 0 ? (
          <Empty className="border-none">
            <EmptyHeader>
              {query.trim() !== "" ? (
                <EmptyMedia variant="icon">
                  <MagnifyingGlass />
                </EmptyMedia>
              ) : (
                <EmptyMedia>
                  <AppIcon className="size-12" />
                </EmptyMedia>
              )}
              <EmptyTitle>
                {query.trim() !== "" ? t("viewer.grid.noResults") : t("viewer.grid.empty")}
              </EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : (
          <div className="relative" style={{ height: gridWindow.totalHeight }}>
            {filtered.slice(gridWindow.startIndex, gridWindow.endIndex).map((item, offset) => {
              const position = gridWindow.startIndex + offset
              const isCurrent = currentIndices.has(item.index)
              const isSelected = position === selectedPos
              const src = urls.get(item.path)
              const isCloudOnly = cloudOnlySet.has(item.path)
              const isFailed =
                !isCloudOnly && (failedSet.has(item.path) || thumbFailed.has(item.path))
              const name = basename(item.path)
              const left = PADDING + (position % layout.columns) * (layout.cellWidth + GAP)
              const top = gridOffsetForIndex(
                position,
                layout.columns,
                layout.cellHeight,
                GAP,
                PADDING
              )

              return (
                <div
                  key={item.path}
                  role="option"
                  id={`grid-item-${item.index}`}
                  aria-selected={isSelected}
                  aria-current={isCurrent ? "true" : undefined}
                  aria-setsize={filtered.length}
                  aria-posinset={position + 1}
                  aria-label={t("viewer.nav.thumb", {
                    index: item.index + 1,
                    name
                  })}
                  title={name}
                  onClick={() => {
                    if (isFailed && !src) retry(item.path)
                    jump(item.index)
                  }}
                  className={cn(
                    "absolute cursor-pointer overflow-hidden rounded-md border bg-background transition-colors",
                    isCurrent ? "border-foreground" : "border-border hover:border-muted-foreground",
                    isSelected && "ring-2 ring-ring ring-offset-0"
                  )}
                  style={{
                    left,
                    top,
                    width: layout.cellWidth,
                    height: layout.cellHeight
                  }}
                >
                  <div
                    className="relative w-full overflow-hidden bg-muted/40"
                    style={{ height: layout.thumbSide }}
                  >
                    {src ? (
                      <img
                        src={src}
                        alt=""
                        loading="lazy"
                        decoding="async"
                        draggable={false}
                        className="h-full w-full object-contain"
                      />
                    ) : isCloudOnly ? (
                      <span className="flex h-full w-full flex-col items-center justify-center gap-1 px-1 text-center">
                        <Cloud aria-hidden="true" className="size-5 text-muted-foreground" />
                        <span className="text-[10px] leading-tight text-muted-foreground">
                          {t("viewer.grid.cloudOnly")}
                        </span>
                      </span>
                    ) : isFailed ? (
                      <span className="flex h-full w-full flex-col items-center justify-center gap-1">
                        <WarningCircle aria-hidden="true" className="size-5 text-destructive" />
                        <span className="text-[10px] text-muted-foreground">
                          {t("viewer.grid.failed")}
                        </span>
                      </span>
                    ) : (
                      <span
                        aria-hidden="true"
                        className="block h-full w-full animate-pulse motion-reduce:animate-none"
                      />
                    )}
                    <span className="absolute top-0.5 left-0.5 rounded bg-background/85 px-1 text-[10px] tabular-nums">
                      {item.index + 1}
                    </span>
                    {isFailed && src && (
                      <span className="absolute top-0.5 right-0.5 rounded-full bg-background p-0.5">
                        <WarningCircle aria-hidden="true" className="size-3.5 text-destructive" />
                      </span>
                    )}
                    {isFailed && !src && (
                      <span className="absolute right-0.5 bottom-0.5 rounded-full bg-background/90 p-0.5">
                        <ArrowClockwise aria-hidden="true" className="size-3" />
                      </span>
                    )}
                  </div>
                  <p className="truncate px-1.5 py-0.5 text-[11px]">{name}</p>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
