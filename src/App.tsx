import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

interface ImageInfo {
  base64: string;
  mime_type: string;
  file_name: string;
  file_size: number;
}

interface DirectoryImages {
  images: string[];
  current_index: number;
}

function App() {
  const [image, setImage] = useState<ImageInfo | null>(null);
  const [dirImages, setDirImages] = useState<DirectoryImages | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [isDragging, setIsDragging] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const imageContainerRef = useRef<HTMLDivElement>(null);

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
          extensions: ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif", "avif"],
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

  // Mouse wheel zoom
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    if (e.deltaY < 0) {
      setZoom((prev) => Math.min(prev * 1.1, 10));
    } else {
      setZoom((prev) => Math.max(prev / 1.1, 0.1));
    }
  }, []);

  // Pan (drag to move)
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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowLeft":
          navigateImage("prev");
          break;
        case "ArrowRight":
          navigateImage("next");
          break;
        case "+":
        case "=":
          handleZoomIn();
          break;
        case "-":
          handleZoomOut();
          break;
        case "0":
          handleResetZoom();
          break;
        case "o":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            handleOpenFile();
          }
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigateImage, handleZoomIn, handleZoomOut, handleResetZoom, handleOpenFile]);

  // Drag and drop
  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const files = e.dataTransfer.files;
    if (files.length > 0) {
      const file = files[0];
      // Tauri exposes the file path via the webkitRelativePath or we use the name
      // For Tauri v2, we need to get the path from the drop event
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

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const imageSrc = image ? `data:${image.mime_type};base64,${image.base64}` : null;

  return (
    <div
      className="app"
      onDrop={handleDrop}
      onDragOver={handleDragOver}
    >
      {/* Toolbar */}
      <div className="toolbar">
        <div className="toolbar-left">
          <button className="toolbar-btn" onClick={handleOpenFile} title="Open file (Ctrl+O)">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
            <span>Open</span>
          </button>
        </div>

        <div className="toolbar-center">
          {dirImages && dirImages.images.length > 1 && (
            <>
              <button className="toolbar-btn" onClick={() => navigateImage("prev")} title="Previous image">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <polyline points="15 18 9 12 15 6" />
                </svg>
              </button>
              <span className="image-counter">
                {currentIndex + 1} / {dirImages.images.length}
              </span>
              <button className="toolbar-btn" onClick={() => navigateImage("next")} title="Next image">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <polyline points="9 18 15 12 9 6" />
                </svg>
              </button>
            </>
          )}
        </div>

        <div className="toolbar-right">
          <button className="toolbar-btn" onClick={handleZoomOut} title="Zoom out (-)">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
              <line x1="8" y1="11" x2="14" y2="11" />
            </svg>
          </button>
          <span className="zoom-level" onClick={handleResetZoom} title="Reset zoom (0)">
            {Math.round(zoom * 100)}%
          </span>
          <button className="toolbar-btn" onClick={handleZoomIn} title="Zoom in (+)">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
              <line x1="11" y1="8" x2="11" y2="14" />
              <line x1="8" y1="11" x2="14" y2="11" />
            </svg>
          </button>
        </div>
      </div>

      {/* Image area */}
      <div
        ref={imageContainerRef}
        className={`image-container ${isDragging ? "dragging" : ""}`}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
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
            <p>Drag & drop an image here</p>
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

      {/* Status bar */}
      {image && (
        <div className="status-bar">
          <span className="file-name">{image.file_name}</span>
          <span className="file-info">
            {formatFileSize(image.file_size)} | {image.mime_type}
          </span>
        </div>
      )}
    </div>
  );
}

export default App;
