import { useState, useCallback } from "react";
import { useHotkey } from "@tanstack/react-hotkeys";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImageInfo, DirectoryImages } from "../types";

export function useImageViewer() {
  const [image, setImage] = useState<ImageInfo | null>(null);
  const [dirImages, setDirImages] = useState<DirectoryImages | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [isDragging, setIsDragging] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadImage = useCallback(async (filePath: string) => {
    setLoading(true);
    setError(null);
    try {
      const imgInfo = await invoke<ImageInfo>("load_image", { filePath });
      setImage(imgInfo);
      setZoom(1);
      setPosition({ x: 0, y: 0 });

      const dirInfo = await invoke<DirectoryImages>("get_directory_images", { filePath });
      setDirImages(dirInfo);
      setCurrentIndex(dirInfo.current_index);
    } catch (e) {
      setError(String(e));
      setImage(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif", "avif", "heic", "heif"],
        },
      ],
    });

    if (selected) {
      await loadImage(selected);
    }
  }, [loadImage]);

  const navigateImage = useCallback(async (direction: "prev" | "next") => {
    if (!dirImages || dirImages.images.length <= 1) return;

    let newIndex: number;
    if (direction === "prev") {
      newIndex = currentIndex > 0 ? currentIndex - 1 : dirImages.images.length - 1;
    } else {
      newIndex = currentIndex < dirImages.images.length - 1 ? currentIndex + 1 : 0;
    }

    setCurrentIndex(newIndex);
    await loadImage(dirImages.images[newIndex]);
  }, [dirImages, currentIndex, loadImage]);

  const handleZoomIn = useCallback(() => {
    setZoom((prev) => Math.min(prev * 1.25, 10));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom((prev) => Math.max(prev / 1.25, 0.1));
  }, []);

  const handleResetZoom = useCallback(() => {
    setZoom(1);
    setPosition({ x: 0, y: 0 });
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    if (e.deltaY < 0) {
      setZoom((prev) => Math.min(prev * 1.1, 10));
    } else {
      setZoom((prev) => Math.max(prev / 1.1, 0.1));
    }
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (zoom > 1) {
      setIsDragging(true);
      setDragStart({ x: e.clientX - position.x, y: e.clientY - position.y });
    }
  }, [zoom, position]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDragging) {
      setPosition({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      });
    }
  }, [isDragging, dragStart]);

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const files = e.dataTransfer.files;
    if (files.length > 0) {
      const file = files[0];
      const filePath = (file as any).path;
      if (filePath) {
        await loadImage(filePath);
      }
    }
  }, [loadImage]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  // Keyboard shortcuts
  useHotkey("ArrowLeft", () => navigateImage("prev"));
  useHotkey("ArrowRight", () => navigateImage("next"));
  useHotkey({ key: "+" }, () => handleZoomIn());
  useHotkey("=", () => handleZoomIn());
  useHotkey("-", () => handleZoomOut());
  useHotkey("0", () => handleResetZoom());
  useHotkey("Mod+O", () => handleOpenFile());

  return {
    image,
    dirImages,
    currentIndex,
    zoom,
    isDragging,
    position,
    loading,
    error,
    handleOpenFile,
    navigateImage,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    handleDrop,
    handleDragOver,
  };
}
