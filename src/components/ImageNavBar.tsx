import {
  CaretLeft,
  CaretRight,
  SquaresFour,
  WarningCircle,
  ArrowClockwise
} from "@phosphor-icons/react"
import { useEffect, useMemo, useRef } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { useThumbnailSrcs } from "@/hooks/useThumbnailSrcs"
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/appStore"
import type { ImageInfo } from "@/types"
import { parentFolderNameOf, basenameOf } from "@/utils/statusBar"

import { useSettingsStore } from "../store/settingsStore"
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
  /** UI 자동 숨김 시 투명화 (마우스 이동 시 복귀) */
  hidden?: boolean
}

export function ImageNavBar({
  onNavigate,
  onNavigateToIndex,
  getOrLoadImage,
  onToggleGrid,
  gridActive = false,
  hidden = false
}: ImageNavBarProps) {
  const { t } = useTranslation()
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const failedPaths = useAppStore((state) => state.failedPaths)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  const failedSet = useMemo(() => new Set(failedPaths), [failedPaths])

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
  const thumbnailOptions = useMemo(() => ({ archivePath }), [archivePath])
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
    const el = stripRef.current?.querySelector('[data-current="true"]')
    el?.scrollIntoView({ block: "nearest", inline: "nearest" })
  }, [dirImages.current_index])

  return (
    // 하단 중앙에 고정된 내비게이션 바 (이전/다음 버튼 + 진행률 표시)
    <div
      className={cn(
        "absolute bottom-12 flex w-xl max-w-[calc(100%-2rem)] flex-col gap-2 rounded-md border border-border bg-background p-2 shadow-lg transition-opacity duration-300",
        hidden && "pointer-events-none opacity-0"
      )}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      {/* 썸네일 스트립: scale-110이 잘리지 않게 여유를 두고, 스크롤바 없이 스크롤만 유지 */}
      {thumbnails.length > 1 && (
        <div
          ref={stripRef}
          className="flex [scrollbar-width:none] items-center [justify-content:safe_center] gap-1 overflow-x-auto overflow-y-hidden p-1 [&::-webkit-scrollbar]:hidden"
        >
          {thumbnails.map(({ index, path }) => {
            const src = urls.get(path)
            const name = thumbLabel(path)
            const failed = failedSet.has(path) || thumbFailed.has(path)
            const isCurrent = index === dirImages.current_index
            return (
              <button
                key={path}
                type="button"
                data-current={isCurrent ? "true" : undefined}
                aria-current={isCurrent ? "true" : undefined}
                onClick={() => {
                  if (failed && !src) retry(path)
                  onNavigateToIndex(index)
                }}
                className={cn(
                  "relative h-12 w-12 shrink-0 overflow-hidden rounded border-2 transition duration-150 ease-motion-out focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                  isCurrent
                    ? "scale-110 border-primary motion-reduce:scale-100"
                    : "border-transparent opacity-60 hover:opacity-100"
                )}
                title={name}
                aria-label={
                  failed
                    ? t("viewer.nav.thumbError", { index: index + 1, name })
                    : t("viewer.nav.thumb", { index: index + 1, name })
                }
              >
                {src ? (
                  <img
                    src={src}
                    alt={name}
                    loading="lazy"
                    className="h-full w-full object-cover"
                    draggable={false}
                  />
                ) : failed ? (
                  <span className="flex h-full w-full items-center justify-center bg-muted/40">
                    <WarningCircle aria-hidden="true" className="size-5 text-destructive" />
                    <span className="sr-only">
                      {t("viewer.nav.thumbError", {
                        index: index + 1,
                        name
                      })}
                    </span>
                  </span>
                ) : (
                  <span
                    aria-hidden="true"
                    className="h-full w-full animate-pulse bg-muted/40 motion-reduce:animate-none"
                  />
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
              </button>
            )
          })}
        </div>
      )}

      <div className="flex items-center gap-4">
        <ButtonGroup>
          <Button
            variant="outline"
            size="icon"
            onClick={() => onNavigate("prev")}
            title={t("viewer.nav.prev")}
            aria-label={t("viewer.nav.prev")}
            disabled={isPrevDisabled}
          >
            <CaretLeft />
          </Button>
          <Button
            variant="outline"
            size="icon"
            onClick={() => onNavigate("next")}
            title={t("viewer.nav.next")}
            aria-label={t("viewer.nav.next")}
            disabled={isNextDisabled}
          >
            <CaretRight />
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

        {/* 슬라이더를 클릭/드래그해서 원하는 위치로 점프 이동 */}
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
      </div>
    </div>
  )
}
