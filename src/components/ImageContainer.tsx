import type { RefObject } from "react";
import { cn } from "@/lib/utils";
import { Spinner } from "@/components/ui/spinner";
import { ImageNavBar } from "./ImageNavBar";
import type { ImageInfo } from "../types";
import { useAppStore } from "@/store/appStore";

type ImageContainerProps = {
  image: ImageInfo | null;
  loading: boolean;
  error: string | null;
  zoom: number;
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
  loading,
  error,
  zoom,
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
  const dirImages = useAppStore().dirImages;

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
            className="max-w-full max-h-full object-contain transition-transform duration-[50ms] ease-out pointer-events-auto"
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

      {!image && !loading && !error && (
        <div className="flex flex-col items-center gap-3 text-[hsl(var(--muted-foreground))] pointer-events-none">
          <svg
            width="64"
            height="64"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1"
            opacity="0.4"
          >
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            <circle cx="8.5" cy="8.5" r="1.5" />
            <polyline points="21 15 16 10 5 21" />
          </svg>
          <p className="text-base opacity-80">Drag &amp; drop an image here</p>
          <p className="text-sm opacity-60">or press Ctrl+O to open a file</p>
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
