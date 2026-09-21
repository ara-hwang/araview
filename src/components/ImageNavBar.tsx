import {
  ArrowClockwise,
  CaretDown,
  CaretLeft,
  CaretRight,
  CaretUp,
  Cloud,
  DotsThree,
  SquaresFour,
  WarningCircle
} from "@phosphor-icons/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from "@/components/ui/dropdown-menu"
import { useThumbnailSrcs } from "@/hooks/useThumbnailSrcs"
import { useWheelNavigation } from "@/hooks/useWheelNavigation"
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/appStore"
import type { ImageInfo } from "@/types"
import { cloudOnlyPathSet } from "@/utils/fileAvailability"
import { basenameOf, parentFolderNameOf } from "@/utils/statusBar"
import { computeStripWindow, stripOffsetForIndex, stripScrollToReveal } from "@/utils/stripWindow"
import { resolveWheelAction } from "@/utils/wheelAction"

import {
  getSettings,
  updateSettings,
  useSettingsStore,
  type DockPosition,
  type DockThumbSize
} from "../store/settingsStore"
import { Toggle } from "./ui/toggle"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

type ImageNavBarProps = {
  onNavigate: (direction: "prev" | "next") => void
  onNavigateToIndex: (index: number) => void
  getOrLoadImage: GetOrLoadImage
  /** 썸네일 그리드 토글 */
  onToggleGrid?: () => void
  gridActive?: boolean
  /** 도크 접기 (전체 숨김) */
  onToggleDock?: () => void
  position: DockPosition
  thumbSize: DockThumbSize
  showName: boolean
  showIndex: boolean
  /**
   * 도크 숨김 (자동 숨김 포함). 언마운트하지 않고 display:none으로 감춰
   * 목록 스크롤 위치와 로드한 썸네일을 그대로 유지한다.
   */
  hidden?: boolean
}

const THUMB_SIDE: Record<DockThumbSize, number> = { s: 48, m: 72, l: 96 }
const STRIP_GAP = 4
const STRIP_PADDING = 4
/** 화면 밖으로 미리 그릴 항목 수 (스크롤 방향 여유) */
const STRIP_OVERSCAN = 4

export function ImageNavBar({
  onNavigate,
  onNavigateToIndex,
  getOrLoadImage,
  onToggleGrid,
  gridActive = false,
  onToggleDock,
  position,
  thumbSize,
  showName,
  showIndex,
  hidden = false
}: ImageNavBarProps) {
  const { t } = useTranslation()
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const failedPaths = useAppStore((state) => state.failedPaths)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  const failedSet = useMemo(() => new Set(failedPaths), [failedPaths])
  const isVertical = position === "left" || position === "right"
  const side = THUMB_SIDE[thumbSize]

  // 맨 앞·맨 뒤 이미지 여부와 설정에 따른 이전/다음 비활성화 상태 계산
  const isFirst = dirImages.current_index === 0
  const isLast = dirImages.current_index === dirImages.images.length - 1
  const isPrevDisabled = isFirst && !loopNavigation
  const isNextDisabled = isLast && !loopNavigation

  const stripRef = useRef<HTMLDivElement>(null)
  const [scrollOffset, setScrollOffset] = useState(0)
  const [viewportSize, setViewportSize] = useState(0)
  const scrollFrameRef = useRef(0)
  const needsRevealRef = useRef(true)

  // 컨테이너 크기 추적 (가시 창 계산 기준)
  useEffect(() => {
    const el = stripRef.current
    if (!el) return
    const update = () => {
      const next = isVertical ? el.clientHeight : el.clientWidth
      setViewportSize((prev) => (prev === next ? prev : next))
    }
    update()
    if (typeof ResizeObserver === "undefined") return
    const observer = new ResizeObserver(update)
    observer.observe(el)
    return () => observer.disconnect()
  }, [isVertical])

  const handleStripScroll = useCallback(() => {
    if (scrollFrameRef.current !== 0) return
    scrollFrameRef.current = window.requestAnimationFrame(() => {
      scrollFrameRef.current = 0
      const el = stripRef.current
      if (!el) return
      setScrollOffset(isVertical ? el.scrollTop : el.scrollLeft)
    })
  }, [isVertical])

  useEffect(() => {
    return () => window.cancelAnimationFrame(scrollFrameRef.current)
  }, [])

  // 인덱스나 썸네일 크기가 바뀌면 현재 항목이 보이도록 맞춘다.
  // 도크를 접었다 펼 때는 위치를 그대로 두기 위해 이 플래그를 세우지 않는다.
  useEffect(() => {
    needsRevealRef.current = true
  }, [dirImages.current_index, side])

  // 도크가 보이는 상태가 되면 스크롤 상태를 동기화한다.
  // (display:none 동안 브라우저가 스크롤 위치를 보존하므로 값만 다시 읽는다)
  useEffect(() => {
    const el = stripRef.current
    if (!el || viewportSize <= 0) return
    const current = isVertical ? el.scrollTop : el.scrollLeft
    if (needsRevealRef.current) {
      needsRevealRef.current = false
      const next = stripScrollToReveal({
        index: dirImages.current_index,
        scrollOffset: current,
        viewportSize,
        itemSize: side,
        gap: STRIP_GAP,
        padding: STRIP_PADDING
      })
      if (next !== current) {
        if (isVertical) el.scrollTo({ top: next })
        else el.scrollTo({ left: next })
        setScrollOffset(next)
        return
      }
    }
    setScrollOffset((prev) => (prev === current ? prev : current))
  }, [viewportSize, dirImages.current_index, isVertical, side])

  const stripWindow = useMemo(
    () =>
      computeStripWindow({
        scrollOffset,
        viewportSize,
        itemCount: dirImages.images.length,
        itemSize: side,
        gap: STRIP_GAP,
        padding: STRIP_PADDING,
        overscan: STRIP_OVERSCAN
      }),
    [scrollOffset, viewportSize, dirImages.images.length, side]
  )

  const visiblePaths = useMemo(
    () => dirImages.images.slice(stripWindow.startIndex, stripWindow.endIndex),
    [dirImages.images, stripWindow.startIndex, stripWindow.endIndex]
  )

  // 목록 전체를 대상으로 하되, 보이는 창을 먼저 채우고 나머지를 천천히 이어서 채운다.
  const cloudOnlySet = useMemo(() => cloudOnlyPathSet(dirImages), [dirImages])
  const thumbnailOptions = useMemo(
    () => ({
      archivePath,
      skipThumbnailPaths: cloudOnlySet,
      priorityPaths: visiblePaths
    }),
    [archivePath, cloudOnlySet, visiblePaths]
  )
  const {
    urls,
    failed: thumbFailed,
    retry
  } = useThumbnailSrcs(dirImages.images, getOrLoadImage, thumbnailOptions)

  const thumbLabel = (path: string) => {
    if (archivePath) return path.split(/[\\/]/).pop() ?? path
    const name = basenameOf(path)
    const parent = parentFolderNameOf(path)
    return parent ? `${parent}/${name}` : name
  }

  // 도크 위 휠은 설정된 휠 동작을 따른다. 동작이 없으면 스크롤로 넘긴다.
  const wheelNavigate = useWheelNavigation(
    { handleWheel: () => {} },
    (direction) => onNavigate(direction),
    { webtoonPassthrough: false }
  )
  const handleStripWheel = useCallback(
    (e: React.WheelEvent) => {
      const action = resolveWheelAction(e, getSettings().wheel)
      if (action) {
        wheelNavigate(e)
        return
      }
      const el = stripRef.current
      if (!el || e.deltaY === 0) return
      e.preventDefault()
      if (isVertical) el.scrollTop += e.deltaY
      else el.scrollLeft += e.deltaY
    },
    [isVertical, wheelNavigate]
  )

  const PrevIcon = isVertical ? CaretUp : CaretLeft
  const NextIcon = isVertical ? CaretDown : CaretRight
  const CollapseIcon =
    position === "top"
      ? CaretUp
      : position === "bottom"
        ? CaretDown
        : position === "left"
          ? CaretLeft
          : CaretRight
  // 메뉴가 읽기 영역 쪽으로 열리도록 도크 반대편을 side로 둔다.
  const menuSide =
    position === "top"
      ? "bottom"
      : position === "bottom"
        ? "top"
        : position === "left"
          ? "right"
          : "left"

  const dockMenu = (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon"
            title={t("viewer.nav.dockMenu")}
            aria-label={t("viewer.nav.dockMenu")}
          />
        }
      >
        <DotsThree aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent side={menuSide} align="end" className="min-w-48">
        <DropdownMenuGroup>
          <DropdownMenuLabel>{t("settings.dock.position")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={position}
            onValueChange={(value) => void updateSettings({ dockPosition: value as DockPosition })}
          >
            <DropdownMenuRadioItem value="top">{t("settings.dock.top")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="bottom">
              {t("settings.dock.bottom")}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="left">{t("settings.dock.left")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="right">{t("settings.dock.right")}</DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuLabel>{t("settings.dock.thumbSize")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={thumbSize}
            onValueChange={(value) =>
              void updateSettings({ dockThumbSize: value as DockThumbSize })
            }
          >
            <DropdownMenuRadioItem value="s">{t("settings.dock.thumbS")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="m">{t("settings.dock.thumbM")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="l">{t("settings.dock.thumbL")}</DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuCheckboxItem
            checked={showName}
            onCheckedChange={(checked) => void updateSettings({ dockShowName: checked === true })}
          >
            {t("settings.dock.showName")}
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={showIndex}
            onCheckedChange={(checked) => void updateSettings({ dockShowIndex: checked === true })}
          >
            {t("settings.dock.showIndex")}
          </DropdownMenuCheckboxItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )

  const renderThumbButton = (index: number) => {
    const path = dirImages.images[index]
    if (!path) return null
    const src = urls.get(path)
    const name = thumbLabel(path)
    const shortName = archivePath ? name : basenameOf(path)
    const isCloudOnly = cloudOnlySet.has(path)
    const failed = !isCloudOnly && (failedSet.has(path) || thumbFailed.has(path))
    const isCurrent = index === dirImages.current_index
    const labelHeight = showName ? 18 : 0
    const left = stripOffsetForIndex(index, side, STRIP_GAP, STRIP_PADDING)
    return (
      <button
        key={path}
        type="button"
        data-dock-current={isCurrent ? "true" : undefined}
        aria-current={isCurrent ? "true" : undefined}
        onClick={() => {
          if (failed && !src) retry(path)
          onNavigateToIndex(index)
        }}
        style={
          isVertical
            ? { top: left, width: side, height: side + labelHeight }
            : { left, width: side, height: side + labelHeight }
        }
        className={cn(
          "absolute overflow-hidden rounded border-2 transition duration-150 ease-motion-out focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
          isCurrent ? "border-primary" : "border-transparent opacity-60 hover:opacity-100"
        )}
        title={name}
        aria-label={
          isCloudOnly
            ? t("viewer.nav.cloudOnly", { index: index + 1, name })
            : failed
              ? t("viewer.nav.thumbError", { index: index + 1, name })
              : t("viewer.nav.thumb", { index: index + 1, name })
        }
      >
        <span
          className="relative block w-full overflow-hidden bg-muted/40"
          style={{ width: side, height: side }}
        >
          {src ? (
            <img
              src={src}
              alt={shortName}
              loading="lazy"
              className="h-full w-full object-cover"
              draggable={false}
            />
          ) : isCloudOnly ? (
            <span className="flex h-full w-full items-center justify-center">
              <Cloud aria-hidden="true" className="size-5 text-muted-foreground" />
              <span className="sr-only">
                {t("viewer.nav.cloudOnly", { index: index + 1, name })}
              </span>
            </span>
          ) : failed ? (
            <span className="flex h-full w-full items-center justify-center">
              <WarningCircle aria-hidden="true" className="size-5 text-destructive" />
              <span className="sr-only">
                {t("viewer.nav.thumbError", { index: index + 1, name })}
              </span>
            </span>
          ) : (
            <span
              aria-hidden="true"
              className="block h-full w-full animate-pulse motion-reduce:animate-none"
            />
          )}
          {showIndex && (
            <span className="absolute top-0.5 left-0.5 rounded bg-background/85 px-1 text-[10px] tabular-nums">
              {index + 1}
            </span>
          )}
          {failed && src && (
            <span
              aria-hidden="true"
              className="absolute top-0.5 right-0.5 rounded-full bg-background"
            >
              <WarningCircle className="size-4 text-destructive" />
            </span>
          )}
          {failed && !src && (
            <span
              aria-hidden="true"
              title={t("viewer.nav.thumbRetry", { index: index + 1 })}
              className="absolute right-0.5 bottom-0.5 rounded-full bg-background/90 p-0.5"
            >
              <ArrowClockwise aria-hidden="true" className="size-3.5" />
            </span>
          )}
        </span>
        {showName && (
          <span className="block truncate px-1 py-0.5 text-left text-[10px] leading-tight">
            {shortName}
          </span>
        )}
      </button>
    )
  }

  const hasStrip = dirImages.images.length > 1
  const stripSize = side + (showName ? 18 : 0)

  const strip = hasStrip ? (
    <div
      ref={stripRef}
      onWheel={handleStripWheel}
      className={cn(
        "relative min-h-0 min-w-0 flex-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
        isVertical ? "overflow-x-hidden overflow-y-auto" : "overflow-x-auto overflow-y-hidden"
      )}
      onScroll={handleStripScroll}
    >
      <div
        className="relative mx-auto"
        style={
          isVertical
            ? { height: stripWindow.totalSize, width: stripSize }
            : { width: stripWindow.totalSize, height: stripSize }
        }
      >
        {Array.from(
          { length: stripWindow.endIndex - stripWindow.startIndex },
          (_, offset) => stripWindow.startIndex + offset
        ).map(renderThumbButton)}
      </div>
    </div>
  ) : null

  return (
    <div
      className={cn(
        "min-h-0 min-w-0 items-center gap-1 border-border bg-background p-1",
        hidden ? "hidden" : cn("flex", isVertical ? "h-full flex-col" : "w-full flex-row"),
        position === "top" && "border-b",
        position === "bottom" && "border-t",
        position === "left" && "border-r",
        position === "right" && "border-l"
      )}
      style={isVertical ? { width: side + 24 } : undefined}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      <Button
        variant="outline"
        size="icon"
        onClick={() => onNavigate("prev")}
        title={t("viewer.nav.prev")}
        aria-label={t("viewer.nav.prev")}
        disabled={isPrevDisabled}
        className="shrink-0"
      >
        <PrevIcon />
      </Button>

      {strip}

      <Button
        variant="outline"
        size="icon"
        onClick={() => onNavigate("next")}
        title={t("viewer.nav.next")}
        aria-label={t("viewer.nav.next")}
        disabled={isNextDisabled}
        className="shrink-0"
      >
        <NextIcon />
      </Button>

      {onToggleGrid && (
        <Toggle
          variant="outline"
          pressed={gridActive}
          onPressedChange={onToggleGrid}
          title={t("viewer.nav.gridTitle")}
          aria-label={t("viewer.nav.grid")}
          className="shrink-0"
        >
          <SquaresFour />
        </Toggle>
      )}

      {dockMenu}

      {onToggleDock && (
        <Button
          variant="ghost"
          size="icon"
          onClick={onToggleDock}
          title={t("viewer.nav.dockHide")}
          aria-label={t("viewer.nav.dockHide")}
          className="shrink-0"
        >
          <CollapseIcon />
        </Button>
      )}
    </div>
  )
}
