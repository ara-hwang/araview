import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"

import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import { getSettings, updateSettings, type SettingsState } from "@/store/settingsStore"
import type { DirectoryImages } from "@/types"
import { errorMessage } from "@/utils/appError"

export type DirListOptionsPayload = {
  sortKey: SettingsState["sortKey"]
  descending: boolean
  shuffle: boolean
  recursive: boolean
}

type SortPatch = Pick<SettingsState, "sortKey" | "sortDescending" | "shuffle" | "includeSubfolders">

/** settingsStore → 백엔드 DirListOptions 페이로드 (camelCase) */
export function buildDirListOptions(settings: SortPatch): DirListOptionsPayload {
  return {
    sortKey: settings.sortKey,
    descending: settings.sortDescending,
    shuffle: settings.shuffle,
    recursive: settings.includeSubfolders
  }
}

/** 새로고침된 목록에서 이전에 보던 파일을 찾아 인덱스를 복원한다.
 *  파일이 사라졌으면 범위 안으로 clamp한다. */
export function resolveRefreshedIndex(
  prevImages: string[],
  prevIndex: number,
  nextImages: string[]
): number {
  if (nextImages.length === 0) return 0
  const current = prevImages[prevIndex]
  if (current) {
    const found = nextImages.indexOf(current)
    if (found >= 0) return found
  }
  return Math.max(0, Math.min(prevIndex, nextImages.length - 1))
}

/** 현재 폴더 목록을 최신 정렬 옵션으로 다시 읽음 (현재 이미지 위치 유지) */
export async function refreshDirectoryListing(): Promise<void> {
  const { imageInfo, dirImages: prevDir, archivePath } = useAppStore.getState()
  if (!imageInfo || archivePath) return
  // HEIC sidecar처럼 imageInfo.file_path가 원본 폴더가 아닐 수 있어
  // dirImages의 원본 경로를 우선 사용한다.
  const currentSource = prevDir.images[prevDir.current_index] ?? imageInfo.file_path
  try {
    const dirImages = await invoke<DirectoryImages>("get_directory_images", {
      filePath: currentSource,
      options: buildDirListOptions(getSettings())
    })
    const current_index = resolveRefreshedIndex(
      prevDir.images,
      prevDir.current_index,
      dirImages.images
    )
    useAppStore.setState({ dirImages: { ...dirImages, current_index } })
  } catch (e) {
    toast.error(i18n.t("toast.dir.refreshFail"), {
      description: errorMessage(e)
    })
  }
}

/** 정렬 설정 변경 + 목록 새로고침 (설정 패널/단축키 공용) */
export async function applySortSettings(patch: Partial<SortPatch>): Promise<void> {
  await updateSettings(patch)
  await refreshDirectoryListing()
}

/** S 키: 셔플 토글 + 목록 새로고침 */
export function toggleShuffleAndRefresh(): void {
  void applySortSettings({ shuffle: !getSettings().shuffle })
}
