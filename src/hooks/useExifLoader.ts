import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import { useAppStore } from "@/store/appStore"
import type { ComicInfo, ExifData, ImageDetails, ImageHistogram } from "@/types"
import { errorMessage } from "@/utils/appError"

async function loadHistogramSilent(filePath: string, isCurrent: () => boolean) {
  try {
    const data = await invoke<ImageHistogram>("get_image_histogram", {
      filePath
    })
    if (isCurrent()) {
      useAppStore.setState({ histogramData: data })
    }
  } catch {
    // 디코드 불가 포맷(SVG/AVIF 등)은 섹션을 숨긴다. EXIF와 달리
    // 실패 자체가 정보이므로 에러 상태로 취급하지 않는다.
    if (isCurrent()) {
      useAppStore.setState({ histogramData: null })
    }
  }
}

async function loadDetailsSilent(filePath: string, isCurrent: () => boolean) {
  try {
    const data = await invoke<ImageDetails>("get_image_details", { filePath })
    if (isCurrent()) {
      useAppStore.setState({ imageDetails: data })
    }
  } catch {
    if (isCurrent()) {
      useAppStore.setState({ imageDetails: null })
    }
  }
}

export function useExifLoader() {
  const loadExif = useCallback(async () => {
    // 아카이브 모드에서는 추출된 임시 경로, 일반 모드에서는 실제 파일 경로다.
    // EXIF와 파일 상세는 원본(source_path)을 설명해야 하므로 축소 sidecar가
    // 아닌 원본을 읽고, 히스토그램은 렌더 바이트(file_path)를 그대로 쓴다.
    const { imageInfo, archivePreviewPath } = useAppStore.getState()
    if (archivePreviewPath !== null) {
      // 폴더 미리보기는 CBZ/ZIP 파일 자체를 설명한다. 첫 페이지의 EXIF/히스토그램은 쓰지 않는다.
      useAppStore.setState({ exifData: null, exifError: null, histogramData: null })
      const isCurrentPreview = () =>
        useAppStore.getState().archivePreviewPath === archivePreviewPath
      const [details, comic] = await Promise.all([
        invoke<ImageDetails>("get_image_details", { filePath: archivePreviewPath }).catch(
          () => null
        ),
        invoke<ComicInfo | null>("get_comic_info", { filePath: archivePreviewPath }).then(
          (info) => ({ info: info ?? null, error: null as string | null }),
          (e) => ({ info: null, error: errorMessage(e) })
        )
      ])
      if (!isCurrentPreview()) return
      useAppStore.setState({
        imageDetails: details,
        comicInfo: comic.info,
        comicInfoError: comic.error
      })
      return
    }
    const paintPath = imageInfo?.file_path
    const sourcePath = imageInfo?.source_path ?? imageInfo?.file_path
    if (!sourcePath || !paintPath) return

    // 응답 도착 시점에 표시 중인 이미지가 바뀌었으면 이전 이미지의
    // exif/히스토그램/상세가 새 이미지 패널을 덮어쓰지 않도록 버린다.
    const isCurrent = () => {
      const info = useAppStore.getState().imageInfo
      return info?.file_path === paintPath && (info?.source_path ?? info?.file_path) === sourcePath
    }

    const loadExifData = async () => {
      try {
        const data = await invoke<ExifData>("get_exif_data", { filePath: sourcePath })
        if (!isCurrent()) return
        // EXIF가 없거나 읽을 수 없는 파일은 백엔드가 빈 맵으로 알린다(실패 아님).
        const hasExif = Object.keys(data).length > 0
        useAppStore.setState({ exifData: hasExif ? data : null, exifError: null })
      } catch (e) {
        if (isCurrent()) {
          useAppStore.setState({ exifData: null, exifError: errorMessage(e) })
        }
      }
    }
    // EXIF, 히스토그램, 파일 상세는 서로 독립이라 함께 시작한다.
    await Promise.all([
      loadExifData(),
      loadHistogramSilent(paintPath, isCurrent),
      loadDetailsSilent(sourcePath, isCurrent)
    ])
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
