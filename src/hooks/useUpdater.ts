import { getVersion } from "@tauri-apps/api/app"
import { relaunch } from "@tauri-apps/plugin-process"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { useEffect, useState } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { useUpdateStore } from "@/store/updateStore"
import { errorMessage } from "@/utils/appError"

/** 명령 팔레트 등 전역에서 수동 업데이트 확인을 요청하는 이벤트. */
export const REQUEST_UPDATE_CHECK_EVENT = "tiv:check-updates"

export function requestUpdateCheck() {
  window.dispatchEvent(new CustomEvent(REQUEST_UPDATE_CHECK_EVENT))
}

let checkInFlight = false
let downloadInFlight = false
let pendingUpdate: Update | null = null

export type UpdateCheckResult = "latest" | "available" | "busy" | "failed"

/**
 * 수동 업데이트 확인. 오프라인 우선 원칙에 따라 시작 시 자동 확인은
 * 하지 않고, 사용자가 버튼/팔레트로 요청할 때만 네트워크를 쓴다.
 * 새 버전이 있으면 Dialog로 다운로드 여부를 묻는다.
 */
export async function checkForUpdatesNow(): Promise<UpdateCheckResult> {
  if (checkInFlight) return "busy"
  if (useUpdateStore.getState().stage !== "idle") return "busy"
  checkInFlight = true
  try {
    const update = await check()
    if (!update) {
      toast.success(i18n.t("toast.update.latest"))
      return "latest"
    }
    pendingUpdate = update
    useUpdateStore.getState().showAvailable(update.version, update.body ?? null)
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

/** 사용 가능한 Dialog에서 [다운로드]를 눌렀을 때 호출된다. */
export async function startUpdateDownload(): Promise<void> {
  const update = pendingUpdate
  if (!update || downloadInFlight) return
  downloadInFlight = true
  useUpdateStore.getState().startDownload()
  try {
    let total: number | undefined
    let downloaded = 0
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength
        useUpdateStore.getState().setProgress(null)
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength
        if (total) {
          const pct = Math.min(100, Math.round((downloaded / total) * 100))
          useUpdateStore.getState().setProgress(pct)
        }
      }
    })
    useUpdateStore.getState().markReady()
  } catch (e) {
    useUpdateStore.getState().setDownloadError(errorMessage(e))
  } finally {
    downloadInFlight = false
  }
}

/**
 * Dialog를 닫는다. 다운로드 진행 중(에러 없음)에는 닫기를 무시한다.
 * ready 단계의 [닫은 후 적용]은 다음 실행 시 적용됨을 토스트로 알린다.
 */
export function dismissUpdate(deferred = false) {
  const { stage, error } = useUpdateStore.getState()
  if (stage === "downloading" && !error) return
  pendingUpdate = null
  useUpdateStore.getState().dismiss()
  if (deferred) {
    toast.success(i18n.t("toast.update.deferred"))
  }
}

/** 설치 준비 완료 Dialog에서 [지금 다시 시작]을 눌렀을 때 호출된다. */
export async function relaunchAfterUpdate(): Promise<void> {
  try {
    await relaunch()
  } catch (e) {
    toast.error(i18n.t("toast.update.restartFail"), {
      description: errorMessage(e)
    })
  }
}

/** 테스트에서 모듈 상태(진행 중 플래그, 보류 업데이트)를 초기화한다. */
export function __resetUpdateModuleForTests() {
  checkInFlight = false
  downloadInFlight = false
  pendingUpdate = null
  useUpdateStore.getState().dismiss()
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
