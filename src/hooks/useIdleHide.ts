import { useEffect, useState } from "react"

const IDLE_DELAY_MS = 3000

/** 입력이 일정 시간 없으면 true (읽기 중 UI 자동 숨김) */
export function useIdleHide(enabled: boolean, delayMs = IDLE_DELAY_MS) {
  const [idle, setIdle] = useState(false)

  useEffect(() => {
    if (!enabled) {
      setIdle(false)
      return
    }

    let timer: ReturnType<typeof setTimeout> | null = null

    const wake = () => {
      setIdle(false)
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => setIdle(true), delayMs)
    }

    wake()
    window.addEventListener("mousemove", wake)
    window.addEventListener("mousedown", wake)
    window.addEventListener("keydown", wake)
    window.addEventListener("wheel", wake)

    return () => {
      if (timer) clearTimeout(timer)
      window.removeEventListener("mousemove", wake)
      window.removeEventListener("mousedown", wake)
      window.removeEventListener("keydown", wake)
      window.removeEventListener("wheel", wake)
    }
  }, [enabled, delayMs])

  return idle
}
