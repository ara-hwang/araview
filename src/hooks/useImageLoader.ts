import { useCallback, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import i18n from "@/i18n"
import type { DirectoryImages, ImageInfo } from "@/types"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { useSettingsStore } from "@/store/settingsStore"
import { updateDirImagesIndex, useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { buildDirListOptions } from "@/utils/directoryOptions"
import { MAX_SKIP_ATTEMPTS, findSkipTarget } from "@/utils/skipBroken"

const ARCHIVE_EXTENSIONS = ["cbz", "cb7"]

function isArchiveFile(filePath: string): boolean {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? ""
  return ARCHIVE_EXTENSIONS.includes(ext)
}

type LoadImageOptions = {
  refreshDirectory?: boolean
  onAfterLoad?: () => void
  /** 자동 스킵 연쇄 깊이 (내부용, 무한 재귀 방지) */
  skipDepth?: number
}

export function useImageLoader() {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  const viewMode = useSettingsStore((state) => state.viewMode)

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance } =
    useImageCache()

  const [isDragOver, setIsDragOver] = useState(false)
  const dragDepthRef = useRef(0)

  /** 아카이브 파일을 열어 내부 첫 이미지(또는 이어보기 위치)를 표시 */
  const loadArchive = useCallback(async (archivePath: string) => {
    useAppStore.setState({ loading: true, archivePath })
    try {
      const archiveImages = await invoke<DirectoryImages>(
        "get_archive_images",
        { filePath: archivePath }
      )

      // 저장된 이어보기 엔트리가 목록에 있으면 거기서 시작
      const saved = useArchiveProgressStore.getState().get(archivePath)
      const startIndex =
        saved !== null ? archiveImages.images.indexOf(saved) : 0
      const firstEntry =
        startIndex > 0
          ? archiveImages.images[startIndex]
          : archiveImages.images[0]
      const imgInfo = await invoke<ImageInfo>("load_archive_image", {
        archivePath,
        entryName: firstEntry
      })

      useAppStore.setState({
        imageInfo: imgInfo,
        dirImages: {
          images: archiveImages.images,
          current_index: startIndex > 0 ? startIndex : 0
        },
        error: null
      })
      void useArchiveProgressStore.getState().save(archivePath, firstEntry)

      if (useSettingsStore.getState().recordRecentFiles) {
        void useRecentFilesStore.getState().add(archivePath)
      }
    } catch (e) {
      const message = String(e)
      useAppStore.setState({
        error: message,
        imageInfo: null,
        archivePath: null
      })
      toast.error(i18n.t("toast.load.archiveFail"), { description: message })
    } finally {
      useAppStore.setState({ loading: false })
    }
  }, [])

  /** 아카이브 내부 특정 인덱스의 이미지를 로드 */
  const loadArchiveImageByIndex = useCallback(
    async (archivePath: string, entryName: string, skipDepth = 0) => {
      useAppStore.setState({ loading: true })
      try {
        const imgInfo = await invoke<ImageInfo>("load_archive_image", {
          archivePath,
          entryName
        })
        useAppStore.setState({ imageInfo: imgInfo, error: null })
        useAppStore.getState().removeFailedPath(entryName)
        void useArchiveProgressStore.getState().save(archivePath, entryName)
      } catch (e) {
        const message = String(e)
        useAppStore.setState({ error: message })
        useAppStore.getState().addFailedPath(entryName)
        const settings = useSettingsStore.getState()
        if (settings.skipBrokenFiles && skipDepth < MAX_SKIP_ATTEMPTS) {
          const st = useAppStore.getState()
          const failedIndex = st.dirImages.images.indexOf(entryName)
          const target = findSkipTarget(
            st.dirImages.images,
            failedIndex >= 0 ? failedIndex : st.dirImages.current_index,
            new Set(st.failedPaths),
            settings.loopNavigation
          )
          if (target !== null) {
            const nextEntry = st.dirImages.images[target]
            toast.info(i18n.t("toast.load.skipped"), {
              description: entryName
            })
            await loadArchiveImageByIndex(archivePath, nextEntry, skipDepth + 1)
            updateDirImagesIndex(target)
            return
          }
        }
        toast.error(i18n.t("toast.load.imageFail"), { description: message })
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
        useAppStore.getState().removeFailedPath(filePath)
        if (useSettingsStore.getState().recordRecentFiles) {
          void useRecentFilesStore.getState().add(filePath)
        }
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
          // 보기 모드별 프리페치: 양면은 짝 페이지를, webtoon은 스크롤 앞쪽을 더 넓게.
          const baseDistance = getPrefetchDistance()
          const prefetchDistance =
            viewMode === "webtoon"
              ? baseDistance * 2
              : viewMode === "single"
                ? baseDistance
                : baseDistance + 1
          prefetchNearbyImages(
            resolvedDirInfo.images,
            nextIndex,
            loopNavigation,
            prefetchDistance
          )
        }
      } catch (e) {
        const message = String(e)
        useAppStore.setState({ error: message })
        useAppStore.setState({ imageInfo: null })
        useAppStore.getState().addFailedPath(filePath)
        const settings = useSettingsStore.getState()
        const skipDepth = options?.skipDepth ?? 0
        if (settings.skipBrokenFiles && skipDepth < MAX_SKIP_ATTEMPTS) {
          const st = useAppStore.getState()
          const failedIndex = st.dirImages.images.indexOf(filePath)
          const target =
            failedIndex >= 0
              ? findSkipTarget(
                  st.dirImages.images,
                  failedIndex,
                  new Set(st.failedPaths),
                  settings.loopNavigation
                )
              : null
          if (target !== null) {
            const nextPath = st.dirImages.images[target]
            toast.info(i18n.t("toast.load.skipped"), {
              description: filePath
            })
            await loadImage(nextPath, {
              refreshDirectory: false,
              skipDepth: skipDepth + 1,
              onAfterLoad: options?.onAfterLoad
            })
            updateDirImagesIndex(target)
            return
          }
        }
        toast.error(i18n.t("toast.load.imageFail"), {
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
      viewMode,
      loadArchive
    ]
  )

  const handleOpenFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: i18n.t("picker.images"),
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
        toast.error(i18n.t("toast.drop.fail"), {
          description: rawPaths[0]
        })
        return
      }

      resolved.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))
      await loadImage(resolved[0])
      if (resolved.length > 1) {
        toast.info(i18n.t("toast.drop.firstOf", { count: resolved.length }), {
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
