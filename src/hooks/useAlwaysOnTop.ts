import { getCurrentWindow } from "@tauri-apps/api/window"
import { useCallback, useEffect } from "react"
import { toast } from "sonner"

import i18n from "@/i18n"
import { getSettings, updateSettings, useSettingsStore } from "@/store/settingsStore"
import { errorMessage } from "@/utils/appError"

// 창 항상 위 토글. settingsStore.alwaysOnTop이 단일 소스이며
// 변경될 때마다 Tauri 창에 반영하고 settings.json에 유지한다.
export function useAlwaysOnTop() {
  const alwaysOnTop = useSettingsStore((state) => state.alwaysOnTop)

  useEffect(() => {
    getCurrentWindow()
      .setAlwaysOnTop(alwaysOnTop)
      .catch(() => {})
  }, [alwaysOnTop])

  const toggle = useCallback(async () => {
    const next = !getSettings().alwaysOnTop
    try {
      await getCurrentWindow().setAlwaysOnTop(next)
    } catch (e) {
      toast.error(i18n.t("toast.alwaysOnTop.fail"), {
        description: errorMessage(e)
      })
      return
    }
    await updateSettings({ alwaysOnTop: next })
  }, [])

  return { alwaysOnTop, toggle }
}

// 부트스트랩 시 저장된 값을 창에 적용 (React 마운트 전)
export async function applyAlwaysOnTopFromSettings(): Promise<void> {
  try {
    await getCurrentWindow().setAlwaysOnTop(getSettings().alwaysOnTop)
  } catch {
    // jsdom/테스트 환경에서는 Tauri 창 API가 없으므로 무시
  }
}
