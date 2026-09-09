import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import { useAppStore } from "@/store/appStore"
import {
  getSettings,
  updateSettings,
  type SettingsState
} from "@/store/settingsStore"
import type { DirectoryImages } from "@/types"

export type DirListOptionsPayload = {
  sortKey: SettingsState["sortKey"]
  descending: boolean
  shuffle: boolean
  recursive: boolean
}

type SortPatch = Pick<
  SettingsState,
  "sortKey" | "sortDescending" | "shuffle" | "includeSubfolders"
>

/** settingsStore → 백엔드 DirListOptions 페이로드 (camelCase) */
export function buildDirListOptions(
  settings: SortPatch
): DirListOptionsPayload {
  return {
    sortKey: settings.sortKey,
    descending: settings.sortDescending,
    shuffle: settings.shuffle,
    recursive: settings.includeSubfolders
  }
}

/** 현재 폴더 목록을 최신 정렬 옵션으로 다시 읽음 (현재 이미지 위치 유지) */
export async function refreshDirectoryListing(): Promise<void> {
  const { imageInfo, archivePath } = useAppStore.getState()
  if (!imageInfo || archivePath) return
  try {
    const dirImages = await invoke<DirectoryImages>("get_directory_images", {
      filePath: imageInfo.file_path,
      options: buildDirListOptions(getSettings())
    })
    useAppStore.setState({ dirImages })
  } catch (e) {
    toast.error("목록 새로고침 실패", { description: String(e) })
  }
}

/** 정렬 설정 변경 + 목록 새로고침 (설정 패널/단축키 공용) */
export async function applySortSettings(
  patch: Partial<SortPatch>
): Promise<void> {
  await updateSettings(patch)
  await refreshDirectoryListing()
}

/** S 키: 셔플 토글 + 목록 새로고침 */
export function toggleShuffleAndRefresh(): void {
  void applySortSettings({ shuffle: !getSettings().shuffle })
}
