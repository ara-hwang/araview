import { useCallback, useEffect, useState } from "react"
import { toast } from "sonner"
import { useSettingsStore } from "@/store/settingsStore"

// 슬라이드쇼: 설정된 간격마다 onNavigateNext를 호출한다.
// Space / F5 단축키로 토글되며, 종료 시 토스트로 알린다.

export function useSlideshow(onNavigateNext: () => void) {
  const intervalMs = useSettingsStore((s) => s.slideshowIntervalMs)
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (!active) return
    const id = window.setInterval(() => {
      onNavigateNext()
    }, intervalMs)
    return () => {
      window.clearInterval(id)
    }
  }, [active, intervalMs, onNavigateNext])

  const toggle = useCallback(() => {
    setActive((prev) => {
      const next = !prev
      toast.info(next ? "슬라이드쇼 시작" : "슬라이드쇼 정지", {
        duration: 1500
      })
      return next
    })
  }, [])

  const stop = useCallback(() => setActive(false), [])

  return { active, toggle, stop }
}
