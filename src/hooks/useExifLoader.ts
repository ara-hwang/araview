import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import { useAppStore } from "@/store/appStore"
import type { ExifData, ImageDetails, ImageHistogram } from "@/types"
import { errorMessage } from "@/utils/appError"

async function loadHistogramSilent(filePath: string) {
  try {
    const data = await invoke<ImageHistogram>("get_image_histogram", {
      filePath
    })
    useAppStore.setState({ histogramData: data })
  } catch {
    // 디코드 불가 포맷(SVG/AVIF 등)은 섹션을 숨긴다. EXIF와 달리
    // 실패 자체가 정보이므로 에러 상태로 취급하지 않는다.
    useAppStore.setState({ histogramData: null })
  }
}

async function loadDetailsSilent(filePath: string) {
  try {
    const data = await invoke<ImageDetails>("get_image_details", { filePath })
    useAppStore.setState({ imageDetails: data })
  } catch {
    useAppStore.setState({ imageDetails: null })
  }
}

export function useExifLoader() {
  const loadExif = useCallback(async () => {
    // 아카이브 모드에서는 추출된 임시 경로, 일반 모드에서는 실제 파일 경로다.
    // EXIF와 파일 상세는 원본(source_path)을 설명해야 하므로 축소 sidecar가
    // 아닌 원본을 읽고, 히스토그램은 렌더 바이트(file_path)를 그대로 쓴다.
    const { imageInfo } = useAppStore.getState()
    const paintPath = imageInfo?.file_path
    const sourcePath = imageInfo?.source_path ?? imageInfo?.file_path
    if (!sourcePath || !paintPath) return

    try {
      const data = await invoke<ExifData>("get_exif_data", { filePath: sourcePath })
      useAppStore.setState({ exifData: data, exifError: null })
    } catch (e) {
      // EXIF 자체가 없는 파일은 실패가 아닌 빈 상태로 취급한다.
      // 백엔드는 "No EXIF data found: ..." 에러로 알리므로 여기서 구분한다.
      const message = errorMessage(e)
      if (message.toLowerCase().includes("no exif")) {
        useAppStore.setState({ exifData: null, exifError: null })
      } else {
        useAppStore.setState({ exifData: null, exifError: message })
      }
    }
    // 히스토그램/파일 상세는 EXIF 유무와 독립적으로 병렬 로드한다.
    await Promise.all([loadHistogramSilent(paintPath), loadDetailsSilent(sourcePath)])
  }, [])

  const toggleExifPanel = useCallback(async () => {
    const { showExifPanel, imageInfo } = useAppStore.getState()

    if (showExifPanel) {
      useAppStore.setState({ showExifPanel: false })
      return
    }

    if (!imageInfo?.file_path) return
    await loadExif()
    useAppStore.setState({ showExifPanel: true })
  }, [loadExif])

  return { toggleExifPanel, reloadExif: loadExif }
}
