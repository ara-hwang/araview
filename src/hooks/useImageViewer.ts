import { useState, useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { useHotkey } from "@tanstack/react-hotkeys";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import type { ImageInfo, DirectoryImages, Settings } from "../types";

function getCacheLimit(mode: Settings["cacheMode"]) {
  switch (mode) {
    case "off":
      return 1;
    case "extended":
      return 64;
    case "nearby":
    default:
      return 24;
  }
}

function getPrefetchDistance(mode: Settings["cacheMode"]) {
  switch (mode) {
    case "off":
      return 0;
    case "extended":
      return 3;
    case "nearby":
    default:
      return 1;
  }
}

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
  const [settings, setSettings] = useState<Settings>({
    background: "checkered",
    loopNavigation: false,
    cacheMode: "nearby",
  });
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const prevZoomRef = useRef(1);
  const imageCacheRef = useRef<Map<string, ImageInfo>>(new Map());
  const inflightLoadsRef = useRef<Map<string, Promise<ImageInfo>>>(new Map());

  /** 뷰어에 꽉 차게 맞추는 줌(최소 줌). ref 미준비 시 1 반환해 공백 방지 */
  const getFitZoom = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return 1;
    const cw = containerRef.current.offsetWidth;
    const ch = containerRef.current.offsetHeight;
    const iw = imageRef.current.offsetWidth;
    const ih = imageRef.current.offsetHeight;
    if (iw <= 0 || ih <= 0) return 1;
    const fit = Math.min(cw / iw, ch / ih);
    return fit * (1 + 1e-6);
  }, []);

  const trimCacheToLimit = useCallback((limit: number) => {
    const cache = imageCacheRef.current;
    while (cache.size > limit) {
      const oldestKey = cache.keys().next().value;
      if (!oldestKey) {
        break;
      }
      cache.delete(oldestKey);
    }
  }, []);

  const cacheImage = useCallback(
    (filePath: string, imgInfo: ImageInfo) => {
      const cache = imageCacheRef.current;
      if (cache.has(filePath)) {
        cache.delete(filePath);
      }
      cache.set(filePath, imgInfo);
      trimCacheToLimit(getCacheLimit(settings.cacheMode));
    },
    [settings.cacheMode, trimCacheToLimit],
  );

  const getOrLoadImage = useCallback(
    async (filePath: string): Promise<ImageInfo> => {
      const cached = imageCacheRef.current.get(filePath);
      if (cached) {
        return cached;
      }

      const inflight = inflightLoadsRef.current.get(filePath);
      if (inflight) {
        return inflight;
      }

      const promise = invoke<ImageInfo>("load_image", { filePath })
        .then((imgInfo) => {
          cacheImage(filePath, imgInfo);
          return imgInfo;
        })
        .finally(() => {
          inflightLoadsRef.current.delete(filePath);
        });

      inflightLoadsRef.current.set(filePath, promise);
      return promise;
    },
    [cacheImage],
  );

  const prefetchNearbyImages = useCallback(
    (
      images: string[],
      index: number,
      loopNavigation: boolean,
      prefetchDistance: number,
    ) => {
      if (!images.length) return;
      if (prefetchDistance <= 0) return;

      const targets = new Set<number>();
      const imageCount = images.length;

      const normalizeIndex = (value: number) =>
        ((value % imageCount) + imageCount) % imageCount;

      for (let offset = 1; offset <= prefetchDistance; offset += 1) {
        const prevIndex = index - offset;
        if (prevIndex >= 0) {
          targets.add(prevIndex);
        } else if (loopNavigation && imageCount > 1) {
          targets.add(normalizeIndex(prevIndex));
        }

        const nextIndex = index + offset;
        if (nextIndex < imageCount) {
          targets.add(nextIndex);
        } else if (loopNavigation && imageCount > 1) {
          targets.add(normalizeIndex(nextIndex));
        }
      }

      for (const targetIndex of targets) {
        const targetPath = images[targetIndex];
        if (!targetPath) continue;
        if (imageCacheRef.current.has(targetPath)) continue;
        if (inflightLoadsRef.current.has(targetPath)) continue;

        void getOrLoadImage(targetPath).catch(() => {
          // Ignore preload failures; the image can still be loaded on demand.
        });
      }
    },
    [getOrLoadImage],
  );

  const loadImage = useCallback(
    async (filePath: string, options?: { refreshDirectory?: boolean }) => {
      const refreshDirectory = options?.refreshDirectory ?? true;

      setLoading(true);
      setError(null);
      try {
        const imgInfo = await getOrLoadImage(filePath);
        setImage(imgInfo);
        setZoom(1);
        setPosition({ x: 0, y: 0 });
        prevZoomRef.current = 1;

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
            getPrefetchDistance(settings.cacheMode),
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
      settings.cacheMode,
      settings.loopNavigation,
    ],
  );

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: [
            "png",
            "jpg",
            "jpeg",
            "gif",
            "bmp",
            "webp",
            "svg",
            "ico",
            "tiff",
            "tif",
            "avif",
            "heic",
            "heif",
          ],
        },
      ],
    });

    if (selected) {
      await loadImage(selected);
    }
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

  const handleZoomIn = useCallback(() => {
    setZoom((prev) => Math.min(prev * 1.25, 10));
  }, []);

  const handleZoomOut = useCallback(() => {
    const minZoom = getFitZoom();
    setZoom((prev) => Math.max(prev / 1.25, minZoom));
  }, [getFitZoom]);

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

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      if (e.ctrlKey) {
        const minZoom = getFitZoom();
        if (e.deltaY < 0) {
          setZoom((prev) => Math.min(prev * 1.1, 10));
        } else {
          setZoom((prev) => Math.max(prev / 1.1, minZoom));
        }
      } else {
        if (e.deltaY < 0) {
          navigateImage("prev");
        } else {
          navigateImage("next");
        }
      }
    },
    [navigateImage, getFitZoom],
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (image) {
        setIsDragging(true);
        setDragStart({ x: e.clientX - position.x, y: e.clientY - position.y });
      }
    },
    [image, position],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (isDragging && containerRef.current && imageRef.current) {
        const containerW = containerRef.current.offsetWidth;
        const containerH = containerRef.current.offsetHeight;
        const imgW = imageRef.current.offsetWidth;
        const imgH = imageRef.current.offsetHeight;
        const scaledW = imgW * zoom;
        const scaledH = imgH * zoom;

        // 뷰어에 다 들어오면 드래그로 이동하지 않고 항상 중앙 유지
        if (scaledW <= containerW && scaledH <= containerH) {
          setPosition({ x: 0, y: 0 });
          return;
        }

        const newX = e.clientX - dragStart.x;
        const newY = e.clientY - dragStart.y;
        const maxX = Math.abs(scaledW - containerW) / 2;
        const maxY = Math.abs(scaledH - containerH) / 2;

        setPosition({
          x: Math.max(-maxX, Math.min(maxX, newX)),
          y: Math.max(-maxY, Math.min(maxY, newY)),
        });
      } else if (isDragging) {
        setPosition({
          x: e.clientX - dragStart.x,
          y: e.clientY - dragStart.y,
        });
      }
    },
    [isDragging, dragStart, zoom],
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
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

      const navItemPromises =
        dirImages && dirImages.images.length > 1
          ? [
              PredefinedMenuItem.new({ item: "Separator" }),
              MenuItem.new({
                text: "Previous Image",
                accelerator: "Left",
                action: () => navigateImage("prev"),
              }),
              MenuItem.new({
                text: "Next Image",
                accelerator: "Right",
                action: () => navigateImage("next"),
              }),
            ]
          : [];

      const [openItem, ...rest] = await Promise.all([
        MenuItem.new({
          text: "Open File",
          accelerator: "CmdOrCtrl+O",
          action: () => handleOpenFile(),
        }),
        ...navItemPromises,
        PredefinedMenuItem.new({ item: "Separator" }),
        MenuItem.new({
          text: "Zoom In",
          accelerator: "=",
          action: () => handleZoomIn(),
        }),
        MenuItem.new({
          text: "Zoom Out",
          accelerator: "-",
          action: () => handleZoomOut(),
        }),
        MenuItem.new({
          text: "Actual Size",
          accelerator: "0",
          action: () => handleResetZoom(),
        }),
        PredefinedMenuItem.new({ item: "Separator" }),
        MenuItem.new({
          text: "Fit Width",
          accelerator: "1",
          action: () => handleFitWidth(),
        }),
        MenuItem.new({
          text: "Fit Height",
          accelerator: "2",
          action: () => handleFitHeight(),
        }),
        MenuItem.new({
          text: "Fit to Screen",
          accelerator: "3",
          action: () => handleFitScreen(),
        }),
      ]);

      const menu = await Menu.new({ items: [openItem, ...rest] });
      await menu.popup();
    },
    [
      handleOpenFile,
      handleZoomIn,
      handleZoomOut,
      handleResetZoom,
      handleFitWidth,
      handleFitHeight,
      handleFitScreen,
      dirImages,
      navigateImage,
    ],
  );

  // Listen for files opened via OS file association (double-click / "Open With")
  useEffect(() => {
    const unlistenPromise = listen<string>("open-file", (event) => {
      loadImage(event.payload);
    });

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [loadImage]);

  useEffect(() => {
    if (!dirImages || !dirImages.images.length) return;
    prefetchNearbyImages(
      dirImages.images,
      currentIndex,
      settings.loopNavigation,
      getPrefetchDistance(settings.cacheMode),
    );
  }, [
    dirImages,
    currentIndex,
    settings.cacheMode,
    settings.loopNavigation,
    prefetchNearbyImages,
  ]);

  useEffect(() => {
    trimCacheToLimit(getCacheLimit(settings.cacheMode));
  }, [settings.cacheMode, trimCacheToLimit]);

  // 뷰어에 이미지가 다 들어오면 항상 중앙(0,0)으로 유지
  useLayoutEffect(() => {
    if (!image || !containerRef.current || !imageRef.current) return;
    const containerW = containerRef.current.offsetWidth;
    const containerH = containerRef.current.offsetHeight;
    const imgW = imageRef.current.offsetWidth;
    const imgH = imageRef.current.offsetHeight;
    const scaledW = imgW * zoom;
    const scaledH = imgH * zoom;
    if (scaledW <= containerW && scaledH <= containerH) {
      setPosition({ x: 0, y: 0 });
    }
  }, [image, zoom]);

  // 확대/축소 시 화면 중앙 기준 보정 후, 가장자리 기준으로 클램프해 한쪽만 공백 나지 않게 함
  useLayoutEffect(() => {
    if (prevZoomRef.current === zoom) return;
    const ratio = zoom / prevZoomRef.current;
    setPosition((p) => {
      const newX = p.x * ratio;
      const newY = p.y * ratio;
      if (!containerRef.current || !imageRef.current) return { x: newX, y: newY };
      const cw = containerRef.current.offsetWidth;
      const ch = containerRef.current.offsetHeight;
      const iw = imageRef.current.offsetWidth;
      const ih = imageRef.current.offsetHeight;
      const scaledW = iw * zoom;
      const scaledH = ih * zoom;
      const maxX = scaledW >= cw ? (scaledW - cw) / 2 : 0;
      const maxY = scaledH >= ch ? (scaledH - ch) / 2 : 0;
      const clampX = scaledW >= cw ? Math.max(-maxX, Math.min(maxX, newX)) : 0;
      const clampY = scaledH >= ch ? Math.max(-maxY, Math.min(maxY, newY)) : 0;
      return { x: clampX, y: clampY };
    });
    prevZoomRef.current = zoom;
  }, [zoom]);

  const handleOpenSettings = useCallback(() => {
    setIsSettingsOpen(true);
  }, []);

  const handleCloseSettings = useCallback(() => {
    setIsSettingsOpen(false);
  }, []);

  const handleSettingsChange = useCallback((newSettings: Partial<Settings>) => {
    setSettings((prev) => ({ ...prev, ...newSettings }));
  }, []);

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
    settings,
    isSettingsOpen,
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
    handleOpenSettings,
    handleCloseSettings,
    handleSettingsChange,
  };
}
