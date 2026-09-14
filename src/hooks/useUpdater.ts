import { useEffect, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { toast } from "sonner"
import i18n from "@/i18n"
import { errorMessage } from "@/utils/appError"

/** 명령 팔레트 등 전역에서 수동 업데이트 확인을 요청하는 이벤트. */
export const REQUEST_UPDATE_CHECK_EVENT = "tiv:check-updates"

export function requestUpdateCheck() {
  window.dispatchEvent(new CustomEvent(REQUEST_UPDATE_CHECK_EVENT))
}

let checkInFlight = false

export type UpdateCheckResult = "latest" | "available" | "busy" | "failed"

/**
 * 수동 업데이트 확인. 오프라인 우선 원칙에 따라 시작 시 자동 확인은
 * 하지 않고, 사용자가 버튼/팔레트로 요청할 때만 네트워크를 쓴다.
 */
export async function checkForUpdatesNow(): Promise<UpdateCheckResult> {
  if (checkInFlight) return "busy"
  checkInFlight = true
  try {
    const update = await check()
    if (!update) {
      toast.success(i18n.t("toast.update.latest"))
      return "latest"
    }
    notifyUpdateAvailable(update)
    return "available"
  } catch (e) {
    toast.error(i18n.t("toast.update.checkFail"), {
      description: errorMessage(e)
    })
    return "failed"
  } finally {
    checkInFlight = false
  }
}

function notifyUpdateAvailable(update: Update) {
  toast.message(
    i18n.t("toast.update.availableTitle", { version: update.version }),
    {
      description: update.body ?? i18n.t("toast.update.availableDesc"),
      duration: 15000,
      action: {
        label: i18n.t("toast.update.install"),
        onClick: () => void installUpdate(update)
      }
    }
  )
}

async function installUpdate(update: Update) {
  const id = "araview-update-install"
  try {
    let total: number | undefined
    let downloaded = 0
    toast.loading(i18n.t("toast.update.downloading"), { id })
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength
        if (total) {
          const pct = Math.min(100, Math.round((downloaded / total) * 100))
          toast.loading(i18n.t("toast.update.downloadingPct", { pct }), { id })
        }
      }
    })
    toast.success(i18n.t("toast.update.installed"), {
      id,
      action: {
        label: i18n.t("toast.update.restart"),
        onClick: () =>
          void relaunch().catch((e: unknown) =>
            toast.error(i18n.t("toast.update.restartFail"), {
              description: errorMessage(e)
            })
          )
      }
    })
  } catch (e) {
    toast.error(i18n.t("toast.update.installFail"), {
      id,
      description: errorMessage(e)
    })
  }
}

/** 설정 화면에 표시할 현재 앱 버전. 실패 시 null이다. */
export function useAppVersion() {
  const [version, setVersion] = useState<string | null>(null)
  useEffect(() => {
    let alive = true
    getVersion()
      .then((v) => {
        if (alive) setVersion(v)
      })
      .catch(() => {
        if (alive) setVersion(null)
      })
    return () => {
      alive = false
    }
  }, [])
  return version
}

/** 루트에 마운트해 팔레트/외부의 확인 요청을 실제 확인으로 연결한다. */
export function useUpdateCheckRequestListener() {
  useEffect(() => {
    const handler = () => {
      void checkForUpdatesNow()
    }
    window.addEventListener(REQUEST_UPDATE_CHECK_EVENT, handler)
    return () => {
      window.removeEventListener(REQUEST_UPDATE_CHECK_EVENT, handler)
    }
  }, [])
}
