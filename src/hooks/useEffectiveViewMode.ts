import { useMemo } from "react"

import { useAppStore } from "@/store/appStore"
import { useSettingsStore, type ViewMode } from "@/store/settingsStore"
import { isArchiveFilePath } from "@/utils/archiveFile"
import { archiveSoloIndices, type SoloIndices } from "@/utils/dirNavigation"

/** 폴더 목록에서 아카이브 파일(CBZ/ZIP)을 열어 첫 페이지만 미리 보는 중인지 여부. */
function isArchiveFolderPreview(state: ReturnType<typeof useAppStore.getState>): boolean {
  if (state.archivePath !== null) return false
  const current = state.dirImages.images[state.dirImages.current_index]
  return current !== undefined && isArchiveFilePath(current)
}

/**
 * 화면에 실제로 적용되는 보기 모드. 만화 자동 양쪽 보기가 정한 값이 있으면 그 값이다.
 * 폴더 안 아카이브 미리보기는 이웃 항목을 이미지로 디코드할 수 없으므로 단일 보기다.
 * 웹툰은 예외로, 연속 뷰가 아카이브 자리에 만화 열기 카드를 그린다.
 */
export function useEffectiveViewMode(): ViewMode {
  const viewMode = useSettingsStore((state) => state.viewMode)
  const comicViewMode = useAppStore((state) => state.comicViewMode)
  const inArchive = useAppStore((state) => state.archivePath !== null)
  const archivePreview = useAppStore(isArchiveFolderPreview)
  if (archivePreview && viewMode !== "webtoon") return "single"
  return inArchive && comicViewMode !== null ? comicViewMode : viewMode
}

/**
 * 이동·도크 하이라이트가 쓰는 화면 배치 모드. 화면 그리기용 모드와 달리 폴더
 * 아카이브 미리보기에서도 단일로 바꾸지 않는다. 양쪽 보기에서 아카이브는 단독
 * 화면으로 배치되므로, 거기서 넘겨도 짝 정렬이 유지된다.
 */
export function useLayoutViewMode(): ViewMode {
  const viewMode = useSettingsStore((state) => state.viewMode)
  const comicViewMode = useAppStore((state) => state.comicViewMode)
  const inArchive = useAppStore((state) => state.archivePath !== null)
  return inArchive && comicViewMode !== null ? comicViewMode : viewMode
}

/** 양쪽 보기에서 혼자 한 화면을 쓰는 폴더 아카이브 인덱스. */
export function useSoloIndices(): SoloIndices {
  const images = useAppStore((state) => state.dirImages.images)
  const inArchive = useAppStore((state) => state.archivePath !== null)
  return useMemo(() => archiveSoloIndices(images, inArchive), [images, inArchive])
}

/** `useLayoutViewMode`의 비구독 버전. 이벤트 시점 계산용이다. */
export function getLayoutViewMode(): ViewMode {
  const state = useAppStore.getState()
  const viewMode = useSettingsStore.getState().viewMode
  return state.archivePath !== null && state.comicViewMode !== null ? state.comicViewMode : viewMode
}

export function getEffectiveViewMode(): ViewMode {
  const state = useAppStore.getState()
  const viewMode = useSettingsStore.getState().viewMode
  if (isArchiveFolderPreview(state) && viewMode !== "webtoon") return "single"
  return state.archivePath !== null && state.comicViewMode !== null ? state.comicViewMode : viewMode
}
