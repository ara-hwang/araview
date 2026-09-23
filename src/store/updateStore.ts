import { create } from "zustand"

export type UpdateStage = "idle" | "available" | "downloading" | "installing"

type UpdateStoreState = {
  stage: UpdateStage
  version: string | null
  body: string | null
  /** 0-100. 총 길이를 알기 전에는 null이다. */
  pct: number | null
  /** 이번 다운로드에서 누적된 바이트. Started에서 0으로 초기화한다. */
  downloadedBytes: number
  /** 서버가 알려준 총 바이트. 모르면 null이다. */
  totalBytes: number | null
  error: string | null
  showAvailable: (version: string, body: string | null) => void
  startDownload: () => void
  setProgress: (pct: number | null, downloadedBytes: number, totalBytes: number | null) => void
  markInstalling: () => void
  setDownloadError: (message: string) => void
  dismiss: () => void
}

const initialUpdate: Pick<
  UpdateStoreState,
  "stage" | "version" | "body" | "pct" | "downloadedBytes" | "totalBytes" | "error"
> = {
  stage: "idle",
  version: null,
  body: null,
  pct: null,
  downloadedBytes: 0,
  totalBytes: null,
  error: null
}

export const useUpdateStore = create<UpdateStoreState>((set) => ({
  ...initialUpdate,
  showAvailable: (version, body) =>
    set({
      stage: "available",
      version,
      body,
      pct: null,
      downloadedBytes: 0,
      totalBytes: null,
      error: null
    }),
  startDownload: () =>
    set({
      stage: "downloading",
      pct: null,
      downloadedBytes: 0,
      totalBytes: null,
      error: null
    }),
  setProgress: (pct, downloadedBytes, totalBytes) => set({ pct, downloadedBytes, totalBytes }),
  // Windows에서 플러그인이 NSIS 설치 관리자를 띄우고 프로세스를 종료하므로,
  // 다운로드 완료(Finished) 시점이 사용자에게 보여줄 수 있는 마지막 상태다.
  markInstalling: () => set({ stage: "installing", pct: 100, error: null }),
  setDownloadError: (message) => set({ error: message }),
  dismiss: () => set({ ...initialUpdate })
}))
