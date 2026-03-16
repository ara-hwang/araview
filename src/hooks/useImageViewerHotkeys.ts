import { useHotkey } from "@tanstack/react-hotkeys";

// 이미지 뷰어 전용 키보드 단축키 맵핑을 담당하는 파라미터 타입
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

// 이미지 내비게이션/줌/파일 열기 등에 대한 모든 단축키를 한 곳에서 등록하는 훅
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
  // 방향키 / 숫자 / 조합키 등으로 각각의 액션을 바인딩
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
