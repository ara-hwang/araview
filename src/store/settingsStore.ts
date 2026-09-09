import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { toast } from "sonner"

export type CacheMode =
  "off" | "nearby" | "extended" | "memory-1gb" | "memory-2gb"

export type ViewMode = "single" | "left-to-right" | "right-to-left" | "webtoon"

export type ViewerBackground = "theme" | "black" | "white" | "checker"

export type DirSortKey = "name" | "date" | "size"

export type SettingsState = {
  loopNavigation: boolean
  cacheMode: CacheMode
  viewMode: ViewMode
  slideshowIntervalMs: number
  autoOpenLastFile: boolean
  viewerBackground: ViewerBackground
  autoHideUI: boolean
  sortKey: DirSortKey
  sortDescending: boolean
  shuffle: boolean
  includeSubfolders: boolean
}

type SettingsStoreActions = {
  setLoopNavigation: (
    nextLoopNavigation: SettingsState["loopNavigation"]
  ) => void
  setCacheMode: (nextCacheMode: SettingsState["cacheMode"]) => void
  setViewMode: (nextViewMode: SettingsState["viewMode"]) => void
  setSlideshowIntervalMs: (
    nextSlideshowIntervalMs: SettingsState["slideshowIntervalMs"]
  ) => void
}

type SettingsStore = SettingsState & SettingsStoreActions

const initialSettings: SettingsState = {
  loopNavigation: false,
  cacheMode: "nearby",
  viewMode: "single",
  slideshowIntervalMs: 3000,
  autoOpenLastFile: false,
  viewerBackground: "theme",
  autoHideUI: false,
  sortKey: "name",
  sortDescending: false,
  shuffle: false,
  includeSubfolders: false
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  ...initialSettings,
  setLoopNavigation: (nextLoopNavigation) =>
    set(() => ({
      loopNavigation: nextLoopNavigation
    })),
  setCacheMode: (nextCacheMode) =>
    set(() => ({
      cacheMode: nextCacheMode
    })),
  setViewMode: (nextViewMode) =>
    set(() => ({
      viewMode: nextViewMode
    })),
  setSlideshowIntervalMs: (nextSlideshowIntervalMs) =>
    set(() => ({
      slideshowIntervalMs: nextSlideshowIntervalMs
    }))
}))

let tauriStorePromise: Promise<TauriStore> | null = null

const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json")
  }
  return tauriStorePromise
}

export const getSettings = () => useSettingsStore.getState()

const BACKGROUND_ORDER: SettingsState["viewerBackground"][] = [
  "theme",
  "black",
  "white",
  "checker"
]

/** B 키: 배경 모드 순환 (theme → black → white → checker) */
export const cycleViewerBackground = () => {
  const current = getSettings().viewerBackground
  const next =
    BACKGROUND_ORDER[
      (BACKGROUND_ORDER.indexOf(current) + 1) % BACKGROUND_ORDER.length
    ]
  void updateSettings({ viewerBackground: next })
}

export const updateSettings = async (partial: Partial<SettingsState>) => {
  const next = {
    ...getSettings(),
    ...partial
  }

  useSettingsStore.setState(next)

  try {
    const store = await getTauriStore()
    await store.set("settings", next)
    await store.save()
    return true
  } catch (e) {
    // 설정 저장 실패는 UI 동작을 막지 않지만 사용자에게 알림
    toast.error("설정 저장 실패", { description: String(e) })
    return false
  }
}

export const resetSettings = async () => {
  const saved = await updateSettings({ ...initialSettings })
  if (saved) toast.success("설정을 초기화했습니다")
}

export const initSettingsFromStore = async () => {
  try {
    const store = await getTauriStore()
    const stored = await store.get<SettingsState>("settings")
    if (stored) useSettingsStore.setState({ ...initialSettings, ...stored })
  } catch (e) {
    // 초기 로드 실패 시 기본값 유지하되 콘솔에 기록
    console.warn("[settings] Failed to load persisted settings:", e)
  }
}
