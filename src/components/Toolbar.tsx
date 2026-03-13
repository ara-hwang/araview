import { FolderOpen, ChevronLeft, ChevronRight, ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { DirectoryImages } from "../types";

interface ToolbarProps {
  dirImages: DirectoryImages | null;
  currentIndex: number;
  zoom: number;
  onOpenFile: () => void;
  onNavigate: (direction: "prev" | "next") => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
}

export function Toolbar({
  dirImages,
  currentIndex,
  zoom,
  onOpenFile,
  onNavigate,
  onZoomIn,
  onZoomOut,
  onResetZoom,
}: ToolbarProps) {
  return (
    <div
      className="flex items-center justify-between h-11 px-3 bg-[hsl(var(--card))] border-b border-[hsl(var(--border))] flex-shrink-0"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="flex items-center gap-1" style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}>
        <Button variant="ghost" size="sm" onClick={onOpenFile} title="Open file (Ctrl+O)" className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--muted))]">
          <FolderOpen />
          <span>Open</span>
        </Button>
      </div>

      <div className="flex flex-1 items-center justify-center gap-1" style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}>
        {dirImages && dirImages.images.length > 1 && (
          <>
            <Button variant="ghost" size="icon" onClick={() => onNavigate("prev")} title="Previous image" className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--muted))]">
              <ChevronLeft />
            </Button>
            <span className="text-sm text-[hsl(var(--muted-foreground))] min-w-[60px] text-center tabular-nums">
              {currentIndex + 1} / {dirImages.images.length}
            </span>
            <Button variant="ghost" size="icon" onClick={() => onNavigate("next")} title="Next image" className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--muted))]">
              <ChevronRight />
            </Button>
          </>
        )}
      </div>

      <div className="flex items-center gap-1" style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}>
        <Button variant="ghost" size="icon" onClick={onZoomOut} title="Zoom out (-)" className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--muted))]">
          <ZoomOut />
        </Button>
        <span
          className={cn(
            "text-sm text-[hsl(var(--muted-foreground))] min-w-[50px] text-center cursor-pointer px-1.5 py-1 rounded tabular-nums",
            "hover:bg-[hsl(var(--muted))] hover:text-[hsl(var(--foreground))] transition-colors"
          )}
          onClick={onResetZoom}
          title="Reset zoom (0)"
        >
          {Math.round(zoom * 100)}%
        </span>
        <Button variant="ghost" size="icon" onClick={onZoomIn} title="Zoom in (+)" className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--muted))]">
          <ZoomIn />
        </Button>
      </div>
    </div>
  );
}
