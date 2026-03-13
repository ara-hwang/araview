import { useState, useCallback, useEffect, useRef } from "react";
import { useHotkey } from "@tanstack/react-hotkeys";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
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
  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);

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
          extensions: ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif", "avif", "heic", "heif", "cbz"],
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

  const handleFitWidth = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return;
    const containerW = containerRef.current.offsetWidth;
    const imgW = imageRef.current.offsetWidth;
    if (imgW === 0) return;
    setZoom(containerW / imgW);
    setPosition({ x: 0, y: 0 });
  }, []);

  const handleFitHeight = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return;
    const containerH = containerRef.current.offsetHeight;
    const imgH = imageRef.current.offsetHeight;
    if (imgH === 0) return;
    setZoom(containerH / imgH);
    setPosition({ x: 0, y: 0 });
  }, []);

  const handleFitScreen = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return;
    const containerW = containerRef.current.offsetWidth;
    const containerH = containerRef.current.offsetHeight;
    const imgW = imageRef.current.offsetWidth;
    const imgH = imageRef.current.offsetHeight;
    if (imgW === 0 || imgH === 0) return;
    setZoom(Math.min(containerW / imgW, containerH / imgH));
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
    if (image) {
      setIsDragging(true);
      setDragStart({ x: e.clientX - position.x, y: e.clientY - position.y });
    }
  }, [image, position]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDragging) {
      const newX = e.clientX - dragStart.x;
      const newY = e.clientY - dragStart.y;

      if (containerRef.current && imageRef.current) {
        const containerW = containerRef.current.offsetWidth;
        const containerH = containerRef.current.offsetHeight;
        const imgW = imageRef.current.offsetWidth;
        const imgH = imageRef.current.offsetHeight;

        const scaledW = imgW * zoom;
        const scaledH = imgH * zoom;

        const maxX = Math.abs(scaledW - containerW) / 2;
        const maxY = Math.abs(scaledH - containerH) / 2;

        setPosition({
          x: Math.max(-maxX, Math.min(maxX, newX)),
          y: Math.max(-maxY, Math.min(maxY, newY)),
        });
      } else {
        setPosition({ x: newX, y: newY });
      }
    }
  }, [isDragging, dragStart, zoom]);

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

  const handleContextMenu = useCallback(async (e: React.MouseEvent) => {
    e.preventDefault();

    const navItemPromises = dirImages && dirImages.images.length > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          MenuItem.new({ text: "Previous Image", accelerator: "Left", action: () => navigateImage("prev") }),
          MenuItem.new({ text: "Next Image", accelerator: "Right", action: () => navigateImage("next") }),
        ]
      : [];

    const [
      openItem,
      ...rest
    ] = await Promise.all([
      MenuItem.new({ text: "Open File", accelerator: "CmdOrCtrl+O", action: () => handleOpenFile() }),
      ...navItemPromises,
      PredefinedMenuItem.new({ item: "Separator" }),
      MenuItem.new({ text: "Zoom In", accelerator: "=", action: () => handleZoomIn() }),
      MenuItem.new({ text: "Zoom Out", accelerator: "-", action: () => handleZoomOut() }),
      MenuItem.new({ text: "Actual Size", accelerator: "0", action: () => handleResetZoom() }),
      PredefinedMenuItem.new({ item: "Separator" }),
      MenuItem.new({ text: "Fit Width", accelerator: "1", action: () => handleFitWidth() }),
      MenuItem.new({ text: "Fit Height", accelerator: "2", action: () => handleFitHeight() }),
      MenuItem.new({ text: "Fit to Screen", accelerator: "3", action: () => handleFitScreen() }),
    ]);

    const menu = await Menu.new({ items: [openItem, ...rest] });
    await menu.popup();
  }, [handleOpenFile, handleZoomIn, handleZoomOut, handleResetZoom, handleFitWidth, handleFitHeight, handleFitScreen, dirImages, navigateImage]);

  // Listen for files opened via OS file association (double-click / "Open With")
  useEffect(() => {
    const unlistenPromise = listen<string>("open-file", (event) => {
      loadImage(event.payload);
    });

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [loadImage]);

  // Keyboard shortcuts
  useHotkey("ArrowLeft", () => navigateImage("prev"));
  useHotkey("ArrowRight", () => navigateImage("next"));
  useHotkey({ key: "+" }, () => handleZoomIn());
  useHotkey("=", () => handleZoomIn());
  useHotkey("-", () => handleZoomOut());
  useHotkey("0", () => handleResetZoom());
  useHotkey("1", () => handleFitWidth());
  useHotkey("2", () => handleFitHeight());
  useHotkey("3", () => handleFitScreen());
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
    containerRef,
    imageRef,
    handleOpenFile,
    navigateImage,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleFitWidth,
    handleFitHeight,
    handleFitScreen,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    handleDrop,
    handleDragOver,
    handleContextMenu,
  };
}
