import { useStore } from "zustand"
import { createStore } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"
import type { Settings } from "@/types/settings"

const DEFAULT_SETTINGS: Settings = {
  loopNavigation: false,
  cacheMode: "nearby",
  viewMode: "single"
}

const settingsStore = createStore<Settings>(() => DEFAULT_SETTINGS)

let tauriStorePromise: Promise<TauriStore> | null = null

const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json")
  }
  return tauriStorePromise
}

export const getSettings = () => settingsStore.getState()

export const subscribeToSettings = (listener: (settings: Settings) => void) => {
  return settingsStore.subscribe(listener)
}

export const updateSettings = async (partial: Partial<Settings>) => {
  const next = {
    ...settingsStore.getState(),
    ...partial
  }

  settingsStore.setState(next)

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
    const stored = await store.get<Settings>("settings")
    if (stored) settingsStore.setState({ ...DEFAULT_SETTINGS, ...stored })
  } catch {
    // 초기 로드 실패 시 기본값 유지
  }
}

export const useSettingsStore = () => {
  return useStore(settingsStore)
}
