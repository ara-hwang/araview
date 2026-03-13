import type { DirectoryImages } from "../types";

interface ToolbarProps {
  dirImages: DirectoryImages | null;
  currentIndex: number;
  zoom: number;
  onOpenFile: () => void;
  onNavigate: (direction: "prev" | "next") => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
}

export function Toolbar({
  dirImages,
  currentIndex,
  zoom,
  onOpenFile,
  onNavigate,
  onZoomIn,
  onZoomOut,
  onResetZoom,
}: ToolbarProps) {
  return (
    <div className="toolbar">
      <div className="toolbar-left">
        <button className="toolbar-btn" onClick={onOpenFile} title="Open file (Ctrl+O)">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
          <span>Open</span>
        </button>
      </div>

      <div className="toolbar-center">
        {dirImages && dirImages.images.length > 1 && (
          <>
            <button className="toolbar-btn" onClick={() => onNavigate("prev")} title="Previous image">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polyline points="15 18 9 12 15 6" />
              </svg>
            </button>
            <span className="image-counter">
              {currentIndex + 1} / {dirImages.images.length}
            </span>
            <button className="toolbar-btn" onClick={() => onNavigate("next")} title="Next image">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polyline points="9 18 15 12 9 6" />
              </svg>
            </button>
          </>
        )}
      </div>

      <div className="toolbar-right">
        <button className="toolbar-btn" onClick={onZoomOut} title="Zoom out (-)">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        </button>
        <span className="zoom-level" onClick={onResetZoom} title="Reset zoom (0)">
          {Math.round(zoom * 100)}%
        </span>
        <button className="toolbar-btn" onClick={onZoomIn} title="Zoom in (+)">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="11" y1="8" x2="11" y2="14" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        </button>
      </div>
    </div>
  );
}
