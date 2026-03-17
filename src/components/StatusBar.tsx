import { useAppStore } from "@/store/appStore";
import { formatFileSize } from "../utils/format";

export function StatusBar() {
  const app = useAppStore();
  // if (!app.imageInfo) return null;

  return (
    <div className="absolute w-full bottom-0 flex items-center justify-between h-6 px-2 bg-background/30 text-xs backdrop-blur-sm">
      <span className="overflow-hidden text-ellipsis whitespace-nowrap max-w-[60%]">
        {app.imageInfo?.file_name ?? "Empty"}
      </span>
      <span>
        {formatFileSize(app.imageInfo?.file_size ?? 0)} |{" "}
        {app.imageInfo?.mime_type ?? "Unknown"}
      </span>
    </div>
  );
}
