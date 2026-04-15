import { useCallback } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { toast } from "sonner"
import { useAppStore } from "@/store/appStore"

// 현재 이미지를 PNG로 변환해 시스템 클립보드에 복사한다.
// Asset Protocol을 통해 파일을 fetch → Blob → Canvas → PNG Blob → ClipboardItem.

export function useCopyImage() {
  const copy = useCallback(async () => {
    const { imageInfo } = useAppStore.getState()
    if (!imageInfo) {
      toast.error("복사할 이미지가 없습니다")
      return
    }

    try {
      const url = convertFileSrc(imageInfo.file_path)
      const response = await fetch(url)
      const blob = await response.blob()

      // ClipboardItem은 PNG만 안정적으로 지원 → Canvas로 재인코딩
      const bitmap = await createImageBitmap(blob)
      const canvas = document.createElement("canvas")
      canvas.width = bitmap.width
      canvas.height = bitmap.height
      const ctx = canvas.getContext("2d")
      if (!ctx) throw new Error("Canvas 2D context 생성 실패")
      ctx.drawImage(bitmap, 0, 0)
      bitmap.close()

      const pngBlob = await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob(
          (b) => (b ? resolve(b) : reject(new Error("PNG 변환 실패"))),
          "image/png"
        )
      })

      await navigator.clipboard.write([
        new ClipboardItem({ "image/png": pngBlob })
      ])
      toast.success("이미지 복사 완료", {
        description: imageInfo.file_name,
        duration: 1500
      })
    } catch (e) {
      toast.error("이미지 복사 실패", { description: String(e) })
    }
  }, [])

  return { copy }
}
