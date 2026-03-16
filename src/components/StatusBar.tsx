import { useAppStore } from "@/store/appStore";
import { formatFileSize } from "../utils/format";

export function StatusBar() {
  const app = useAppStore();
  if (!app.imageInfo) return null;

  return (
    <div className="flex items-center justify-between h-7 px-3 bg-[hsl(var(--card))] shrink-0 text-xs text-[hsl(var(--muted-foreground))]">
      <span className="overflow-hidden text-ellipsis whitespace-nowrap max-w-[60%]">
        {app.imageInfo.file_name}
      </span>
      <span className="shrink-0 opacity-70">
        {formatFileSize(app.imageInfo.file_size)} | {app.imageInfo.mime_type}
      </span>
    </div>
  );
}
