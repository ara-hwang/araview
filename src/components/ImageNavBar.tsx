import { ChevronLeft, ChevronRight, PinIcon } from "lucide-react"
import { useMemo } from "react"
import { Button } from "@/components/ui/button"
import { useSettingsStore } from "../store/settingsStore"
import { Slider } from "./ui/slider"
import { ButtonGroup } from "./ui/button-group"
import { useAppStore } from "@/store/appStore"
import { cn } from "@/lib/utils"
import { usePaintSrcs } from "@/hooks/usePaintSrcs"
import type { ImageInfo } from "@/types"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

type ImageNavBarProps = {
  onNavigate: (direction: "prev" | "next") => void
  onNavigateToIndex: (index: number) => void
  getOrLoadImage: GetOrLoadImage
}

export function ImageNavBar({
  onNavigate,
  onNavigateToIndex,
  getOrLoadImage
}: ImageNavBarProps) {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

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
  const urls = usePaintSrcs(thumbPaths, getOrLoadImage)

  return (
    // 하단 중앙에 고정된 내비게이션 바 (이전/다음 버튼 + 진행률 표시)
    <div
      className="bg-background/75 border-border absolute bottom-12 flex w-xl flex-col gap-2 rounded-md border p-2 opacity-50 hover:opacity-100"
      onMouseDown={(e) => e.stopPropagation()}
    >
      {/* 썸네일 스트립 */}
      {thumbnails.length > 1 && (
        <div className="flex items-center justify-center gap-1">
          {thumbnails.map(({ index, path }) => {
            const src = urls.get(path)
            return (
              <button
                key={path}
                type="button"
                onClick={() => onNavigateToIndex(index)}
                className={cn(
                  "h-12 w-12 shrink-0 overflow-hidden rounded border-2 transition-all",
                  index === dirImages.current_index
                    ? "border-primary scale-110"
                    : "border-transparent opacity-60 hover:opacity-100"
                )}
                title={path.split(/[\\/]/).pop()}
              >
                {src ? (
                  <img
                    src={src}
                    alt=""
                    loading="lazy"
                    className="h-full w-full object-cover"
                    draggable={false}
                  />
                ) : null}
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
            title="Previous image"
            disabled={isPrevDisabled}
          >
            <ChevronLeft />
          </Button>
          <Button
            variant="outline"
            size="icon"
            onClick={() => onNavigate("next")}
            title="Next image"
            disabled={isNextDisabled}
          >
            <ChevronRight />
          </Button>
        </ButtonGroup>

        {/* 슬라이더를 클릭/드래그해서 원하는 위치로 점프 이동 */}
        <Slider
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

        <Button variant="ghost" size="icon" title="Pin Navigation Bar">
          <PinIcon />
        </Button>
      </div>
    </div>
  )
}
