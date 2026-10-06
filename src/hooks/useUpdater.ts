import { getVersion } from "@tauri-apps/api/app"
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater"
import { useEffect, useState } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import { useUpdateStore } from "@/store/updateStore"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

/** 명령 팔레트 등 전역에서 수동 업데이트 확인을 요청하는 이벤트. */
export const REQUEST_UPDATE_CHECK_EVENT = "tiv:check-updates"

export function requestUpdateCheck() {
  window.dispatchEvent(new CustomEvent(REQUEST_UPDATE_CHECK_EVENT))
}

/** 릴리스 피드(latest.json) 확인 요청의 HTTP 타임아웃. */
export const UPDATE_CHECK_TIMEOUT_MS = 30_000

/**
 * 설치 관리자 다운로드 요청의 HTTP 타임아웃. 플러그인은 이 값을 reqwest
 * 요청 전체(본문 수신 포함) 기한으로 쓰므로, 느린 회선에서도 끝낼 수 있게
 * 넉넉히 둔다. 무응답 연결에서 Dialog가 닫히지 않는 상태를 막는 상한이다.
 */
export const UPDATE_DOWNLOAD_TIMEOUT_MS = 10 * 60_000

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
    const update = await check({ timeout: UPDATE_CHECK_TIMEOUT_MS })
    if (!update) {
      toast.success(i18n.t("toast.update.latest"))
      return "latest"
    }
    pendingUpdate = update
    useUpdateStore.getState().showAvailable(update.version, update.body ?? null)
    return "available"
  } catch (e) {
    toast.error(i18n.t("toast.update.checkFail"), {
      description: errorMessage(e),
      details: errorCopyDetails(e)
    })
    return "failed"
  } finally {
    checkInFlight = false
  }
}

/**
 * 다운로드 진행률(0-100 정수)을 계산한다. 총 길이를 알 수 없거나 값이
 * 비정상적이면 null을 돌려주고, 호출자는 indeterminate 표시를 유지한다.
 * NaN/Infinity/음수/100 초과는 절대 내보내지 않는다.
 */
export function computeDownloadPct(
  downloaded: number,
  total: number | null | undefined
): number | null {
  if (total == null || !Number.isFinite(total) || total <= 0) return null
  if (!Number.isFinite(downloaded) || downloaded < 0) return null
  return Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)))
}

/** 사용 가능한 Dialog에서 [다운로드]를 눌렀을 때 호출된다. */
export async function startUpdateDownload(): Promise<void> {
  const update = pendingUpdate
  if (!update || downloadInFlight) return
  downloadInFlight = true
  useUpdateStore.getState().startDownload()
  try {
    let total: number | null = null
    let downloaded = 0
    const onEvent = (event: DownloadEvent) => {
      if (event.event === "Started") {
        const contentLength = event.data.contentLength
        total =
          typeof contentLength === "number" && Number.isFinite(contentLength) && contentLength > 0
            ? contentLength
            : null
        downloaded = 0
        useUpdateStore.getState().setProgress(null, 0, total)
      } else if (event.event === "Progress") {
        const chunkLength = event.data.chunkLength
        if (typeof chunkLength !== "number" || !Number.isFinite(chunkLength) || chunkLength < 0) {
          return
        }
        downloaded += chunkLength
        useUpdateStore
          .getState()
          .setProgress(computeDownloadPct(downloaded, total), downloaded, total)
      } else if (event.event === "Finished") {
        // Windows에서는 이 시점 이후 서명 검증, zip 해제, NSIS 실행을 거쳐
        // 플러그인이 프로세스를 종료한다. downloadAndInstall은 resolve되지
        // 않으므로 installing 단계가 종료 전 마지막 화면이 된다.
        useUpdateStore.getState().markInstalling()
      }
    }
    await update.downloadAndInstall(onEvent, { timeout: UPDATE_DOWNLOAD_TIMEOUT_MS })
    // Windows에서는 설치 관리자 인계 시 프로세스가 종료되어 여기에 도달하지 않는다.
  } catch (e) {
    useUpdateStore.getState().setDownloadError(errorMessage(e))
  } finally {
    downloadInFlight = false
  }
}

/**
 * Dialog를 닫는다. 다운로드/설치 인계 진행 중(에러 없음)에는 닫기를 무시한다.
 */
export function dismissUpdate() {
  const { stage, error } = useUpdateStore.getState()
  if ((stage === "downloading" || stage === "installing") && !error) return
  pendingUpdate = null
  useUpdateStore.getState().dismiss()
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
