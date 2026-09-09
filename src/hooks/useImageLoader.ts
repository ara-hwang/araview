import { useCallback, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import type { DirectoryImages, ImageInfo } from "@/types"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { useSettingsStore } from "@/store/settingsStore"
import { useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { buildDirListOptions } from "@/utils/directoryOptions"

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

  const [isDragOver, setIsDragOver] = useState(false)
  const dragDepthRef = useRef(0)

  /** 아카이브 파일을 열어 내부 첫 이미지를 표시 */
  const loadArchive = useCallback(async (archivePath: string) => {
    useAppStore.setState({ loading: true, archivePath })
    try {
      const archiveImages = await invoke<DirectoryImages>(
        "get_archive_images",
        { filePath: archivePath }
      )

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
      if (isArchiveFile(filePath)) {
        await loadArchive(filePath)
        options?.onAfterLoad?.()
        return
      }

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
            {
              filePath,
              options: buildDirListOptions(useSettingsStore.getState())
            }
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
      dragDepthRef.current = 0
      setIsDragOver(false)
      const files = Array.from(e.dataTransfer.files)
      if (files.length === 0) return

      const rawPaths = files
        .map((f) => (f as { path?: string }).path)
        .filter((p): p is string => !!p)
      if (rawPaths.length === 0) return

      // 여러 파일/폴더 드롭: 각각 해석 후 첫 이미지를 열고 나머지는 알림
      const resolved: string[] = []
      for (const raw of rawPaths) {
        if (isArchiveFile(raw)) {
          resolved.push(raw)
          continue
        }
        try {
          const one = await invoke<string>("resolve_dropped_path", {
            path: raw
          })
          resolved.push(one)
        } catch {
          // 해석 실패 항목은 건너뜀
        }
      }
      if (resolved.length === 0) {
        toast.error("드롭한 항목을 열 수 없습니다", {
          description: rawPaths[0]
        })
        return
      }

      resolved.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))
      await loadImage(resolved[0])
      if (resolved.length > 1) {
        toast.info(`${resolved.length}개 중 첫 이미지를 표시합니다`, {
          duration: 2000
        })
      }
    },
    [loadImage]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragOver(true)
  }, [])

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragDepthRef.current += 1
    setIsDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1)
    if (dragDepthRef.current === 0) setIsDragOver(false)
  }, [])

  return {
    loadImage,
    loadArchiveImageByIndex,
    handleOpenFile,
    handleDrop,
    handleDragOver,
    handleDragEnter,
    handleDragLeave,
    isDragOver,
    getOrLoadImage
  }
}
