import { listen } from "@tauri-apps/api/event"
import { useEffect, useState } from "react"
import { detach } from "tauri-plugin-snap-layout"

/**
 * Windows 11 Snap Layouts를 커스텀 최대화 버튼에 연결한다.
 *
 * `decorations: false` 창은 WebView2가 마우스를 가로채 Tauri 창이
 * `WM_NCHITTEST`를 받지 못해 OS가 Snap Layouts 플라이아웃을 띄우지 못한다.
 * `tauri-plugin-snap-layout`이 최대화 버튼 위에 `HTMAXBUTTON`을 반환하는
 * 투명 네이티브 오버레이를 띄워 이를 해결한다 (Rust 쪽 button_id와 일치).
 *
 * 오버레이가 버튼 위의 마우스를 가로채므로 CSS `:hover`와 React 이벤트가
 * 발생하지 않는다. 플러그인이 내보내는 `tauri-snap://snap/mouseenter|mouseleave`
 * 이벤트로 `snapHover`를 제공하니 이걸로 hover 스타일을 미러링한다.
 * 캡션 버튼에는 OS 네이티브와 동일하게 툴팁을 붙이지 않는다.
 * 클릭(최대화/복원)은 네이티브가 처리하며, 키보드는 버튼의 onClick이 그대로 담당한다.
 *
 * 비-Tauri 환경(jsdom/테스트)에서는 모든 호출이 조용히 무시된다.
 *
 * @param enabled 오버레이 유지 여부. 헤더가 완전히 가려지는(auto-hide,
 *   메뉴바 숨김) 동안 false로 바꿔 네이티브 오버레이를 떼어낸다. opacity-0로만
 *   숨기면 버튼 rect가 유효해 플러그인이 오버레이를 유지하므로 명시적 detach가 필요하다.
 */
export function useSnapLayout(enabled: boolean): { snapHover: boolean } {
  const [snapHover, setSnapHover] = useState(false)

  useEffect(() => {
    if (!enabled) {
      setSnapHover(false)
      return
    }

    let cancelled = false

    // 주입 스크립트는 초기화돼야 window.__SNAP_LAYOUT_ATTACH__를 정의한다.
    // attach()는 정의 전에 호출되면 무음 no-op이므로, 헤더가 늦게 마운트되는
    // 경우(HMR, 0fr 그리드 트랙 등)를 대비해 정의를 잠깐 기다린 뒤 붙인다.
    const attachWhenReady = (attempt: number): void => {
      if (cancelled) return
      const globalAttach = (
        window as {
          __SNAP_LAYOUT_ATTACH__?: (id?: string) => void
        }
      ).__SNAP_LAYOUT_ATTACH__
      if (globalAttach) {
        globalAttach("caption-maximize")
        return
      }
      if (attempt <= 0) return
      setTimeout(() => attachWhenReady(attempt - 1), 50)
    }

    const enterUnlisten = listen("tauri-snap://snap/mouseenter", () => {
      setSnapHover(true)
    })
    const leaveUnlisten = listen("tauri-snap://snap/mouseleave", () => {
      setSnapHover(false)
    })

    attachWhenReady(20)

    return () => {
      cancelled = true
      for (const promise of [enterUnlisten, leaveUnlisten]) {
        void promise.then((fn) => fn()).catch(() => {})
      }
      // 언마운트/비활성화 시 네이티브 오버레이를 떼어낸다.
      void detach().catch(() => {})
    }
  }, [enabled])

  return { snapHover }
}
