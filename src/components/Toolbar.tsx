import {
  FolderOpen,
  ZoomIn,
  ZoomOut,
  ArrowLeftRight,
  ArrowUpDown,
  Maximize2,
  Settings,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ButtonGroup, ButtonGroupText } from "./ui/button-group";
import { Separator } from "./ui/separator";
import type { DirectoryImages, Settings as AppSettings } from "../types";

const noDragStyle = { WebkitAppRegion: "no-drag" } as React.CSSProperties;

type ToolbarRightProps = {
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitWidth: () => void;
  onFitHeight: () => void;
  onFitScreen: () => void;
  onOpenSettings: () => void;
};

function ToolbarRight({
  zoom,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitWidth,
  onFitHeight,
  onFitScreen,
  onOpenSettings,
}: ToolbarRightProps) {
  return (
    <div className="flex items-center gap-1" style={noDragStyle}>
      <ButtonGroup>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitWidth}
          title="Fit to width (1)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <ArrowLeftRight />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitHeight}
          title="Fit to height (2)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <ArrowUpDown />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitScreen}
          title="Fit to screen (3)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <Maximize2 />
        </Button>
      </ButtonGroup>

      <Separator orientation="vertical" />

      {/* 확대율 */}
      <ButtonGroup>
        <Button
          variant="outline"
          size="icon"
          onClick={onZoomOut}
          title="Zoom out (-)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <ZoomOut />
        </Button>
        <ButtonGroupText className="tabular-nums" onClick={onResetZoom}>
          {Math.round(zoom * 100)}%
        </ButtonGroupText>
        <Button
          variant="outline"
          size="icon"
          onClick={onZoomIn}
          title="Zoom in (+)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <ZoomIn />
        </Button>
      </ButtonGroup>

      <Separator orientation="vertical" />

      <Button
        variant="outline"
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

// 위쪽 툴바
export default function Toolbar({
  zoom,
  onOpenFile,
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
      className="flex justify-between p-2 bg-[hsl(var(--card))] border-b border-[hsl(var(--border))] "
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="flex items-center gap-1" style={noDragStyle}>
        <Button
          variant="outline"
          onClick={onOpenFile}
          title="Open file (Ctrl+O)"
          className="text-[hsl(var(--muted-foreground))]"
        >
          <FolderOpen />
          Open
        </Button>
      </div>

      <ToolbarRight
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
