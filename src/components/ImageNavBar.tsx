import { useState, useRef, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { DirectoryImages } from "../types";
import { useSettingsStore } from "../hooks/useSettingsStore";

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
  const [isProgressHovered, setIsProgressHovered] = useState(false);
  const progressTrackRef = useRef<HTMLDivElement>(null);
  const isDraggingProgressRef = useRef(false);

  const isFirst = currentIndex === 0;
  const isLast = currentIndex === dirImages.images.length - 1;
  const isPrevDisabled = isFirst && !settings?.loopNavigation;
  const isNextDisabled = isLast && !settings?.loopNavigation;

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

  const handleProgressPointer = useCallback(
    (clientX: number) => {
      const index = getIndexFromClientX(clientX);
      if (index !== null) onNavigateToIndex(index);
    },
    [getIndexFromClientX, onNavigateToIndex],
  );

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
    <div className="absolute bottom-6 left-1/2 -translate-x-1/2 flex items-center gap-3 rounded-full bg-[hsl(var(--card))]/70 backdrop-blur-sm px-2 py-2 opacity-40 transition-opacity duration-200 hover:opacity-90 focus-within:opacity-90">
      <div className="flex items-center shrink-0">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => onNavigate("prev")}
          title="Previous image"
          className="size-9 rounded-full text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]"
          disabled={isPrevDisabled}
        >
          <ChevronLeft />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => onNavigate("next")}
          title="Next image"
          className="size-9 rounded-full text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]"
          disabled={isNextDisabled}
        >
          <ChevronRight />
        </Button>
      </div>
      <div
        ref={progressTrackRef}
        className="relative flex flex-col items-center justify-center cursor-pointer"
        onMouseEnter={() => setIsProgressHovered(true)}
        onMouseLeave={() => setIsProgressHovered(false)}
        onMouseDown={(e) => {
          e.stopPropagation();
          isDraggingProgressRef.current = true;
          handleProgressPointer(e.clientX);
        }}
      >
        {isProgressHovered && (
          <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1.5 text-xs text-[hsl(var(--foreground))] tabular-nums px-2 py-1 rounded-md bg-[hsl(var(--muted))]/90 whitespace-nowrap">
            {currentIndex + 1} / {dirImages.images.length}
          </span>
        )}
        <Progress
          value={
            dirImages.images.length > 0
              ? ((currentIndex + 1) / dirImages.images.length) * 100
              : 0
          }
          className="w-28"
        />
      </div>
    </div>
  );
}
