import { useAppStore } from "@/store/appStore"
import { formatFileSize } from "../utils/format"

export function StatusBar() {
  const app = useAppStore()
  // if (!app.imageInfo) return null;

  return (
    <div className="bg-background/30 absolute bottom-0 flex h-6 w-full items-center justify-between px-2 text-xs backdrop-blur-sm">
      <span className="max-w-[60%] overflow-hidden text-ellipsis whitespace-nowrap">
        {app.imageInfo?.file_name ?? "Empty"}
      </span>
      <span>
        {formatFileSize(app.imageInfo?.file_size ?? 0)} |{" "}
        {app.imageInfo?.mime_type ?? "Unknown"}
      </span>
    </div>
  )
}
