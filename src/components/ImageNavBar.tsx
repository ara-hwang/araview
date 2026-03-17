import { ChevronLeft, ChevronRight, PinIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useSettingsStore } from "../store/settingsStore"
import { Slider } from "./ui/slider"
import { ButtonGroup } from "./ui/button-group"
import { useAppStore } from "@/store/appStore"

type ImageNavBarProps = {
  onNavigate: (direction: "prev" | "next") => void
  onNavigateToIndex: (index: number) => void
}

export function ImageNavBar({
  onNavigate,
  onNavigateToIndex
}: ImageNavBarProps) {
  const dirImages = useAppStore().dirImages
  const settings = useSettingsStore()

  // 맨 앞·맨 뒤 이미지 여부와 설정에 따른 이전/다음 비활성화 상태 계산
  const isFirst = dirImages.current_index === 0
  const isLast = dirImages.current_index === dirImages.images.length - 1
  const isPrevDisabled = isFirst && !settings?.loopNavigation
  const isNextDisabled = isLast && !settings?.loopNavigation

  return (
    // 하단 중앙에 고정된 내비게이션 바 (이전/다음 버튼 + 진행률 표시)
    <div
      className="bg-background/75 border-border absolute bottom-12 flex w-xl items-center gap-4 rounded-md border p-2 opacity-50 hover:opacity-100"
      onMouseDown={(e) => e.stopPropagation()}
    >
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
          const nextIndex = Array.isArray(value) ? value[0] : (value as number)
          onNavigateToIndex(nextIndex)
        }}
      />

      <Button variant="ghost" size="icon" title="Pin Navigation Bar">
        <PinIcon />
      </Button>
    </div>
  )
}
