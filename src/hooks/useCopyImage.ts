import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import type { ClipboardPng } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

// 클립보드 복사는 백엔드와 협력한다(SPEC §11.3).
// 백엔드가 렌더 바이트를 디코드해 PNG로 인코딩하고(export_clipboard_png),
// 프론트는 그 경로를 시스템 클립보드에 쓴다. 예전 웹뷰 캔버스 경로(풀사이즈
// 비트맵 + 캔버스 + PNG 블롭을 동시에 유지)와 달리 웹뷰에 픽셀 데이터를 올리지 않는다.

export function useCopyImage() {
  const copy = useCallback(async () => {
    const { imageInfo } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.copy.empty"))
      return
    }

    try {
      const png = await invoke<ClipboardPng>("export_clipboard_png", {
        filePath: imageInfo.file_path
      })
      const response = await fetch(convertFileSrc(png.file_path))
      if (!response.ok) {
        throw new Error(`Clipboard PNG fetch failed: ${response.status}`)
      }
      const blob = await response.blob()

      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })])
      toast.success(i18n.t("toast.copy.done"), {
        description: imageInfo.file_name,
        duration: 1500
      })
    } catch (e) {
      toast.error(i18n.t("toast.copy.fail"), {
        description: errorMessage(e),
        details: errorCopyDetails(e, imageInfo.file_path)
      })
    }
  }, [])

  return { copy }
}
