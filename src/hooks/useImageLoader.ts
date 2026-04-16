import { useCallback } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import type { DirectoryImages, ImageInfo } from "@/types"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { useSettingsStore } from "@/store/settingsStore"
import { useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"

const ARCHIVE_EXTENSIONS = ["cbz"]

function isArchiveFile(filePath: string): boolean {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? ""
  return ARCHIVE_EXTENSIONS.includes(ext)
}

type LoadImageOptions = {
  refreshDirectory?: boolean
  onAfterLoad?: () => void
}

export function useImageLoader() {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance } =
    useImageCache()

  /** 아카이브 파일을 열어 내부 첫 이미지를 표시 */
  const loadArchive = useCallback(async (archivePath: string) => {
    useAppStore.setState({ loading: true, archivePath })
    try {
      // 아카이브 내부 이미지 목록 가져오기
      const archiveImages = await invoke<DirectoryImages>(
        "get_archive_images",
        { filePath: archivePath }
      )

      // 첫 번째 이미지 추출
      const firstEntry = archiveImages.images[0]
      const imgInfo = await invoke<ImageInfo>("load_archive_image", {
        archivePath,
        entryName: firstEntry
      })

      useAppStore.setState({
        imageInfo: imgInfo,
        dirImages: archiveImages,
        error: null
      })

      void useRecentFilesStore.getState().add(archivePath)
    } catch (e) {
      const message = String(e)
      useAppStore.setState({
        error: message,
        imageInfo: null,
        archivePath: null
      })
      toast.error("아카이브를 불러오지 못했습니다", { description: message })
    } finally {
      useAppStore.setState({ loading: false })
    }
  }, [])

  /** 아카이브 내부 특정 인덱스의 이미지를 로드 */
  const loadArchiveImageByIndex = useCallback(
    async (archivePath: string, entryName: string) => {
      useAppStore.setState({ loading: true })
      try {
        const imgInfo = await invoke<ImageInfo>("load_archive_image", {
          archivePath,
          entryName
        })
        useAppStore.setState({ imageInfo: imgInfo, error: null })
      } catch (e) {
        const message = String(e)
        useAppStore.setState({ error: message })
        toast.error("이미지를 불러오지 못했습니다", { description: message })
      } finally {
        useAppStore.setState({ loading: false })
      }
    },
    []
  )

  const loadImage = useCallback(
    async (filePath: string, options?: LoadImageOptions) => {
      // 아카이브 파일이면 아카이브 모드로 전환
      if (isArchiveFile(filePath)) {
        await loadArchive(filePath)
        options?.onAfterLoad?.()
        return
      }

      // 일반 이미지: 아카이브 모드 해제
      useAppStore.setState({ archivePath: null })

      const refreshDirectory = options?.refreshDirectory ?? true

      useAppStore.setState({ loading: true })
      try {
        const imgInfo = await getOrLoadImage(filePath)
        useAppStore.setState({ imageInfo: imgInfo })
        void useRecentFilesStore.getState().add(filePath)
        options?.onAfterLoad?.()

        let resolvedDirInfo = dirImages

        if (
          refreshDirectory ||
          !resolvedDirInfo ||
          !resolvedDirInfo.images.includes(filePath)
        ) {
          resolvedDirInfo = await invoke<DirectoryImages>(
            "get_directory_images",
            { filePath }
          )
          useAppStore.setState({ dirImages: resolvedDirInfo })
        }

        if (resolvedDirInfo) {
          const resolvedIndex = resolvedDirInfo.images.indexOf(filePath)
          const nextIndex =
            resolvedIndex >= 0 ? resolvedIndex : resolvedDirInfo.current_index
          prefetchNearbyImages(
            resolvedDirInfo.images,
            nextIndex,
            loopNavigation,
            getPrefetchDistance()
          )
        }
      } catch (e) {
        const message = String(e)
        useAppStore.setState({ error: message })
        useAppStore.setState({ imageInfo: null })
        toast.error("이미지를 불러오지 못했습니다", {
          description: message
        })
      } finally {
        useAppStore.setState({ loading: false })
      }
    },
    [
      dirImages,
      getOrLoadImage,
      getPrefetchDistance,
      prefetchNearbyImages,
      loopNavigation,
      loadArchive
    ]
  )

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: [...SUPPORTED_IMAGE_EXTENSIONS]
        }
      ]
    })
    if (selected) await loadImage(selected)
  }, [loadImage])

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const files = e.dataTransfer.files
      if (files.length === 0) return

      // 첫 번째 드롭 항목만 사용 (파일 또는 폴더)
      const first = files[0]
      const droppedPath = (first as { path?: string }).path
      if (!droppedPath) return

      // 아카이브 파일이 드롭된 경우 직접 처리
      if (isArchiveFile(droppedPath)) {
        await loadImage(droppedPath)
        return
      }

      try {
        const resolved = await invoke<string>("resolve_dropped_path", {
          path: droppedPath
        })
        await loadImage(resolved)
      } catch (err) {
        toast.error("드롭한 항목을 열 수 없습니다", {
          description: String(err)
        })
      }
    },
    [loadImage]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])

  return {
    loadImage,
    loadArchiveImageByIndex,
    handleOpenFile,
    handleDrop,
    handleDragOver,
    getOrLoadImage
  }
}
