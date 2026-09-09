import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useAppStore } from "@/store/appStore"
import type { ExifData } from "@/types"

export function useExifLoader() {
  const toggleExifPanel = useCallback(async () => {
    const { showExifPanel, imageInfo } = useAppStore.getState()

    if (showExifPanel) {
      useAppStore.setState({ showExifPanel: false })
      return
    }

    // imageInfo.file_path가 정답: 아카이브 모드에서는 추출된 임시 경로,
    // 일반 모드에서는 실제 파일 경로. dirImages entry는 아카이브에서
    // zip 내부 이름이라 get_exif_data에 그대로 쓸 수 없다.
    const filePath = imageInfo?.file_path
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
