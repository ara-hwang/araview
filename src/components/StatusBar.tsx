import type { ImageInfo } from "../types";
import { formatFileSize } from "../utils/format";

interface StatusBarProps {
  image: ImageInfo | null;
}

export function StatusBar({ image }: StatusBarProps) {
  if (!image) return null;

  return (
    <div className="status-bar">
      <span className="file-name">{image.file_name}</span>
      <span className="file-info">
        {formatFileSize(image.file_size)} | {image.mime_type}
      </span>
    </div>
  );
}
