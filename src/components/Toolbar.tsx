import {
  FolderOpen,
  ChevronLeft,
  ChevronRight,
  ZoomIn,
  ZoomOut,
  ArrowLeftRight,
  ArrowUpDown,
  Maximize2,
  Settings,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { DirectoryImages, Settings as AppSettings } from "../types";

const noDragStyle = { WebkitAppRegion: "no-drag" } as React.CSSProperties;

type ToolbarNavProps = {
  dirImages: DirectoryImages | null;
  currentIndex: number;
  loopNavigation: boolean;
  onNavigate: (direction: "prev" | "next") => void;
};

function ToolbarNav({
  dirImages,
  currentIndex,
  loopNavigation,
  onNavigate,
}: ToolbarNavProps) {
  const showNav = !!(dirImages && dirImages.images.length > 1);
  const isFirst = !dirImages || currentIndex === 0;
  const isLast =
    !dirImages || currentIndex === (dirImages?.images.length ?? 1) - 1;
  const isPrevDisabled = isFirst && !loopNavigation;
  const isNextDisabled = isLast && !loopNavigation;
  return (
    <div
      className="flex flex-1 items-center justify-center gap-1"
      style={noDragStyle}
    >
      {showNav && dirImages && (
        <>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onNavigate("prev")}
            title="Previous image"
            className="text-[hsl(var(--muted-foreground))]"
            disabled={isPrevDisabled}
          >
            <ChevronLeft />
          </Button>
          <span className="text-sm text-[hsl(var(--muted-foreground))] min-w-[60px] text-center tabular-nums">
            {currentIndex + 1} / {dirImages.images.length}
          </span>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onNavigate("next")}
            title="Next image"
            className="text-[hsl(var(--muted-foreground))]"
            disabled={isNextDisabled}
          >
            <ChevronRight />
          </Button>
        </>
      )}
    </div>
  );
}

type ToolbarZoomProps = {
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitWidth: () => void;
  onFitHeight: () => void;
  onFitScreen: () => void;
  onOpenSettings: () => void;
};

function ToolbarZoom({
  zoom,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitWidth,
  onFitHeight,
  onFitScreen,
  onOpenSettings,
}: ToolbarZoomProps) {
  return (
    <div className="flex items-center gap-1" style={noDragStyle}>
      <Button
        variant="ghost"
        size="icon"
        onClick={onFitWidth}
        title="Fit to width (1)"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <ArrowLeftRight />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        onClick={onFitHeight}
        title="Fit to height (2)"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <ArrowUpDown />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        onClick={onFitScreen}
        title="Fit to screen (3)"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <Maximize2 />
      </Button>
      <div className="w-px h-5 bg-[hsl(var(--border))] mx-1" />
      <Button
        variant="ghost"
        size="icon"
        onClick={onZoomOut}
        title="Zoom out (-)"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <ZoomOut />
      </Button>
      <span
        className={cn(
          "text-sm text-[hsl(var(--muted-foreground))] min-w-[50px] text-center cursor-pointer px-1.5 py-1 rounded tabular-nums",
          "hover:bg-[hsl(var(--muted))] hover:text-[hsl(var(--foreground))] transition-colors",
        )}
        onClick={onResetZoom}
        title="Reset zoom (0)"
      >
        {Math.round(zoom * 100)}%
      </span>
      <Button
        variant="ghost"
        size="icon"
        onClick={onZoomIn}
        title="Zoom in (+)"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <ZoomIn />
      </Button>
      <div className="w-px h-5 bg-[hsl(var(--border))] mx-1" />
      <Button
        variant="ghost"
        size="icon"
        onClick={onOpenSettings}
        title="Settings"
        className="text-[hsl(var(--muted-foreground))]"
      >
        <Settings />
      </Button>
    </div>
  );
}

type ToolbarProps = {
  dirImages: DirectoryImages | null;
  currentIndex: number;
  zoom: number;
  settings: AppSettings;
  onOpenFile: () => void;
  onNavigate: (direction: "prev" | "next") => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitWidth: () => void;
  onFitHeight: () => void;
  onFitScreen: () => void;
  onOpenSettings: () => void;
};

export function Toolbar({
  dirImages,
  currentIndex,
  zoom,
  settings,
  onOpenFile,
  onNavigate,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitWidth,
  onFitHeight,
  onFitScreen,
  onOpenSettings,
}: ToolbarProps) {
  return (
    <div
      className="flex items-center justify-between h-11 px-3 bg-[hsl(var(--card))] border-b border-[hsl(var(--border))] flex-shrink-0"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="flex items-center gap-1" style={noDragStyle}>
        <Button
          variant="ghost"
          size="sm"
          onClick={onOpenFile}
          title="Open file (Ctrl+O)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <FolderOpen />
          <span>Open</span>
        </Button>
      </div>
      <ToolbarNav
        dirImages={dirImages}
        currentIndex={currentIndex}
        loopNavigation={settings.loopNavigation}
        onNavigate={onNavigate}
      />
      <ToolbarZoom
        zoom={zoom}
        onZoomIn={onZoomIn}
        onZoomOut={onZoomOut}
        onResetZoom={onResetZoom}
        onFitWidth={onFitWidth}
        onFitHeight={onFitHeight}
        onFitScreen={onFitScreen}
        onOpenSettings={onOpenSettings}
      />
    </div>
  );
}
