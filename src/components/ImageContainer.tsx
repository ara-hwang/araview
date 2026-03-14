import type { RefObject } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";
import { Spinner } from "@/components/ui/spinner";
import { Button } from "@/components/ui/button";
import type { ImageInfo, BackgroundType, DirectoryImages, Settings as AppSettings } from "../types";

interface ImageContainerProps {
  image: ImageInfo | null;
  loading: boolean;
  error: string | null;
  zoom: number;
  isDragging: boolean;
  position: { x: number; y: number };
  background: BackgroundType;
  containerRef: RefObject<HTMLDivElement>;
  imageRef: RefObject<HTMLImageElement>;
  onWheel: (e: React.WheelEvent) => void;
  onMouseDown: (e: React.MouseEvent) => void;
  onMouseMove: (e: React.MouseEvent) => void;
  onMouseUp: () => void;
  dirImages?: DirectoryImages | null;
  currentIndex?: number;
  settings?: AppSettings;
  onNavigate?: (direction: "prev" | "next") => void;
}

export function ImageContainer({
  image,
  loading,
  error,
  zoom,
  isDragging,
  position,
  background,
  containerRef,
  imageRef,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
  dirImages,
  currentIndex = 0,
  settings,
  onNavigate,
}: ImageContainerProps) {
  const showNavButtons = !!(dirImages && dirImages.images.length > 1 && onNavigate);
  const isFirst = !dirImages || currentIndex === 0;
  const isLast = !dirImages || currentIndex === dirImages.images.length - 1;
  const isPrevDisabled = isFirst && !(settings?.loopNavigation);
  const isNextDisabled = isLast && !(settings?.loopNavigation);
  const imageSrc = image
    ? `data:${image.mime_type};base64,${image.base64}`
    : null;

  return (
    <div
      ref={containerRef}
      className={cn(
        "flex-1 overflow-hidden flex items-center justify-center relative bg-[hsl(var(--background))]",
        background === "checkered" && [
          "[background-image:linear-gradient(45deg,hsl(var(--muted))_25%,transparent_25%),linear-gradient(-45deg,hsl(var(--muted))_25%,transparent_25%),linear-gradient(45deg,transparent_75%,hsl(var(--muted))_75%),linear-gradient(-45deg,transparent_75%,hsl(var(--muted))_75%)]",
          "[background-size:20px_20px]",
          "[background-position:0_0,0_10px,10px_-10px,-10px_0]",
        ],
        isDragging && "[&]:cursor-grabbing",
      )}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
    >
      {imageSrc && (
        <img
          ref={imageRef}
          src={imageSrc}
          alt={image?.file_name}
          className="max-w-full max-h-full object-contain transition-transform duration-[50ms] ease-out"
          style={{
            transform: `translate(${position.x}px, ${position.y}px) scale(${zoom})`,
            cursor: isDragging ? "grabbing" : "grab",
          }}
          draggable={false}
        />
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

      {showNavButtons && (
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 flex items-center gap-1 rounded-lg bg-[hsl(var(--card))]/70 backdrop-blur-sm p-1 opacity-40 transition-opacity duration-200 hover:opacity-90 focus-within:opacity-90">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onNavigate?.("prev")}
            title="Previous image"
            className="text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]"
            disabled={isPrevDisabled}
          >
            <ChevronLeft />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onNavigate?.("next")}
            title="Next image"
            className="text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]"
            disabled={isNextDisabled}
          >
            <ChevronRight />
          </Button>
        </div>
      )}
    </div>
  );
}
