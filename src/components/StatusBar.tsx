import type { ImageInfo } from "../types";
import { formatFileSize } from "../utils/format";

interface StatusBarProps {
  image: ImageInfo | null;
}

export function StatusBar({ image }: StatusBarProps) {
  if (!image) return null;

  return (
    <div className="flex items-center justify-between h-7 px-3 bg-[hsl(var(--card))] border-t border-[hsl(var(--border))] flex-shrink-0 text-xs text-[hsl(var(--muted-foreground))]">
      <span className="overflow-hidden text-ellipsis whitespace-nowrap max-w-[60%]">{image.file_name}</span>
      <span className="flex-shrink-0 opacity-70">
        {formatFileSize(image.file_size)} | {image.mime_type}
      </span>
    </div>
  );
}
