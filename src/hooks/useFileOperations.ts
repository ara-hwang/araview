import { useNavigate } from "@tanstack/react-router"
import { invoke } from "@tauri-apps/api/core"
import { confirm } from "@tauri-apps/plugin-dialog"
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener"
import { useCallback } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { closeImage, useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"
import { maxSideForResolution } from "@/utils/resolutionLimit"

type LoadImageFn = (filePath: string, options?: { refreshDirectory?: boolean }) => Promise<void>

/** 아카이브 모드(전체/미리보기)면 원본 아카이브 경로, 아니면 현재 이미지 원본 경로 */
export function getEffectivePath(
  sourcePath: string | null,
  archivePath: string | null
): string | null {
  if (archivePath) return archivePath
  return sourcePath
}

/** 휴지통 이동 후 보여줄 다음 경로. 비어있으면 null (홈으로 귀환) */
export function getNextPathAfterTrash(
  images: string[],
  currentIndex: number,
  trashedPath: string
): string | null {
  const remaining = images.filter((p) => p !== trashedPath)
  if (remaining.length === 0) return null
  const nextIndex = Math.min(currentIndex, remaining.length - 1)
  return remaining[nextIndex] ?? null
}

/** 이름 변경 후 디렉토리 목록에서 구 경로를 신 경로로 교체 */
export function replacePathInList(images: string[], oldPath: string, newPath: string): string[] {
  return images.map((p) => (p === oldPath ? newPath : p))
}

/**
 * 현재 대상 경로(아카이브면 아카이브 원본)로 작업을 실행한다.
 * 대상이 없거나 작업이 실패하면 `toast.<key>.empty` / `toast.<key>.fail`로 알린다.
 */
async function withEffectiveTarget(
  key: "reveal" | "external" | "path",
  action: (target: string) => Promise<void>
) {
  const { imageInfo, archivePath, archivePreviewPath } = useAppStore.getState()
  const target = getEffectivePath(imageInfo?.source_path ?? null, archivePath ?? archivePreviewPath)
  if (!target) {
    toast.error(i18n.t(`toast.${key}.empty`))
    return
  }
  try {
    await action(target)
  } catch (e) {
    toast.error(i18n.t(`toast.${key}.fail`), {
      description: errorMessage(e),
      details: errorCopyDetails(e, target)
    })
  }
}

export function useFileOperations({ loadImage }: { loadImage: LoadImageFn }) {
  const navigate = useNavigate()

  const revealCurrent = useCallback(
    () => withEffectiveTarget("reveal", (target) => revealItemInDir(target)),
    []
  )

  const openExternal = useCallback(
    () => withEffectiveTarget("external", (target) => openPath(target)),
    []
  )

  const trashCurrent = useCallback(async () => {
    const { imageInfo, dirImages, archivePath, archivePreviewPath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.trash.empty"))
      return
    }
    if (archivePath || archivePreviewPath) {
      toast.info(i18n.t("toast.trash.noArchive"))
      return
    }

    const ok = await confirm(i18n.t("confirm.trash.message", { name: imageInfo.file_name }), {
      title: i18n.t("confirm.trash.title"),
      kind: "warning"
    }).catch(() => false)
    if (!ok) return

    const sourcePath = imageInfo.source_path
    try {
      await invoke("trash_file", { filePath: sourcePath })
    } catch (e) {
      toast.error(i18n.t("toast.trash.fail"), {
        description: errorMessage(e),
        details: errorCopyDetails(e, sourcePath)
      })
      return
    }

    void useRecentFilesStore.getState().remove(sourcePath)

    const next = getNextPathAfterTrash(dirImages.images, dirImages.current_index, sourcePath)

    if (!next) {
      closeImage()
      toast.success(i18n.t("toast.trash.done"))
      void navigate({ to: "/" })
      return
    }

    toast.success(i18n.t("toast.trash.done"), {
      description: imageInfo.file_name,
      duration: 2000
    })
    await loadImage(next, { refreshDirectory: true })
  }, [loadImage, navigate])

  const copyPathCurrent = useCallback(
    () =>
      withEffectiveTarget("path", async (target) => {
        await navigator.clipboard.writeText(target)
        toast.success(i18n.t("toast.path.done"), {
          description: target,
          duration: 1500
        })
      }),
    []
  )

  /** 이름 변경. 성공 시 true (다이얼로그를 닫아도 됨) */
  const renameCurrent = useCallback(async (newName: string) => {
    const { imageInfo, dirImages, archivePath, archivePreviewPath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.rename.empty"))
      return false
    }
    if (archivePath || archivePreviewPath) {
      toast.info(i18n.t("toast.rename.noArchive"))
      return false
    }
    if (!newName.trim()) {
      toast.error(i18n.t("toast.rename.blank"))
      return false
    }

    const sourcePath = imageInfo.source_path
    const settings = useSettingsStore.getState()
    let nextInfo: ImageInfo
    try {
      nextInfo = await invoke<ImageInfo>("rename_file", {
        oldPath: sourcePath,
        newName: newName.trim(),
        maxSide: maxSideForResolution(settings.maxResolution),
        imageScalingMode: settings.imageScalingMode,
        autoDetectPixelArt: settings.autoDetectPixelArt
      })
    } catch (e) {
      toast.error(i18n.t("toast.rename.fail"), {
        description: errorMessage(e),
        details: errorCopyDetails(e, sourcePath)
      })
      return false
    }

    useAppStore.setState({
      imageInfo: nextInfo,
      dirImages: {
        ...dirImages,
        images: replacePathInList(dirImages.images, sourcePath, nextInfo.source_path)
      },
      error: null,
      errorCode: null
    })
    const recent = useRecentFilesStore.getState()
    void recent.remove(sourcePath).then(() => {
      if (useSettingsStore.getState().recordRecentFiles) {
        return recent.add(nextInfo.source_path)
      }
    })

    toast.success(i18n.t("toast.rename.done"), {
      description: nextInfo.file_name,
      duration: 2000
    })
    return true
  }, [])

  return {
    revealCurrent,
    openExternal,
    trashCurrent,
    copyPathCurrent,
    renameCurrent
  }
}
