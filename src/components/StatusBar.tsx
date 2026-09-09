import { useAppStore } from "@/store/appStore"
import { formatFileSize } from "../utils/format"

export function StatusBar() {
  const imageInfo = useAppStore((state) => state.imageInfo)
  return (
    <div className="bg-background absolute bottom-0 flex h-6 w-full items-center justify-between border-t px-2 text-xs">
      <span className="max-w-[60%] overflow-hidden text-ellipsis whitespace-nowrap">
        {imageInfo?.file_name ?? "Empty"}
      </span>
      <span>
        {formatFileSize(imageInfo?.file_size ?? 0)} |{" "}
        {imageInfo?.mime_type ?? "Unknown"}
      </span>
    </div>
  )
}
