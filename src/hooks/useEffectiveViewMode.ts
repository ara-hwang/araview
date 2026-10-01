import { useAppStore } from "@/store/appStore"
import { useSettingsStore, type ViewMode } from "@/store/settingsStore"

/** 화면에 실제로 적용되는 보기 모드. 만화 자동 양쪽 보기가 정한 값이 있으면 그 값이다. */
export function useEffectiveViewMode(): ViewMode {
  const viewMode = useSettingsStore((state) => state.viewMode)
  const comicViewMode = useAppStore((state) => state.comicViewMode)
  const inArchive = useAppStore((state) => state.archivePath !== null)
  return inArchive && comicViewMode !== null ? comicViewMode : viewMode
}

export function getEffectiveViewMode(): ViewMode {
  const { comicViewMode, archivePath } = useAppStore.getState()
  return archivePath !== null && comicViewMode !== null
    ? comicViewMode
    : useSettingsStore.getState().viewMode
}
