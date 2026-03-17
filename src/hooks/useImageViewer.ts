import { useCallback, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { DirectoryImages } from "../types";
import { SUPPORTED_IMAGE_EXTENSIONS } from "../constants/imageExtensions";
import { showImageViewerContextMenu } from "../utils/contextMenu";
import { useImageCache } from "./useImageCache";
import { useZoomPan } from "./useZoomPan";
import { useImageViewerHotkeys } from "./useImageViewerHotkeys";
import { useOpenFileListener } from "./useOpenFileListener";
import { useSettingsStore } from "@/store/settingsStore";
import { updateApp, useAppStore } from "@/store/appStore";

// 이미지 로드, 디렉터리 내 내비게이션, 줌/팬, 드래그&드롭, 설정 변경까지
// 이미지 뷰어 화면에 필요한 모든 상태와 이벤트 핸들러를 제공하는 최상위 훅

export function useImageViewer() {
  const app = useAppStore();
  const settings = useSettingsStore();

  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance } =
    useImageCache();

  // 실제 줌/팬 로직은 별도 훅(useZoomPan)에 위임하고 여기서는 핸들러만 받아서 노출
  const zoomPan = useZoomPan(containerRef, imageRef);

  // 단일 파일을 로드하고, 필요시 디렉터리 이미지 목록까지 갱신하는 핵심 로더
  const loadImage = useCallback(
    async (filePath: string, options?: { refreshDirectory?: boolean }) => {
      const refreshDirectory = options?.refreshDirectory ?? true;

      void updateApp({ loading: true, error: null });
      try {
        const imgInfo = await getOrLoadImage(filePath);
        void updateApp({ imageInfo: imgInfo });
        zoomPan.resetView();

        let resolvedDirInfo = app.dirImages;

        if (
          refreshDirectory ||
          !resolvedDirInfo ||
          !resolvedDirInfo.images.includes(filePath)
        ) {
          resolvedDirInfo = await invoke<DirectoryImages>(
            "get_directory_images",
            { filePath },
          );

          updateApp({ dirImages: resolvedDirInfo });
        }

        if (resolvedDirInfo) {
          const resolvedIndex = resolvedDirInfo.images.indexOf(filePath);
          const nextIndex =
            resolvedIndex >= 0 ? resolvedIndex : resolvedDirInfo.current_index;
          prefetchNearbyImages(
            resolvedDirInfo.images,
            nextIndex,
            settings.loopNavigation,
            getPrefetchDistance(),
          );
        }
      } catch (e) {
        void updateApp({ error: String(e), loading: false });
        void updateApp({ imageInfo: null });
      } finally {
        void updateApp({ loading: false });
      }
    },
    [
      app.dirImages,
      getOrLoadImage,
      prefetchNearbyImages,
      getPrefetchDistance,
      settings.loopNavigation,
      zoomPan.resetView,
    ],
  );

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: [...SUPPORTED_IMAGE_EXTENSIONS],
        },
      ],
    });
    if (selected) await loadImage(selected);
  }, [loadImage]);

  // 이전/다음 이미지로 이동 (루프 내비게이션 옵션 고려)
  const navigateImage = useCallback(
    async (direction: "prev" | "next") => {
      if (!app.dirImages || app.dirImages.images.length <= 1) return;

      const dirImages = app.dirImages;

      let newIndex: number;
      if (direction === "prev") {
        if (dirImages.current_index === 0) {
          if (!settings.loopNavigation) return;
          newIndex = dirImages.images.length - 1;
        } else {
          newIndex = dirImages.current_index - 1;
        }
      } else {
        if (dirImages.current_index === dirImages.images.length - 1) {
          if (!settings.loopNavigation) return;
          newIndex = 0;
        } else {
          newIndex = dirImages.current_index + 1;
        }
      }

      await loadImage(dirImages.images[newIndex], {
        refreshDirectory: false,
      });

      void updateApp({
        dirImages: {
          ...dirImages,
          current_index: newIndex,
        },
      });
    },
    [app.dirImages, loadImage, settings.loopNavigation],
  );

  // 썸네일/시퀀스에서 특정 인덱스로 바로 점프
  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!app.dirImages || app.dirImages.images.length === 0) return;

      const dirImages = app.dirImages;

      const clamped = Math.max(0, Math.min(index, dirImages.images.length - 1));
      await loadImage(dirImages.images[clamped], {
        refreshDirectory: false,
      });

      void updateApp({
        dirImages: {
          ...dirImages,
          current_index: clamped,
        },
      });
    },
    [app.dirImages, loadImage],
  );

  // Ctrl + 휠은 줌, 그 외 휠은 이전/다음 이미지 이동
  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      if (e.ctrlKey) {
        zoomPan.handleWheel(e);
      } else {
        void navigateImage(e.deltaY < 0 ? "prev" : "next");
      }
    },
    [zoomPan.handleWheel, navigateImage],
  );

  // 파일 드롭으로 이미지 열기 (Tauri 파일 path 사용)
  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const files = e.dataTransfer.files;
      if (files.length > 0) {
        const file = files[0];
        const filePath = (file as { path?: string }).path;
        if (filePath) await loadImage(filePath);
      }
    },
    [loadImage],
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  // 우클릭 컨텍스트 메뉴에서 이미지/줌 관련 액션 제공
  const handleContextMenu = useCallback(
    async (e: React.MouseEvent) => {
      e.preventDefault();
      await showImageViewerContextMenu(app.dirImages, {
        onOpenFile: handleOpenFile,
        onNavigatePrev: () => navigateImage("prev"),
        onNavigateNext: () => navigateImage("next"),
        onZoomIn: zoomPan.handleZoomIn,
        onZoomOut: zoomPan.handleZoomOut,
        onResetZoom: zoomPan.handleResetZoom,
        onFitWidth: zoomPan.handleFitWidth,
        onFitHeight: zoomPan.handleFitHeight,
        onFitScreen: zoomPan.handleFitScreen,
      });
    },
    [
      app.dirImages,
      handleOpenFile,
      navigateImage,
      zoomPan.handleZoomIn,
      zoomPan.handleZoomOut,
      zoomPan.handleResetZoom,
      zoomPan.handleFitWidth,
      zoomPan.handleFitHeight,
      zoomPan.handleFitScreen,
    ],
  );

  useOpenFileListener(loadImage);

  // 디렉터리/설정이 바뀔 때마다 주변 이미지 프리패치
  useEffect(() => {
    if (!app.dirImages?.images.length) return;
    prefetchNearbyImages(
      app.dirImages.images,
      app.dirImages.current_index,
      settings.loopNavigation,
      getPrefetchDistance(),
    );
  }, [
    app.dirImages,
    settings.loopNavigation,
    prefetchNearbyImages,
    getPrefetchDistance,
  ]);

  useImageViewerHotkeys({
    onNavigatePrev: () => navigateImage("prev"),
    onNavigateNext: () => navigateImage("next"),
    onZoomIn: zoomPan.handleZoomIn,
    onZoomOut: zoomPan.handleZoomOut,
    onResetZoom: zoomPan.handleResetZoom,
    onFitWidth: zoomPan.handleFitWidth,
    onFitHeight: zoomPan.handleFitHeight,
    onFitScreen: zoomPan.handleFitScreen,
    onOpenFile: handleOpenFile,
  });

  return {
    containerRef,
    imageRef,
    handleOpenFile,
    navigateImage,
    navigateToIndex,
    handleZoomIn: zoomPan.handleZoomIn,
    handleZoomOut: zoomPan.handleZoomOut,
    handleResetZoom: zoomPan.handleResetZoom,
    handleFitWidth: zoomPan.handleFitWidth,
    handleFitHeight: zoomPan.handleFitHeight,
    handleFitScreen: zoomPan.handleFitScreen,
    handleWheel,
    handleMouseDown: zoomPan.handleMouseDown,
    handleMouseMove: zoomPan.handleMouseMove,
    handleMouseUp: zoomPan.handleMouseUp,
    handleDrop,
    handleDragOver,
    handleContextMenu,
  };
}
