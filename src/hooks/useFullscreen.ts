import { useCallback } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { toast } from "sonner"

// F11: Tauri 창의 전체화면 토글. 실패 시 토스트로 알림.
export function useFullscreen() {
  const toggle = useCallback(async () => {
    try {
      const win = getCurrentWindow()
      const isFs = await win.isFullscreen()
      await win.setFullscreen(!isFs)
    } catch (e) {
      toast.error("전체화면 전환 실패", { description: String(e) })
    }
  }, [])

  return { toggle }
}
