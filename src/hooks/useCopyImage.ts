import { convertFileSrc } from "@tauri-apps/api/core"
import { useCallback } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

// 현재 이미지를 PNG로 변환해 시스템 클립보드에 복사한다.
// Asset Protocol을 통해 파일을 fetch → Blob → Canvas → PNG Blob → ClipboardItem.

export function useCopyImage() {
  const copy = useCallback(async () => {
    const { imageInfo } = useAppStore.getState()
    if (!imageInfo) {
      toast.error(i18n.t("toast.copy.empty"))
      return
    }

    try {
      const url = convertFileSrc(imageInfo.file_path)
      const response = await fetch(url)
      const blob = await response.blob()

      // ClipboardItem은 PNG만 안정적으로 지원 → Canvas로 재인코딩.
      // JPEG EXIF orientation은 브라우저 기본값과 동일하게 명시해 회전 누락을 막는다.
      const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" })
      const canvas = document.createElement("canvas")
      canvas.width = bitmap.width
      canvas.height = bitmap.height
      const ctx = canvas.getContext("2d")
      if (!ctx) throw new Error(i18n.t("toast.copy.canvasFail"))
      ctx.drawImage(bitmap, 0, 0)
      bitmap.close()

      const pngBlob = await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob(
          (b) => (b ? resolve(b) : reject(new Error(i18n.t("toast.copy.pngFail")))),
          "image/png"
        )
      })

      await navigator.clipboard.write([new ClipboardItem({ "image/png": pngBlob })])
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
