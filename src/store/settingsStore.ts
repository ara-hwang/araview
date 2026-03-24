import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"

export type CacheMode =
  | "off"
  | "nearby"
  | "extended"
  | "memory-1gb"
  | "memory-2gb"

export type ViewMode = "single" | "left-to-right" | "right-to-left" | "webtoon"

export type SettingsState = {
  loopNavigation: boolean
  cacheMode: CacheMode
  viewMode: ViewMode
}

type SettingsStoreActions = {
  setLoopNavigation: (
    nextLoopNavigation: SettingsState["loopNavigation"]
  ) => void
  setCacheMode: (nextCacheMode: SettingsState["cacheMode"]) => void
  setViewMode: (nextViewMode: SettingsState["viewMode"]) => void
}

type SettingsStore = SettingsState & SettingsStoreActions

const initialSettings: SettingsState = {
  loopNavigation: false,
  cacheMode: "nearby",
  viewMode: "single"
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
    }))
}))

// Tauri 스토어
let tauriStorePromise: Promise<TauriStore> | null = null

const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json")
  }
  return tauriStorePromise
}

export const getSettings = () => useSettingsStore.getState()

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
  } catch {
    // 설정 저장 실패는 UI 동작을 막지 않음
  }
}

export const initSettingsFromStore = async () => {
  try {
    const store = await getTauriStore()
    const stored = await store.get<SettingsState>("settings")
    if (stored) useSettingsStore.setState({ ...initialSettings, ...stored })
  } catch {
    // 초기 로드 실패 시 기본값 유지
  }
}
