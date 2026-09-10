import {
  CaretLeft,
  CaretRight,
  WarningCircle,
  ArrowClockwise
} from "@phosphor-icons/react"
import { useEffect, useMemo, useRef } from "react"
import { Button } from "@/components/ui/button"
import { useSettingsStore } from "../store/settingsStore"
import { Slider } from "./ui/slider"
import { ButtonGroup } from "./ui/button-group"
import { useAppStore } from "@/store/appStore"
import { cn } from "@/lib/utils"
import { useThumbnailSrcs } from "@/hooks/useThumbnailSrcs"
import type { ImageInfo } from "@/types"
import { useTranslation } from "react-i18next"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

type ImageNavBarProps = {
  onNavigate: (direction: "prev" | "next") => void
  onNavigateToIndex: (index: number) => void
  getOrLoadImage: GetOrLoadImage
  /** UI 자동 숨김 시 투명화 (마우스 이동 시 복귀) */
  hidden?: boolean
}

export function ImageNavBar({
  onNavigate,
  onNavigateToIndex,
  getOrLoadImage,
  hidden = false
}: ImageNavBarProps) {
  const { t } = useTranslation()
  const dirImages = useAppStore((state) => state.dirImages)
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
  const {
    urls,
    failed: thumbFailed,
    retry
  } = useThumbnailSrcs(thumbPaths, getOrLoadImage)
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
        "bg-background border-border absolute bottom-12 flex w-xl max-w-[calc(100%-2rem)] flex-col gap-2 rounded-md border p-2 shadow-lg transition-opacity duration-300",
        hidden && "pointer-events-none opacity-0"
      )}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {/* 썸네일 스트립 */}
      {thumbnails.length > 1 && (
        <div
          ref={stripRef}
          className="flex items-center justify-center gap-1 overflow-x-auto"
        >
          {thumbnails.map(({ index, path }) => {
            const src = urls.get(path)
            const name = path.split(/[\\/]/).pop() ?? path
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
                  "relative h-12 w-12 shrink-0 overflow-hidden rounded border-2 transition-all focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:outline-none",
                  isCurrent
                    ? "border-primary scale-110"
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
                  <span className="bg-muted/40 flex h-full w-full items-center justify-center">
                    <WarningCircle
                      aria-hidden="true"
                      className="text-destructive size-5"
                    />
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
                    className="bg-muted/40 h-full w-full animate-pulse"
                  />
                )}
                {failed && src && (
                  <span
                    aria-hidden="true"
                    className="bg-background absolute top-0.5 right-0.5 rounded-full"
                  >
                    <WarningCircle className="text-destructive size-4" />
                  </span>
                )}
                {failed && !src && (
                  <span
                    aria-hidden="true"
                    title={t("viewer.nav.thumbRetry", { index: index + 1 })}
                    className="bg-background/90 absolute right-0.5 bottom-0.5 rounded-full p-0.5"
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

        {/* 슬라이더를 클릭/드래그해서 원하는 위치로 점프 이동 */}
        <Slider
          aria-label={t("viewer.nav.slider")}
          value={[dirImages.current_index]}
          min={0}
          max={dirImages.images.length - 1}
          step={1}
          onValueChange={(value) => {
            const nextIndex = Array.isArray(value)
              ? value[0]
              : (value as number)
            onNavigateToIndex(nextIndex)
          }}
        />
      </div>
    </div>
  )
}
