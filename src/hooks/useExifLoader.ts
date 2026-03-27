import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useAppStore } from "@/store/appStore"
import type { ExifData } from "@/types"

export function useExifLoader() {
  const toggleExifPanel = useCallback(async () => {
    const { showExifPanel, dirImages } = useAppStore.getState()

    if (showExifPanel) {
      useAppStore.setState({ showExifPanel: false })
      return
    }

    const filePath = dirImages.images[dirImages.current_index]
    if (!filePath) return

    try {
      const data = await invoke<ExifData>("get_exif_data", { filePath })
      useAppStore.setState({ exifData: data, showExifPanel: true })
    } catch {
      useAppStore.setState({ exifData: null, showExifPanel: true })
    }
  }, [])

  return { toggleExifPanel }
}
