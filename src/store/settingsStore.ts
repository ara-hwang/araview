import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { toast } from "sonner"
import i18n, {
  detectSystemLanguage,
  normalizeLanguage,
  setI18nLanguage,
  type AppLanguage
} from "@/i18n"
import {
  DEFAULT_MOUSE,
  DEFAULT_SHORTCUTS,
  DEFAULT_WHEEL,
  sanitizeMouseMap,
  sanitizeShortcutMap,
  sanitizeWheelMap
} from "@/constants/shortcuts"

export type { AppLanguage }

export type CacheMode =
  "off" | "nearby" | "extended" | "memory-1gb" | "memory-2gb"

export type ViewMode = "single" | "left-to-right" | "right-to-left" | "webtoon"

export type ViewerBackground = "theme" | "black" | "white" | "checker"

export type DirSortKey = "name" | "date" | "size"

export type { MouseMap, ShortcutMap, WheelMap } from "@/constants/shortcuts"

export type SettingsState = {
  language: AppLanguage
  loopNavigation: boolean
  cacheMode: CacheMode
  viewMode: ViewMode
  slideshowIntervalMs: number
  autoOpenLastFile: boolean
  recordRecentFiles: boolean
  viewerBackground: ViewerBackground
  autoHideUI: boolean
  alwaysOnTop: boolean
  sortKey: DirSortKey
  sortDescending: boolean
  shuffle: boolean
  includeSubfolders: boolean
  shortcuts: import("@/constants/shortcuts").ShortcutMap
  wheel: import("@/constants/shortcuts").WheelMap
  mouse: import("@/constants/shortcuts").MouseMap
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
  language: "ko",
  loopNavigation: false,
  cacheMode: "nearby",
  viewMode: "single",
  slideshowIntervalMs: 3000,
  autoOpenLastFile: false,
  recordRecentFiles: true,
  viewerBackground: "theme",
  autoHideUI: false,
  alwaysOnTop: false,
  sortKey: "name",
  sortDescending: false,
  shuffle: false,
  includeSubfolders: false,
  shortcuts: { ...DEFAULT_SHORTCUTS },
  wheel: { ...DEFAULT_WHEEL },
  mouse: { ...DEFAULT_MOUSE }
}

export const DEFAULT_SETTINGS: SettingsState = { ...initialSettings }

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

  if (partial.language && partial.language !== i18n.language) {
    await setI18nLanguage(partial.language)
  }

  try {
    const store = await getTauriStore()
    await store.set("settings", next)
    await store.save()
    return true
  } catch (e) {
    // 설정 저장 실패는 UI 동작을 막지 않지만 사용자에게 알림
    toast.error(i18n.t("toast.settings.saveFail"), { description: String(e) })
    return false
  }
}

export const setLanguage = async (language: AppLanguage) => {
  await updateSettings({ language })
}

export const resetSettings = async () => {
  const saved = await updateSettings({
    ...initialSettings,
    language: getSettings().language
  })
  if (saved) toast.success(i18n.t("toast.settings.resetDone"))
}

export const resetShortcutsToDefault = async () => {
  const saved = await updateSettings({
    shortcuts: { ...DEFAULT_SHORTCUTS },
    wheel: { ...DEFAULT_WHEEL },
    mouse: { ...DEFAULT_MOUSE }
  })
  if (saved) toast.success(i18n.t("toast.settings.resetDone"))
}

export const initSettingsFromStore = async () => {
  const systemLanguage = detectSystemLanguage()
  try {
    const store = await getTauriStore()
    const stored = await store.get<SettingsState>("settings")
    const storedLanguage =
      normalizeLanguage((stored as { language?: unknown } | null)?.language) ??
      null
    const language = storedLanguage ?? systemLanguage
    if (stored) {
      const storedRecord = stored as Partial<SettingsState> & {
        shortcuts?: unknown
        wheel?: unknown
        mouse?: unknown
      }
      useSettingsStore.setState({
        ...initialSettings,
        ...stored,
        language,
        shortcuts: sanitizeShortcutMap(storedRecord.shortcuts),
        wheel: sanitizeWheelMap(storedRecord.wheel),
        mouse: sanitizeMouseMap(storedRecord.mouse)
      })
    } else {
      useSettingsStore.setState({
        ...initialSettings,
        language: systemLanguage
      })
    }
    await setI18nLanguage(language)
    return language
  } catch (e) {
    // 초기 로드 실패 시 기본값 유지하되 콘솔에 기록
    console.warn("[settings] Failed to load persisted settings:", e)
    useSettingsStore.setState({ ...initialSettings, language: systemLanguage })
    await setI18nLanguage(systemLanguage)
    return systemLanguage
  }
}
