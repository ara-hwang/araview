import { useRef } from "react";
import type { ImageInfo } from "../types";

interface ImageContainerProps {
  image: ImageInfo | null;
  loading: boolean;
  error: string | null;
  zoom: number;
  isDragging: boolean;
  position: { x: number; y: number };
  onWheel: (e: React.WheelEvent) => void;
  onMouseDown: (e: React.MouseEvent) => void;
  onMouseMove: (e: React.MouseEvent) => void;
  onMouseUp: () => void;
}

export function ImageContainer({
  image,
  loading,
  error,
  zoom,
  isDragging,
  position,
  onWheel,
  onMouseDown,
  onMouseMove,
  onMouseUp,
}: ImageContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const imageSrc = image ? `data:${image.mime_type};base64,${image.base64}` : null;

  return (
    <div
      ref={containerRef}
      className={`image-container ${isDragging ? "dragging" : ""}`}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
    >
      {loading && (
        <div className="loading">Loading...</div>
      )}

      {error && (
        <div className="error-message">{error}</div>
      )}

      {!image && !loading && !error && (
        <div className="empty-state">
          <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" opacity="0.4">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            <circle cx="8.5" cy="8.5" r="1.5" />
            <polyline points="21 15 16 10 5 21" />
          </svg>
          <p>Drag &amp; drop an image here</p>
          <p className="hint">or press Ctrl+O to open a file</p>
        </div>
      )}

      {imageSrc && !loading && (
        <img
          src={imageSrc}
          alt={image?.file_name}
          className="viewer-image"
          style={{
            transform: `translate(${position.x}px, ${position.y}px) scale(${zoom})`,
            cursor: zoom > 1 ? (isDragging ? "grabbing" : "grab") : "default",
          }}
          draggable={false}
        />
      )}
    </div>
  );
}
