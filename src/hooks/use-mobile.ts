import * as React from "react"

// Tailwind 브레이크포인트와 맞춘 모바일 판단 기준(px)
const MOBILE_BREAKPOINT = 768

// 뷰포트 너비를 기준으로 모바일 여부를 실시간으로 반환하는 훅
export function useIsMobile() {
  const [isMobile, setIsMobile] = React.useState<boolean | undefined>(undefined)

  React.useEffect(() => {
    const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`)
    const onChange = () => {
      setIsMobile(window.innerWidth < MOBILE_BREAKPOINT)
    }
    mql.addEventListener("change", onChange)
    setIsMobile(window.innerWidth < MOBILE_BREAKPOINT)
    return () => mql.removeEventListener("change", onChange)
  }, [])

  return !!isMobile
}
