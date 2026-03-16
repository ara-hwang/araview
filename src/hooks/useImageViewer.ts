import { useState, useCallback, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ImageInfo, DirectoryImages } from "../types";
import type { Settings } from "@/types/settings";
import { SUPPORTED_IMAGE_EXTENSIONS } from "../constants/imageExtensions";
import { showImageViewerContextMenu } from "../utils/contextMenu";
import { useImageCache } from "./useImageCache";
import { useZoomPan } from "./useZoomPan";
import { useImageViewerHotkeys } from "./useImageViewerHotkeys";
import { useOpenFileListener } from "./useOpenFileListener";
import { updateSettings, useSettingsStore } from "@/store/settingsStore";
import { updateApp, useAppStore } from "@/store/appStore";

export function useImageViewer() {
  const app = useAppStore();
  const settings = useSettingsStore();

  const [image, setImage] = useState<ImageInfo | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance } =
    useImageCache();

  const zoomPan = useZoomPan(containerRef, imageRef, image);

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = image ? image.file_name : "Image Viewer";
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {});
  }, [image]);

  const loadImage = useCallback(
    async (filePath: string, options?: { refreshDirectory?: boolean }) => {
      const refreshDirectory = options?.refreshDirectory ?? true;

      void updateApp({ loading: true, error: null });
      try {
        const imgInfo = await getOrLoadImage(filePath);
        setImage(imgInfo);
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
        setImage(null);
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

  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!app.dirImages || app.dirImages.images.length === 0) return;

      const dirImages = app.dirImages;

      const clamped = Math.max(
        0,
        Math.min(index, dirImages.images.length - 1),
      );
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

  useEffect(() => {
    void updateApp({ zoom: zoomPan.zoom });
  }, [zoomPan.zoom]);

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

  const handleOpenSettings = useCallback(() => setIsSettingsOpen(true), []);
  const handleCloseSettings = useCallback(() => setIsSettingsOpen(false), []);
  const handleSettingsChange = useCallback((newSettings: Partial<Settings>) => {
    void updateSettings(newSettings);
  }, []);

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
    image,
    isDragging: zoomPan.isDragging,
    position: zoomPan.position,
    settings,
    isSettingsOpen,
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
    handleOpenSettings,
    handleCloseSettings,
    handleSettingsChange,
  };
}
