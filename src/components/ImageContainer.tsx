import type { RefObject } from "react";
import { cn } from "@/lib/utils";
import { Spinner } from "@/components/ui/spinner";
import { ImageNavBar } from "./ImageNavBar";
import type { ImageInfo } from "../types";
import { useAppStore } from "@/store/appStore";

type ImageContainerProps = {
  image: ImageInfo | null;
  isDragging: boolean;
  position: { x: number; y: number };
  containerRef: RefObject<HTMLDivElement>;
  imageRef: RefObject<HTMLImageElement>;
  onWheel: (e: React.WheelEvent) => void;
  onMouseDown: (e: React.MouseEvent) => void;
  onMouseMove: (e: React.MouseEvent) => void;
  onMouseUp: () => void;
  onNavigate?: (direction: "prev" | "next") => void;
  onNavigateToIndex?: (index: number) => void;
};

export function ImageContainer({
  image,
  isDragging,
  position,
  containerRef,
  imageRef,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  onNavigate,
  onNavigateToIndex,
}: ImageContainerProps) {
  const { dirImages, loading, error, zoom } = useAppStore();

  const showNavBar = !!(
    dirImages.images.length > 1 &&
    onNavigate &&
    onNavigateToIndex
  );

  const imageSrc = image
    ? `data:${image.mime_type};base64,${image.base64}`
    : null;

  return (
    <div
      ref={containerRef}
      className={cn(
        "flex-1 overflow-hidden flex items-center justify-center relative bg-[hsl(var(--background))]",
        isDragging && "[&]:cursor-grabbing",
      )}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
    >
      {imageSrc && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <img
            ref={imageRef}
            src={imageSrc}
            alt={image?.file_name}
            className="max-w-full max-h-full object-contain transition-transform duration-50 ease-out pointer-events-auto"
            style={{
              transform: `translate(${position.x}px, ${position.y}px) scale(${zoom})`,
              transformOrigin: "center center",
              cursor: isDragging ? "grabbing" : "grab",
            }}
            draggable={false}
          />
        </div>
      )}

      {loading && (
        <div className="absolute inset-0 z-10 flex items-center justify-center pointer-events-none">
          <Spinner
            className="size-8 text-[hsl(var(--muted-foreground))]"
            aria-label="Loading image"
          />
        </div>
      )}

      {error && (
        <div className="text-sm text-[hsl(var(--destructive))] px-6 py-4 bg-[hsl(var(--destructive))]/10 rounded-lg border border-[hsl(var(--destructive))]/20">
          {error}
        </div>
      )}

      {showNavBar && onNavigate && onNavigateToIndex && (
        <ImageNavBar
          onNavigate={onNavigate}
          onNavigateToIndex={onNavigateToIndex}
        />
      )}
    </div>
  );
}
