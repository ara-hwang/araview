import { useRef, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight, PinIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { DirectoryImages } from "../types";
import { useSettingsStore } from "../store/settingsStore";
import { Slider } from "./ui/slider";
import { ButtonGroup } from "./ui/button-group";

type ImageNavBarProps = {
  dirImages: DirectoryImages;
  currentIndex: number;
  onNavigate: (direction: "prev" | "next") => void;
  onNavigateToIndex: (index: number) => void;
};

export function ImageNavBar({
  dirImages,
  currentIndex,
  onNavigate,
  onNavigateToIndex,
}: ImageNavBarProps) {
  const settings = useSettingsStore();

  const progressTrackRef = useRef<HTMLDivElement>(null);
  const isDraggingProgressRef = useRef(false);

  // 맨 앞·맨 뒤 이미지 여부와 설정에 따른 이전/다음 비활성화 상태 계산
  const isFirst = currentIndex === 0;
  const isLast = currentIndex === dirImages.images.length - 1;
  const isPrevDisabled = isFirst && !settings?.loopNavigation;
  const isNextDisabled = isLast && !settings?.loopNavigation;

  // 프로그레스 바에서 마우스 위치를 현재 이미지 인덱스로 변환
  const getIndexFromClientX = useCallback(
    (clientX: number): number | null => {
      if (!progressTrackRef.current || !dirImages.images.length) return null;
      const rect = progressTrackRef.current.getBoundingClientRect();
      const pct = (clientX - rect.left) / rect.width;
      const index = Math.min(
        dirImages.images.length - 1,
        Math.max(0, Math.floor(pct * dirImages.images.length)),
      );
      return index;
    },
    [dirImages.images.length],
  );

  // 계산된 인덱스로 바로 이동 (드래그/클릭 공통 처리)
  const handleProgressPointer = useCallback(
    (clientX: number) => {
      const index = getIndexFromClientX(clientX);
      if (index !== null) onNavigateToIndex(index);
    },
    [getIndexFromClientX, onNavigateToIndex],
  );

  // 문서 전체에서 마우스 이동/업 이벤트를 감지해 드래그 중 프로그레스 바 갱신
  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (isDraggingProgressRef.current) handleProgressPointer(e.clientX);
    };
    const onUp = () => {
      isDraggingProgressRef.current = false;
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
  }, [handleProgressPointer]);

  return (
    // 하단 중앙에 고정된 내비게이션 바 (이전/다음 버튼 + 진행률 표시)
    <div className="absolute bottom-6 flex items-center gap-4 opacity-50 hover:opacity-100 border border-border rounded-md p-2 w-xl">
      <ButtonGroup>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => onNavigate("prev")}
          title="Previous image"
          disabled={isPrevDisabled}
        >
          <ChevronLeft />
        </Button>
        <Button
          variant="ghost"
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
        value={[currentIndex]}
        min={0}
        max={dirImages.images.length - 1}
        step={1}
        onValueChange={(value) => onNavigateToIndex(value as number)}
      />

      <Button variant="ghost" size="icon" title="Pin Navigation Bar">
        <PinIcon />
      </Button>
    </div>
  );
}
