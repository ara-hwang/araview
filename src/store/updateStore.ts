import { create } from "zustand"

export type UpdateStage = "idle" | "available" | "downloading" | "ready"

type UpdateStoreState = {
  stage: UpdateStage
  version: string | null
  body: string | null
  /** 0-100. 총 길이를 알기 전에는 null이다. */
  pct: number | null
  error: string | null
  showAvailable: (version: string, body: string | null) => void
  startDownload: () => void
  setProgress: (pct: number | null) => void
  markReady: () => void
  setDownloadError: (message: string) => void
  dismiss: () => void
}

const initialUpdate: Pick<
  UpdateStoreState,
  "stage" | "version" | "body" | "pct" | "error"
> = {
  stage: "idle",
  version: null,
  body: null,
  pct: null,
  error: null
}

export const useUpdateStore = create<UpdateStoreState>((set) => ({
  ...initialUpdate,
  showAvailable: (version, body) =>
    set({ stage: "available", version, body, pct: null, error: null }),
  startDownload: () => set({ stage: "downloading", pct: null, error: null }),
  setProgress: (pct) => set({ pct }),
  markReady: () => set({ stage: "ready", pct: 100, error: null }),
  setDownloadError: (message) => set({ error: message }),
  dismiss: () => set({ ...initialUpdate })
}))
