import { invoke } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"
import { useCallback, useEffect, useRef, useState } from "react"

import { toast } from "@/components/ui/toast"
import { SUPPORTED_IMAGE_EXTENSIONS } from "@/constants/imageExtensions"
import { useImageCache } from "@/hooks/useImageCache"
import { clearPixelArtDetectionCache } from "@/hooks/usePixelArtDetection"
import i18n from "@/i18n"
import { setImageInfoAndResetView, updateDirImagesIndex, useAppStore } from "@/store/appStore"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ComicInfo, DirectoryImages, ImageInfo, ThumbnailInfo } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"
import { isArchiveFilePath } from "@/utils/archiveFile"
import { resolveArchiveStartIndex } from "@/utils/archiveResume"
import { resolveCoverIndex } from "@/utils/comicCover"
import { buildDirListOptions } from "@/utils/directoryOptions"
import { resolvePairStart } from "@/utils/dirNavigation"
import { beginImageLoad, isCurrentImageLoad } from "@/utils/imageLoadSession"
import { shouldUsePreviewThumbnail } from "@/utils/previewThumbnail"
import { maxSideForResolution } from "@/utils/resolutionLimit"
import { MAX_SKIP_ATTEMPTS, findSkipTarget } from "@/utils/skipBroken"

function endImageLoadIfCurrent(token: number): void {
  if (isCurrentImageLoad(token)) {
    useAppStore.setState({ loading: false })
  }
}

function imageRenderArgs() {
  const settings = useSettingsStore.getState()
  return {
    imageScalingMode: settings.imageScalingMode,
    autoDetectPixelArt: settings.autoDetectPixelArt
  }
}

type ComicInfoResult = { info: ComicInfo | null; error: string | null }

/**
 * ComicInfo.xml 메타데이터를 조용히 읽는다. XML 부재는 에러가 아니고,
 * 깨진 XML은 패널의 에러 상태로 보여주기 위해 메시지를 실어 돌려준다.
 */
function readComicInfoSilent(archivePath: string): Promise<ComicInfoResult> {
  return invoke<ComicInfo | null>("get_comic_info", { filePath: archivePath }).then(
    (info) => ({ info: info ?? null, error: null }),
    (e) => ({ info: null, error: errorMessage(e) })
  )
}

export type ArchiveOpenMode = "full" | "folderPreview"

export type LoadImageOptions = {
  refreshDirectory?: boolean
  onAfterLoad?: () => void
  /** 자동 스킵 연쇄 깊이 (내부용, 무한 재귀 방지) */
  skipDepth?: number
  /** 아카이브일 때: full=만화 모드, folderPreview=폴더 목록 유지·1쪽 미리보기 (기본) */
  archiveOpen?: ArchiveOpenMode
}

export function useImageLoader() {
  const dirImages = useAppStore((state) => state.dirImages)
  const loopNavigation = useSettingsStore((state) => state.loopNavigation)
  const viewMode = useSettingsStore((state) => state.viewMode)

  const { getOrLoadImage, prefetchNearbyImages, getPrefetchDistance, clearImageMetaCache } =
    useImageCache()

  const [isDragOver, setIsDragOver] = useState(false)
  const dragDepthRef = useRef(0)

  // 아카이브 이웃 선추출. 현재 페인트 후 다음 페이지를 백그라운드로 추출+디코딩해
  // 만화 Next 체감을 올린다. CB7 역압축 경합을 피하려고 거리를 최대 2로 묶는다.
  // current imageInfo/progress를 덮지 않는 fire-and-forget 경로만 쓴다.
  const prefetchArchiveNeighbors = useCallback(
    (archivePath: string, images: string[], currentIndex: number) => {
      const settings = useSettingsStore.getState()
      if (settings.cacheMode === "off") return
      const baseDistance = getPrefetchDistance()
      if (baseDistance <= 0) return
      const bonus =
        settings.viewMode === "webtoon" ? baseDistance : settings.viewMode === "single" ? 0 : 1
      const distance = Math.min(Math.max(baseDistance + bonus, 1), 2)
      // 디스크 선추출은 오픈 1회 배치로, 픽셀 예열은 기존 경로로.
      const total = images.length
      const targets: string[] = []
      for (let k = 1; k <= distance; k += 1) {
        for (const idx of [currentIndex + k, currentIndex - k]) {
          let t = idx
          if (t < 0 || t >= total) {
            if (!settings.loopNavigation || total <= 1) continue
            t = ((t % total) + total) % total
          }
          const name = images[t]
          if (name) targets.push(name)
        }
      }
      if (targets.length > 0) {
        void invoke("archive_prefetch", {
          archivePath,
          entryNames: targets
        }).catch(() => {})
      }
      prefetchNearbyImages(images, currentIndex, settings.loopNavigation, distance)
    },
    [getPrefetchDistance, prefetchNearbyImages]
  )

  /** 아카이브 내부 특정 엔트리의 이미지를 로드 */
  const loadArchiveImageByIndex = useCallback(
    async (archivePath: string, entryName: string, skipDepth = 0) => {
      const loadToken = beginImageLoad()
      useAppStore.setState({ loading: true })
      try {
        // 메타 캐시/in-flight 맵을 경유한다. 선축충·웹툰 예열이 이미 같은
        // 엔트리를 로드 중이면 추출 중복 없이 캐시 히트로 끝난다.
        const scopeMatches = useAppStore.getState().archivePath === archivePath
        const imgInfo = scopeMatches
          ? await getOrLoadImage(entryName, { protectArchive: true })
          : await invoke<ImageInfo>("load_archive_image", {
              archivePath,
              entryName,
              maxSide: maxSideForResolution(useSettingsStore.getState().maxResolution),
              ...imageRenderArgs()
            })
        if (!isCurrentImageLoad(loadToken)) return
        setImageInfoAndResetView(imgInfo)
        useAppStore.getState().removeFailedPath(entryName)

        const st = useAppStore.getState()
        const currentIndex = st.dirImages.images.indexOf(entryName)
        void useArchiveProgressStore.getState().save(archivePath, entryName, {
          index: currentIndex >= 0 ? currentIndex : undefined,
          total: st.dirImages.images.length || undefined
        })
        if (currentIndex >= 0) {
          prefetchArchiveNeighbors(archivePath, st.dirImages.images, currentIndex)
        }
      } catch (e) {
        if (!isCurrentImageLoad(loadToken)) return
        const message = errorMessage(e)
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
        toast.error(i18n.t("toast.load.imageFail"), {
          description: message,
          details: errorCopyDetails(e, archivePath, entryName)
        })
      } finally {
        endImageLoadIfCurrent(loadToken)
      }
    },
    [getOrLoadImage, prefetchArchiveNeighbors]
  )

  /** 아카이브 파일을 열어 내부 첫 이미지(또는 이어보기 위치)를 표시 */
  const loadArchive = useCallback(
    async (archivePath: string, options?: { onAfterLoad?: () => void }) => {
      clearPixelArtDetectionCache()
      const loadToken = beginImageLoad()
      clearImageMetaCache()
      // ComicInfo는 아카이브 경로만 필요하므로 목록 조회와 병행한다.
      const comicPromise = readComicInfoSilent(archivePath)
      useAppStore.setState({
        loading: true,
        archivePath,
        archivePreviewPath: null,
        comicInfo: null,
        comicInfoError: null
      })
      try {
        const archiveImages = await invoke<DirectoryImages>("get_archive_images", {
          filePath: archivePath
        })

        // 표지 판정(ComicInfo FrontCover)에 필요해 추출 전에 먼저 확정한다.
        const comic = await comicPromise

        // 이어보기 설정이 켜져 있고 저장된 엔트리가 목록에 있으면 거기서 시작
        const resumeEnabled = useSettingsStore.getState().resumeReading
        const saved = resumeEnabled ? useArchiveProgressStore.getState().get(archivePath) : null
        const total = archiveImages.images.length
        const rawStartIndex = resolveArchiveStartIndex(archiveImages.images, saved)
        // 양면 보기에서 쌍 중간에 착지하면 화면이 겹치므로 쌍 시작으로 맞춰 연다.
        const settings = useSettingsStore.getState()
        const isDualView =
          settings.viewMode === "left-to-right" || settings.viewMode === "right-to-left"
        const startIndex = isDualView
          ? resolvePairStart(
              rawStartIndex,
              total,
              settings.showCoverAlone,
              resolveCoverIndex(comic.info, total)
            )
          : rawStartIndex
        const firstEntry = archiveImages.images[startIndex]
        const imgInfo = await invoke<ImageInfo>("load_archive_image", {
          archivePath,
          entryName: firstEntry,
          maxSide: maxSideForResolution(useSettingsStore.getState().maxResolution),
          ...imageRenderArgs()
        })

        if (!isCurrentImageLoad(loadToken)) return

        useAppStore.setState({
          dirImages: {
            ...archiveImages,
            current_index: startIndex
          }
        })
        setImageInfoAndResetView(imgInfo)

        // 표시한 뒤 메타데이터를 반영한다 (이전 로드의 응답은 무시).
        if (isCurrentImageLoad(loadToken)) {
          useAppStore.setState({ comicInfo: comic.info, comicInfoError: comic.error })
        }

        // 이어보기를 끈 상태에서는 열기만으로 저장 위치를 0페이지로 덮지 않는다.
        if (resumeEnabled) {
          void useArchiveProgressStore.getState().save(archivePath, firstEntry, {
            index: startIndex,
            total
          })
        }

        if (startIndex > 0) {
          toast.info(i18n.t("toast.archive.resumed", { index: startIndex + 1, total }), {
            actionProps: {
              children: i18n.t("toast.archive.startOver"),
              onClick: () => {
                void loadArchiveImageByIndex(archivePath, archiveImages.images[0])
                updateDirImagesIndex(0)
              }
            }
          })
        }

        if (useSettingsStore.getState().recordRecentFiles) {
          void useRecentFilesStore.getState().add(archivePath)
        }

        prefetchArchiveNeighbors(archivePath, archiveImages.images, startIndex)
        options?.onAfterLoad?.()
      } catch (e) {
        if (!isCurrentImageLoad(loadToken)) return
        const message = errorMessage(e)
        useAppStore.setState({
          error: message,
          imageInfo: null,
          archivePath: null,
          comicInfo: null,
          comicInfoError: null
        })
        toast.error(i18n.t("toast.load.archiveFail"), {
          description: message,
          details: errorCopyDetails(e, archivePath)
        })
      } finally {
        endImageLoadIfCurrent(loadToken)
      }
    },
    [clearImageMetaCache, loadArchiveImageByIndex, prefetchArchiveNeighbors]
  )

  /** 폴더 목록을 유지하고 아카이브 첫 페이지만 미리본다 (이어보기·내부 목록 미적용). */
  const loadArchivePreview = useCallback(
    async (archivePath: string, options?: LoadImageOptions) => {
      clearPixelArtDetectionCache()
      const loadToken = beginImageLoad()
      clearImageMetaCache()
      useAppStore.setState({
        loading: true,
        archivePath: null,
        archivePreviewPath: archivePath,
        comicInfo: null,
        comicInfoError: null
      })
      try {
        const archiveImages = await invoke<DirectoryImages>("get_archive_images", {
          filePath: archivePath
        })
        const firstEntry = archiveImages.images[0]
        if (!firstEntry) {
          throw new Error("No images in archive")
        }
        const imgInfo = await invoke<ImageInfo>("load_archive_image", {
          archivePath,
          entryName: firstEntry,
          maxSide: maxSideForResolution(useSettingsStore.getState().maxResolution),
          ...imageRenderArgs()
        })
        if (!isCurrentImageLoad(loadToken)) return
        setImageInfoAndResetView(imgInfo)
        useAppStore.getState().removeFailedPath(archivePath)
        options?.onAfterLoad?.()
      } catch (e) {
        if (!isCurrentImageLoad(loadToken)) return
        const message = errorMessage(e)
        useAppStore.setState({ error: message, imageInfo: null, archivePreviewPath: null })
        toast.error(i18n.t("toast.load.archiveFail"), {
          description: message,
          details: errorCopyDetails(e, archivePath)
        })
      } finally {
        endImageLoadIfCurrent(loadToken)
      }
    },
    [clearImageMetaCache]
  )

  const openArchiveFromPreview = useCallback(
    async (options?: { onAfterLoad?: () => void }) => {
      const preview = useAppStore.getState().archivePreviewPath
      if (!preview) return
      await loadArchive(preview, options)
    },
    [loadArchive]
  )

  const loadImage = useCallback(
    async (filePath: string, options?: LoadImageOptions) => {
      if (isArchiveFilePath(filePath)) {
        const mode = options?.archiveOpen ?? "folderPreview"
        if (mode === "full") {
          await loadArchive(filePath, { onAfterLoad: options?.onAfterLoad })
        } else {
          await loadArchivePreview(filePath, options)
        }
        return
      }

      clearPixelArtDetectionCache()
      const loadToken = beginImageLoad()
      const prior = useAppStore.getState()
      const leavingArchive = prior.archivePath !== null || prior.archivePreviewPath !== null
      useAppStore.setState({
        archivePath: null,
        archivePreviewPath: null,
        comicInfo: null,
        comicInfoError: null
      })
      if (leavingArchive) clearImageMetaCache()

      const refreshDirectory = options?.refreshDirectory ?? true

      useAppStore.setState({ loading: true })
      // 캐시된 저해상 썸네일이 있으면 풀사이즈 디코드와 병행해 조회한다(생성은 하지 않음).
      const cachedPreview = invoke<ThumbnailInfo | null>("get_cached_thumbnail", {
        filePath
      }).catch(() => null)
      try {
        const imgInfo = await getOrLoadImage(filePath)
        if (!isCurrentImageLoad(loadToken)) return
        setImageInfoAndResetView(imgInfo)
        const preview = await cachedPreview
        if (preview && isCurrentImageLoad(loadToken) && shouldUsePreviewThumbnail(imgInfo)) {
          useAppStore.setState({ previewPath: preview.file_path })
        }
        useAppStore.getState().removeFailedPath(filePath)
        if (useSettingsStore.getState().recordRecentFiles) {
          void useRecentFilesStore.getState().add(filePath)
        }
        options?.onAfterLoad?.()

        let resolvedDirInfo = dirImages

        if (refreshDirectory || !resolvedDirInfo || !resolvedDirInfo.images.includes(filePath)) {
          resolvedDirInfo = await invoke<DirectoryImages>("get_directory_images", {
            filePath,
            options: buildDirListOptions(useSettingsStore.getState())
          })
          if (!isCurrentImageLoad(loadToken)) return
          useAppStore.setState({ dirImages: resolvedDirInfo })
        }

        if (resolvedDirInfo) {
          const resolvedIndex = resolvedDirInfo.images.indexOf(filePath)
          const nextIndex = resolvedIndex >= 0 ? resolvedIndex : resolvedDirInfo.current_index
          // 보기 모드별 프리페치: 양면은 짝 페이지를, webtoon은 스크롤 앞쪽을 더 넓게.
          const baseDistance = getPrefetchDistance()
          const prefetchDistance =
            viewMode === "webtoon"
              ? baseDistance * 2
              : viewMode === "single"
                ? baseDistance
                : baseDistance + 1
          prefetchNearbyImages(resolvedDirInfo.images, nextIndex, loopNavigation, prefetchDistance)
        }
      } catch (e) {
        if (!isCurrentImageLoad(loadToken)) return
        const message = errorMessage(e)
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
          description: message,
          details: errorCopyDetails(e, filePath)
        })
      } finally {
        endImageLoadIfCurrent(loadToken)
      }
    },
    [
      clearImageMetaCache,
      dirImages,
      getOrLoadImage,
      getPrefetchDistance,
      prefetchNearbyImages,
      loopNavigation,
      viewMode,
      loadArchive,
      loadArchivePreview
    ]
  )

  // 표시 해상도나 보간 정책이 바뀌면 이전 기준으로 캐시된 메타를 버리고 현재
  // 이미지를 새 기준으로 다시 로드한다. 폴더 목록 인덱스는 유지된다.
  const maxResolution = useSettingsStore((state) => state.maxResolution)
  const imageScalingMode = useSettingsStore((state) => state.imageScalingMode)
  const autoDetectPixelArt = useSettingsStore((state) => state.autoDetectPixelArt)
  const displaySettingsKey = `${maxResolution}:${imageScalingMode}:${autoDetectPixelArt}`
  const prevDisplaySettingsKeyRef = useRef(displaySettingsKey)
  useEffect(() => {
    if (prevDisplaySettingsKeyRef.current === displaySettingsKey) return
    prevDisplaySettingsKeyRef.current = displaySettingsKey
    clearImageMetaCache()
    const st = useAppStore.getState()
    const current = st.dirImages.images[st.dirImages.current_index]
    if (st.archivePreviewPath) {
      void loadImage(st.archivePreviewPath, { refreshDirectory: false })
    } else if (st.archivePath && current) {
      void loadArchiveImageByIndex(st.archivePath, current)
    } else if (current) {
      void loadImage(current, { refreshDirectory: false })
    }
  }, [
    autoDetectPixelArt,
    clearImageMetaCache,
    displaySettingsKey,
    imageScalingMode,
    loadArchiveImageByIndex,
    loadImage,
    maxResolution
  ])

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
    if (selected) await loadImage(selected, { archiveOpen: "full" })
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
        if (isArchiveFilePath(raw)) {
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
      await loadImage(resolved[0], { archiveOpen: "full" })
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
    openArchiveFromPreview,
    handleOpenFile,
    handleDrop,
    handleDragOver,
    handleDragEnter,
    handleDragLeave,
    isDragOver,
    getOrLoadImage
  }
}
