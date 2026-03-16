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

export function useImageViewer() {
  const settings = useSettingsStore();

  const [image, setImage] = useState<ImageInfo | null>(null);
  const [dirImages, setDirImages] = useState<DirectoryImages | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
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

      setLoading(true);
      setError(null);
      try {
        const imgInfo = await getOrLoadImage(filePath);
        setImage(imgInfo);
        zoomPan.resetView();

        let resolvedDirInfo = dirImages;

        if (
          refreshDirectory ||
          !resolvedDirInfo ||
          !resolvedDirInfo.images.includes(filePath)
        ) {
          resolvedDirInfo = await invoke<DirectoryImages>(
            "get_directory_images",
            { filePath },
          );
          setDirImages(resolvedDirInfo);
        }

        if (resolvedDirInfo) {
          const resolvedIndex = resolvedDirInfo.images.indexOf(filePath);
          const nextIndex =
            resolvedIndex >= 0 ? resolvedIndex : resolvedDirInfo.current_index;
          setCurrentIndex(nextIndex);
          prefetchNearbyImages(
            resolvedDirInfo.images,
            nextIndex,
            settings.loopNavigation,
            getPrefetchDistance(),
          );
        }
      } catch (e) {
        setError(String(e));
        setImage(null);
      } finally {
        setLoading(false);
      }
    },
    [
      dirImages,
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
      if (!dirImages || dirImages.images.length <= 1) return;

      let newIndex: number;
      if (direction === "prev") {
        if (currentIndex === 0) {
          if (!settings.loopNavigation) return;
          newIndex = dirImages.images.length - 1;
        } else {
          newIndex = currentIndex - 1;
        }
      } else {
        if (currentIndex === dirImages.images.length - 1) {
          if (!settings.loopNavigation) return;
          newIndex = 0;
        } else {
          newIndex = currentIndex + 1;
        }
      }

      setCurrentIndex(newIndex);
      await loadImage(dirImages.images[newIndex], { refreshDirectory: false });
    },
    [dirImages, currentIndex, loadImage, settings.loopNavigation],
  );

  const navigateToIndex = useCallback(
    async (index: number) => {
      if (!dirImages || dirImages.images.length === 0) return;
      const clamped = Math.max(0, Math.min(index, dirImages.images.length - 1));
      setCurrentIndex(clamped);
      await loadImage(dirImages.images[clamped], { refreshDirectory: false });
    },
    [dirImages, loadImage],
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
      await showImageViewerContextMenu(dirImages, {
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
      dirImages,
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
    if (!dirImages?.images.length) return;
    prefetchNearbyImages(
      dirImages.images,
      currentIndex,
      settings.loopNavigation,
      getPrefetchDistance(),
    );
  }, [
    dirImages,
    currentIndex,
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
    dirImages,
    currentIndex,
    zoom: zoomPan.zoom,
    isDragging: zoomPan.isDragging,
    position: zoomPan.position,
    loading,
    error,
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
