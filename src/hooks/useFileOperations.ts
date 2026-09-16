import { useNavigate } from "@tanstack/react-router"
import { invoke } from "@tauri-apps/api/core"
import { confirm } from "@tauri-apps/plugin-dialog"
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener"
import { useCallback } from "react"
import { toast } from "sonner"

import i18n from "@/i18n"
import { closeImage, useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useSettingsStore } from "@/store/settingsStore"
import type { ImageInfo } from "@/types"
import { errorMessage } from "@/utils/appError"
import type { SaveEditsPayload } from "@/utils/imageEdits"

type LoadImageFn = (filePath: string, options?: { refreshDirectory?: boolean }) => Promise<void>

/** 아카이브 모드면 원본 아카이브 경로, 아니면 현재 이미지 경로 */
export function getEffectivePath(
  imagePath: string | null,
  archivePath: string | null
): string | null {
  if (archivePath) return archivePath
  return imagePath
}

/** PSD 미리보기 MIME. `file_path`는 JPEG sidecar일 수 있어 판정은 MIME/파일명 기준. */
export const PSD_MIME_TYPE = "image/vnd.adobe.photoshop"

/** SVG MIME. 래스터 편집 파이프라인으로 저장할 수 없어 진입 차단된다. */
export const SVG_MIME_TYPE = "image/svg+xml"

/** AVIF MIME. 백엔드 디코더가 없어(보기만 WebView2 네이티브) 진입 차단된다. */
export const AVIF_MIME_TYPE = "image/avif"

/** 읽기 전용 PSD 미리보기인지. 저장은 진입 차단된다. */
export function isPsdImage(imageInfo: Pick<ImageInfo, "mime_type" | "file_name"> | null): boolean {
  if (!imageInfo) return false
  if (imageInfo.mime_type === PSD_MIME_TYPE) return true
  return imageInfo.file_name.toLowerCase().endsWith(".psd")
}

/** 저장 불가 SVG인지. 저장은 진입 차단된다. */
export function isSvgImage(imageInfo: Pick<ImageInfo, "mime_type" | "file_name"> | null): boolean {
  if (!imageInfo) return false
  if (imageInfo.mime_type === SVG_MIME_TYPE) return true
  return imageInfo.file_name.toLowerCase().endsWith(".svg")
}

/** 저장 불가 AVIF인지. 저장은 진입 차단된다. */
export function isAvifImage(imageInfo: Pick<ImageInfo, "mime_type" | "file_name"> | null): boolean {
  if (!imageInfo) return false
  if (imageInfo.mime_type === AVIF_MIME_TYPE) return true
  return imageInfo.file_name.toLowerCase().endsWith(".avif")
}

export type SaveBlockedReason = "noPsd" | "noSvg" | "noAvif"

/** 저장 진입 차단 사유. null이면 저장 가능. */
export function saveBlockedReason(
  imageInfo: Pick<ImageInfo, "mime_type" | "file_name"> | null
): SaveBlockedReason | null {
  if (isPsdImage(imageInfo)) return "noPsd"
  if (isSvgImage(imageInfo)) return "noSvg"
  if (isAvifImage(imageInfo)) return "noAvif"
  return null
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

export function useFileOperations({ loadImage }: { loadImage: LoadImageFn }) {
  const navigate = useNavigate()

  const revealCurrent = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error(i18n.t("toast.reveal.empty"))
      return
    }
    try {
      await revealItemInDir(target)
    } catch (e) {
      toast.error(i18n.t("toast.reveal.fail"), { description: errorMessage(e) })
    }
  }, [])

  const openExternal = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error(i18n.t("toast.external.empty"))
      return
    }
    try {
      await openPath(target)
    } catch (e) {
      toast.error(i18n.t("toast.external.fail"), {
        description: errorMessage(e)
      })
    }
  }, [])

  const trashCurrent = useCallback(async () => {
    const { imageInfo, dirImages, archivePath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.trash.empty"))
      return
    }
    if (archivePath) {
      toast.info(i18n.t("toast.trash.noArchive"))
      return
    }

    const ok = await confirm(i18n.t("confirm.trash.message", { name: imageInfo.file_name }), {
      title: i18n.t("confirm.trash.title"),
      kind: "warning"
    }).catch(() => false)
    if (!ok) return

    try {
      await invoke("trash_file", { filePath: imageInfo.file_path })
    } catch (e) {
      toast.error(i18n.t("toast.trash.fail"), { description: errorMessage(e) })
      return
    }

    void useRecentFilesStore.getState().remove(imageInfo.file_path)

    const next = getNextPathAfterTrash(
      dirImages.images,
      dirImages.current_index,
      imageInfo.file_path
    )

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

  const copyPathCurrent = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error(i18n.t("toast.path.empty"))
      return
    }
    try {
      await navigator.clipboard.writeText(target)
      toast.success(i18n.t("toast.path.done"), {
        description: target,
        duration: 1500
      })
    } catch (e) {
      toast.error(i18n.t("toast.path.fail"), { description: errorMessage(e) })
    }
  }, [])

  /** 이름 변경. 성공 시 true (다이얼로그를 닫아도 됨) */
  const renameCurrent = useCallback(async (newName: string) => {
    const { imageInfo, dirImages, archivePath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.rename.empty"))
      return false
    }
    if (archivePath) {
      toast.info(i18n.t("toast.rename.noArchive"))
      return false
    }
    if (!newName.trim()) {
      toast.error(i18n.t("toast.rename.blank"))
      return false
    }

    let nextInfo: ImageInfo
    try {
      nextInfo = await invoke<ImageInfo>("rename_file", {
        oldPath: imageInfo.file_path,
        newName: newName.trim()
      })
    } catch (e) {
      toast.error(i18n.t("toast.rename.fail"), { description: errorMessage(e) })
      return false
    }

    useAppStore.setState({
      imageInfo: nextInfo,
      dirImages: {
        ...dirImages,
        images: replacePathInList(dirImages.images, imageInfo.file_path, nextInfo.file_path)
      },
      error: null
    })
    const recent = useRecentFilesStore.getState()
    void recent.remove(imageInfo.file_path).then(() => {
      if (useSettingsStore.getState().recordRecentFiles) {
        return recent.add(nextInfo.file_path)
      }
    })

    toast.success(i18n.t("toast.rename.done"), {
      description: nextInfo.file_name,
      duration: 2000
    })
    return true
  }, [])

  /** 회전/반전 + 포맷 변환 저장. 성공 시 true (다이얼로그를 닫아도 됨) */
  const saveEdits = useCallback(
    async (payload: SaveEditsPayload) => {
      const { imageInfo, archivePath } = useAppStore.getState()
      if (!imageInfo) {
        toast.error(i18n.t("toast.save.empty"))
        return false
      }
      if (archivePath) {
        toast.info(i18n.t("toast.save.noArchive"))
        return false
      }
      const blocked = saveBlockedReason(imageInfo)
      if (blocked) {
        toast.info(i18n.t(`toast.save.${blocked}`))
        return false
      }
      const noChange =
        payload.rotationCw % 360 === 0 &&
        !payload.flipH &&
        !payload.flipV &&
        payload.format === null
      if (noChange) {
        toast.info(i18n.t("toast.save.noChange"))
        return false
      }

      if (payload.overwrite) {
        const ok = await confirm(
          i18n.t("confirm.overwrite.message", {
            name: imageInfo.file_name
          }),
          { title: i18n.t("confirm.overwrite.title"), kind: "warning" }
        ).catch(() => false)
        if (!ok) return false
      }

      let nextInfo: ImageInfo
      try {
        nextInfo = await invoke<ImageInfo>("save_image_edits", {
          filePath: imageInfo.file_path,
          options: payload
        })
      } catch (e) {
        toast.error(i18n.t("toast.save.fail"), { description: errorMessage(e) })
        return false
      }

      if (useSettingsStore.getState().recordRecentFiles) {
        void useRecentFilesStore.getState().add(nextInfo.file_path)
      }
      toast.success(i18n.t("toast.save.done"), {
        description: nextInfo.file_name,
        duration: 2000
      })
      // 재로드하면 목록·인덱스 갱신 + 회전 상태 초기화(resetView)까지 처리
      await loadImage(nextInfo.file_path, { refreshDirectory: true })
      return true
    },
    [loadImage]
  )

  return {
    revealCurrent,
    openExternal,
    trashCurrent,
    copyPathCurrent,
    renameCurrent,
    saveEdits
  }
}
