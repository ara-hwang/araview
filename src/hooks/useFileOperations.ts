import { useCallback } from "react"
import { useNavigate } from "@tanstack/react-router"
import { invoke } from "@tauri-apps/api/core"
import { confirm } from "@tauri-apps/plugin-dialog"
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener"
import { toast } from "sonner"
import { useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useFavoritesStore } from "@/store/favoritesStore"
import type { ImageInfo } from "@/types"
import type { SaveEditsPayload } from "@/utils/imageEdits"

type LoadImageFn = (
  filePath: string,
  options?: { refreshDirectory?: boolean }
) => Promise<void>

/** 아카이브 모드면 원본 아카이브 경로, 아니면 현재 이미지 경로 */
export function getEffectivePath(
  imagePath: string | null,
  archivePath: string | null
): string | null {
  if (archivePath) return archivePath
  return imagePath
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
export function replacePathInList(
  images: string[],
  oldPath: string,
  newPath: string
): string[] {
  return images.map((p) => (p === oldPath ? newPath : p))
}

export function useFileOperations({ loadImage }: { loadImage: LoadImageFn }) {
  const navigate = useNavigate()

  const revealCurrent = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error("표시할 파일이 없습니다")
      return
    }
    try {
      await revealItemInDir(target)
    } catch (e) {
      toast.error("탐색기에서 보기 실패", { description: String(e) })
    }
  }, [])

  const openExternal = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error("열 파일이 없습니다")
      return
    }
    try {
      await openPath(target)
    } catch (e) {
      toast.error("외부 앱으로 열기 실패", { description: String(e) })
    }
  }, [])

  const trashCurrent = useCallback(async () => {
    const { imageInfo, dirImages, archivePath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error("삭제할 이미지가 없습니다")
      return
    }
    if (archivePath) {
      toast.info("아카이브 모드에서는 삭제할 수 없습니다")
      return
    }

    const ok = await confirm(
      `${imageInfo.file_name}을(를) 휴지통으로 이동할까요?`,
      { title: "파일 삭제", kind: "warning" }
    ).catch(() => false)
    if (!ok) return

    try {
      await invoke("trash_file", { filePath: imageInfo.file_path })
    } catch (e) {
      toast.error("휴지통 이동 실패", { description: String(e) })
      return
    }

    void useRecentFilesStore.getState().remove(imageInfo.file_path)
    void useFavoritesStore.getState().remove(imageInfo.file_path)

    const next = getNextPathAfterTrash(
      dirImages.images,
      dirImages.current_index,
      imageInfo.file_path
    )

    if (!next) {
      useAppStore.setState({
        imageInfo: null,
        dirImages: { images: [], current_index: 0 },
        error: null
      })
      toast.success("휴지통으로 이동했습니다")
      void navigate({ to: "/" })
      return
    }

    toast.success("휴지통으로 이동했습니다", {
      description: imageInfo.file_name,
      duration: 2000
    })
    await loadImage(next, { refreshDirectory: true })
  }, [loadImage, navigate])

  const copyPathCurrent = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error("복사할 경로가 없습니다")
      return
    }
    try {
      await navigator.clipboard.writeText(target)
      toast.success("경로 복사 완료", { description: target, duration: 1500 })
    } catch (e) {
      toast.error("경로 복사 실패", { description: String(e) })
    }
  }, [])

  /** 이름 변경. 성공 시 true (다이얼로그를 닫아도 됨) */
  const renameCurrent = useCallback(async (newName: string) => {
    const { imageInfo, dirImages, archivePath } = useAppStore.getState()
    if (!imageInfo) {
      toast.error("이름을 바꿀 이미지가 없습니다")
      return false
    }
    if (archivePath) {
      toast.info("아카이브 모드에서는 이름을 바꿀 수 없습니다")
      return false
    }
    if (!newName.trim()) {
      toast.error("파일 이름을 입력하세요")
      return false
    }

    let nextInfo: ImageInfo
    try {
      nextInfo = await invoke<ImageInfo>("rename_file", {
        oldPath: imageInfo.file_path,
        newName: newName.trim()
      })
    } catch (e) {
      toast.error("이름 변경 실패", { description: String(e) })
      return false
    }

    useAppStore.setState({
      imageInfo: nextInfo,
      dirImages: {
        ...dirImages,
        images: replacePathInList(
          dirImages.images,
          imageInfo.file_path,
          nextInfo.file_path
        )
      },
      error: null
    })
    const recent = useRecentFilesStore.getState()
    void recent
      .remove(imageInfo.file_path)
      .then(() => recent.add(nextInfo.file_path))
    void useFavoritesStore
      .getState()
      .replace(imageInfo.file_path, nextInfo.file_path)

    toast.success("이름 변경 완료", {
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
        toast.error("저장할 이미지가 없습니다")
        return false
      }
      if (archivePath) {
        toast.info("아카이브 모드에서는 저장할 수 없습니다")
        return false
      }
      const noChange =
        payload.rotationCw % 360 === 0 &&
        !payload.flipH &&
        !payload.flipV &&
        payload.format === null
      if (noChange) {
        toast.info("적용할 변경 사항이 없습니다")
        return false
      }

      if (payload.overwrite) {
        const ok = await confirm(
          `${imageInfo.file_name}에 변경 사항을 덮어씁니다. 계속할까요?`,
          { title: "덮어쓰기 저장", kind: "warning" }
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
        toast.error("저장 실패", { description: String(e) })
        return false
      }

      void useRecentFilesStore.getState().add(nextInfo.file_path)
      toast.success("저장 완료", {
        description: nextInfo.file_name,
        duration: 2000
      })
      // 재로드하면 목록·인덱스 갱신 + 회전 상태 초기화(resetView)까지 처리
      await loadImage(nextInfo.file_path, { refreshDirectory: true })
      return true
    },
    [loadImage]
  )

  /** 현재 파일(아카이브면 원본) 즐겨찾기 토글 */
  const toggleFavoriteCurrent = useCallback(async () => {
    const { imageInfo, archivePath } = useAppStore.getState()
    const target = getEffectivePath(imageInfo?.file_path ?? null, archivePath)
    if (!target) {
      toast.error("즐겨찾기할 파일이 없습니다")
      return
    }
    const added = await useFavoritesStore.getState().toggle(target)
    toast.success(
      added ? "즐겨찾기에 추가했습니다" : "즐겨찾기에서 제거했습니다",
      {
        duration: 1500
      }
    )
  }, [])

  return {
    revealCurrent,
    openExternal,
    trashCurrent,
    copyPathCurrent,
    renameCurrent,
    saveEdits,
    toggleFavoriteCurrent
  }
}
