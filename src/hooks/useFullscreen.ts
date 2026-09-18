import { getCurrentWindow } from "@tauri-apps/api/window"
import { useCallback } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

// F11: Tauri 창의 전체화면 토글. 실패 시 토스트로 알림.
export function useFullscreen() {
  const toggle = useCallback(async () => {
    try {
      const win = getCurrentWindow()
      const isFs = await win.isFullscreen()
      await win.setFullscreen(!isFs)
    } catch (e) {
      toast.error(i18n.t("toast.fullscreen.fail"), {
        description: errorMessage(e),
        details: errorCopyDetails(e)
      })
    }
  }, [])

  return { toggle }
}
