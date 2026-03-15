import {
  useState,
  useCallback,
  useLayoutEffect,
  useRef,
  type RefObject,
} from "react";
import type { ImageInfo } from "../types";
import {
  getPositionBounds,
  clampPosition,
  isFullyContained,
} from "../utils/zoomPanUtils";

export function useZoomPan(
  containerRef: RefObject<HTMLDivElement | null>,
  imageRef: RefObject<HTMLImageElement | null>,
  image: ImageInfo | null,
) {
  const [zoom, setZoom] = useState(1);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const prevZoomRef = useRef(1);

  const getFitZoom = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return 1;
    const cw = containerRef.current.offsetWidth;
    const ch = containerRef.current.offsetHeight;
    const iw = imageRef.current.offsetWidth;
    const ih = imageRef.current.offsetHeight;
    if (iw <= 0 || ih <= 0) return 1;
    const fit = Math.min(cw / iw, ch / ih);
    return fit * (1 + 1e-6);
  }, [containerRef, imageRef]);

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
  }, [containerRef, imageRef]);

  const handleFitHeight = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return;
    const containerH = containerRef.current.offsetHeight;
    const imgH = imageRef.current.offsetHeight;
    if (imgH === 0) return;
    setZoom(containerH / imgH);
    setPosition({ x: 0, y: 0 });
  }, [containerRef, imageRef]);

  const handleFitScreen = useCallback(() => {
    if (!containerRef.current || !imageRef.current) return;
    const cw = containerRef.current.offsetWidth;
    const ch = containerRef.current.offsetHeight;
    const iw = imageRef.current.offsetWidth;
    const ih = imageRef.current.offsetHeight;
    if (iw === 0 || ih === 0) return;
    setZoom(Math.min(cw / iw, ch / ih));
    setPosition({ x: 0, y: 0 });
  }, [containerRef, imageRef]);

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
      if (!isDragging) return;
      const newX = e.clientX - dragStart.x;
      const newY = e.clientY - dragStart.y;
      if (containerRef.current && imageRef.current) {
        const cw = containerRef.current.offsetWidth;
        const ch = containerRef.current.offsetHeight;
        const iw = imageRef.current.offsetWidth;
        const ih = imageRef.current.offsetHeight;
        if (isFullyContained(cw, ch, iw, ih, zoom)) {
          setPosition({ x: 0, y: 0 });
          return;
        }
        const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, zoom);
        setPosition(clampPosition(newX, newY, maxX, maxY));
      } else {
        setPosition({ x: newX, y: newY });
      }
    },
    [isDragging, dragStart, zoom, containerRef, imageRef],
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
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
      }
    },
    [getFitZoom],
  );

  useLayoutEffect(() => {
    if (!image || !containerRef.current || !imageRef.current) return;
    const cw = containerRef.current.offsetWidth;
    const ch = containerRef.current.offsetHeight;
    const iw = imageRef.current.offsetWidth;
    const ih = imageRef.current.offsetHeight;
    if (isFullyContained(cw, ch, iw, ih, zoom)) {
      setPosition({ x: 0, y: 0 });
    }
  }, [image, zoom, containerRef, imageRef]);

  const resetView = useCallback(() => {
    setZoom(1);
    setPosition({ x: 0, y: 0 });
    prevZoomRef.current = 1;
  }, []);

  useLayoutEffect(() => {
    if (prevZoomRef.current === zoom) return;
    const ratio = zoom / prevZoomRef.current;
    setPosition((p) => {
      const newX = p.x * ratio;
      const newY = p.y * ratio;
      if (!containerRef.current || !imageRef.current)
        return { x: newX, y: newY };
      const cw = containerRef.current.offsetWidth;
      const ch = containerRef.current.offsetHeight;
      const iw = imageRef.current.offsetWidth;
      const ih = imageRef.current.offsetHeight;
      const { maxX, maxY } = getPositionBounds(cw, ch, iw, ih, zoom);
      return clampPosition(newX, newY, maxX, maxY);
    });
    prevZoomRef.current = zoom;
  }, [zoom, containerRef, imageRef]);

  return {
    zoom,
    position,
    isDragging,
    setZoom,
    setPosition,
    resetView,
    getFitZoom,
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
  };
}
