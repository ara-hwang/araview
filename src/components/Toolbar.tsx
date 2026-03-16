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
import { useAppStore } from "@/store/appStore";

const noDragStyle = { WebkitAppRegion: "no-drag" } as React.CSSProperties;

type ToolbarRightProps = {
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitWidth: () => void;
  onFitHeight: () => void;
  onFitScreen: () => void;
  onOpenSettings: () => void;
};

function ToolbarRight({
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitWidth,
  onFitHeight,
  onFitScreen,
  onOpenSettings,
}: ToolbarRightProps) {
  const { zoom } = useAppStore();
  return (
    <div className="flex items-center gap-1" style={noDragStyle}>
      <ButtonGroup>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitWidth}
          title="Fit to width (1)"
        >
          <ArrowLeftRight />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitHeight}
          title="Fit to height (2)"
        >
          <ArrowUpDown />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={onFitScreen}
          title="Fit to screen (3)"
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
      >
        <Settings />
      </Button>
    </div>
  );
}

type ToolbarProps = {
  onOpenFile: () => void;
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
      className="flex justify-between p-2"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="flex items-center gap-1" style={noDragStyle}>
        <Button
          variant="outline"
          onClick={onOpenFile}
          title="Open file (Ctrl+O)"
        >
          <FolderOpen />
          Open
        </Button>
      </div>

      <ToolbarRight
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
