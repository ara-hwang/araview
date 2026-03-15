import { useHotkey } from "@tanstack/react-hotkeys";

type ImageViewerHotkeysParams = {
  onNavigatePrev: () => void;
  onNavigateNext: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitWidth: () => void;
  onFitHeight: () => void;
  onFitScreen: () => void;
  onOpenFile: () => void;
};

export function useImageViewerHotkeys({
  onNavigatePrev,
  onNavigateNext,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitWidth,
  onFitHeight,
  onFitScreen,
  onOpenFile,
}: ImageViewerHotkeysParams) {
  useHotkey("ArrowLeft", onNavigatePrev);
  useHotkey("ArrowRight", onNavigateNext);
  useHotkey({ key: "+" }, onZoomIn);
  useHotkey("=", onZoomIn);
  useHotkey("-", onZoomOut);
  useHotkey("0", onResetZoom);
  useHotkey("1", onFitWidth);
  useHotkey("2", onFitHeight);
  useHotkey("3", onFitScreen);
  useHotkey("Mod+O", onOpenFile);
}
