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
import { useEffect, useMemo, useRef } from "react"
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
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/appStore"
import type { ImageInfo } from "@/types"
import { cloudOnlyPathSet } from "@/utils/fileAvailability"
import { basenameOf, parentFolderNameOf } from "@/utils/statusBar"

import {
  updateSettings,
  useSettingsStore,
  type DockPosition,
  type DockThumbSize
} from "../store/settingsStore"
import { ButtonGroup } from "./ui/button-group"
import { Slider } from "./ui/slider"
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
}

const THUMB_SIDE: Record<DockThumbSize, number> = { s: 48, m: 72, l: 96 }

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
  showIndex
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

  // 현재 인덱스 주변의 썸네일 창 (양쪽 4장씩, 총 최대 9장)
  const thumbnails = useMemo(() => {
    const THUMB_WINDOW = 4
    const total = dirImages.images.length
    if (total === 0) return [] as { index: number; path: string }[]
    const start = Math.max(0, dirImages.current_index - THUMB_WINDOW)
    const end = Math.min(total, dirImages.current_index + THUMB_WINDOW + 1)
    const out: { index: number; path: string }[] = []
    for (let i = start; i < end; i += 1) {
      out.push({ index: i, path: dirImages.images[i] })
    }
    return out
  }, [dirImages.images, dirImages.current_index])

  const thumbPaths = useMemo(() => thumbnails.map((t) => t.path), [thumbnails])
  const cloudOnlySet = useMemo(() => cloudOnlyPathSet(dirImages), [dirImages])
  const thumbnailOptions = useMemo(
    () => ({ archivePath, skipThumbnailPaths: cloudOnlySet }),
    [archivePath, cloudOnlySet]
  )
  const {
    urls,
    failed: thumbFailed,
    retry
  } = useThumbnailSrcs(thumbPaths, getOrLoadImage, thumbnailOptions)

  const thumbLabel = (path: string) => {
    if (archivePath) return path.split(/[\\/]/).pop() ?? path
    const name = basenameOf(path)
    const parent = parentFolderNameOf(path)
    return parent ? `${parent}/${name}` : name
  }
  const stripRef = useRef<HTMLDivElement>(null)

  // 선택된 썸네일이 윈도우 이동으로 벗어나지 않게 추적
  useEffect(() => {
    const el = stripRef.current?.querySelector('[data-dock-current="true"]')
    el?.scrollIntoView({ block: "nearest", inline: "nearest" })
  }, [dirImages.current_index])

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

  const renderThumbButton = ({ index, path }: { index: number; path: string }) => {
    const src = urls.get(path)
    const name = thumbLabel(path)
    const shortName = archivePath ? name : basenameOf(path)
    const isCloudOnly = cloudOnlySet.has(path)
    const failed = !isCloudOnly && (failedSet.has(path) || thumbFailed.has(path))
    const isCurrent = index === dirImages.current_index
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
        style={{ width: side }}
        className={cn(
          "relative shrink-0 overflow-hidden rounded border-2 transition duration-150 ease-motion-out focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
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

  const strip =
    thumbnails.length > 1 ? (
      <div
        ref={stripRef}
        className={cn(
          "flex [scrollbar-width:none] items-center gap-1 p-1 [&::-webkit-scrollbar]:hidden",
          isVertical
            ? "flex-1 flex-col [justify-content:safe_center] overflow-x-hidden overflow-y-auto"
            : "flex-1 flex-row [justify-content:safe_center] overflow-x-auto overflow-y-hidden"
        )}
      >
        {thumbnails.map(renderThumbButton)}
      </div>
    ) : null

  const controls = (
    <div className={cn("flex items-center gap-2 p-2", isVertical ? "flex-col" : "flex-row")}>
      <ButtonGroup orientation={isVertical ? "vertical" : "horizontal"}>
        <Button
          variant="outline"
          size="icon"
          onClick={() => onNavigate("prev")}
          title={t("viewer.nav.prev")}
          aria-label={t("viewer.nav.prev")}
          disabled={isPrevDisabled}
        >
          <PrevIcon />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={() => onNavigate("next")}
          title={t("viewer.nav.next")}
          aria-label={t("viewer.nav.next")}
          disabled={isNextDisabled}
        >
          <NextIcon />
        </Button>
      </ButtonGroup>

      {onToggleGrid && (
        <Toggle
          variant="outline"
          pressed={gridActive}
          onPressedChange={onToggleGrid}
          title={t("viewer.nav.gridTitle")}
          aria-label={t("viewer.nav.grid")}
        >
          <SquaresFour />
        </Toggle>
      )}

      {/* 수직 도크에서는 슬라이더를 생략하고 썸네일+버튼만 둔다 */}
      {!isVertical && (
        <Slider
          aria-label={t("viewer.nav.slider")}
          value={[dirImages.current_index]}
          min={0}
          max={dirImages.images.length - 1}
          step={1}
          onValueChange={(value) => {
            const nextIndex = Array.isArray(value) ? value[0] : (value as number)
            onNavigateToIndex(nextIndex)
          }}
        />
      )}
      <span className="shrink-0 text-xs text-muted-foreground tabular-nums">
        {t("viewer.nav.count", {
          current: dirImages.current_index + 1,
          total: dirImages.images.length
        })}
      </span>
      {dockMenu}
      {onToggleDock && (
        <Button
          variant="ghost"
          size="icon"
          onClick={onToggleDock}
          title={t("viewer.nav.dockHide")}
          aria-label={t("viewer.nav.dockHide")}
        >
          <CollapseIcon />
        </Button>
      )}
    </div>
  )

  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 border-border bg-background",
        isVertical ? "h-full flex-col" : "w-full flex-col",
        position === "top" && "border-b",
        position === "bottom" && "border-t",
        position === "left" && "border-r",
        position === "right" && "border-l"
      )}
      style={isVertical ? { width: side + 24 } : undefined}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      {isVertical ? (
        <>
          {controls}
          {strip && <div className="my-1 h-px w-full bg-border" aria-hidden="true" />}
          {strip}
        </>
      ) : (
        <>
          {strip}
          {strip && <div className="mx-1 h-px w-auto bg-border" aria-hidden="true" />}
          {controls}
        </>
      )}
    </div>
  )
}
