import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import { toast } from "@/components/ui/toast"
import { getCoverLayout } from "@/hooks/useCoverLayout"
import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import type { ComicInfo } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"
import { coverPagesOf, toggleCoverPage } from "@/utils/comicCover"

/** 아카이브를 통째로 다시 쓰므로 한 번에 하나만 저장한다. */
let saving = false

/** 열린 아카이브에서 `index` 페이지가 지금 단독 표지로 보이는지. */
export function isCoverPage(index: number): boolean {
  if (useAppStore.getState().archivePath === null) return false
  return coverPagesOf(getCoverLayout()).includes(index)
}

/**
 * 열린 아카이브의 표지 지정(ComicInfo `FrontCover`)을 바꾼다. 원본 아카이브의
 * `ComicInfo.xml`을 고쳐 쓰고, 성공하면 `onChanged`로 그 페이지에 다시 맞춘다
 * (양쪽 보기의 쌍 배치가 바뀌므로).
 */
export function useComicCoverEdit(onChanged: (index: number) => void) {
  const toggleCover = useCallback(
    async (index: number) => {
      const { archivePath, dirImages } = useAppStore.getState()
      if (archivePath === null || saving) return
      if (index < 0 || index >= dirImages.images.length) return

      const covers = coverPagesOf(getCoverLayout())
      const wasCover = covers.includes(index)
      saving = true
      try {
        const info = await invoke<ComicInfo>("set_comic_cover_pages", {
          filePath: archivePath,
          coverPages: toggleCoverPage(covers, index)
        })
        // 저장하는 동안 다른 파일로 넘어갔으면 그 파일의 메타데이터를 덮지 않는다.
        if (useAppStore.getState().archivePath !== archivePath) return
        useAppStore.setState({ comicInfo: info, comicInfoError: null })
        toast.success(i18n.t(wasCover ? "toast.cover.unset" : "toast.cover.set"))
        onChanged(index)
      } catch (e) {
        toast.error(i18n.t("toast.cover.fail"), {
          description: errorMessage(e),
          details: errorCopyDetails(e, archivePath)
        })
      } finally {
        saving = false
      }
    },
    [onChanged]
  )

  return { toggleCover }
}
