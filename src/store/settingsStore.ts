import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { toast } from "sonner"
import { create } from "zustand"

import {
  DEFAULT_MOUSE,
  DEFAULT_SHORTCUTS,
  DEFAULT_WHEEL,
  sanitizeMouseMap,
  sanitizeShortcutMap,
  sanitizeWheelMap
} from "@/constants/shortcuts"
import i18n, {
  detectSystemLanguage,
  normalizeLanguage,
  setI18nLanguage,
  type AppLanguage
} from "@/i18n"
import { errorMessage } from "@/utils/appError"

export type { AppLanguage }

export type CacheMode = "off" | "nearby" | "extended" | "memory-1gb" | "memory-2gb"

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
  menuBarHidden: boolean
  alwaysOnTop: boolean
  sortKey: DirSortKey
  sortDescending: boolean
  shuffle: boolean
  includeSubfolders: boolean
  skipBrokenFiles: boolean
  shortcuts: import("@/constants/shortcuts").ShortcutMap
  wheel: import("@/constants/shortcuts").WheelMap
  mouse: import("@/constants/shortcuts").MouseMap
}

type SettingsStoreActions = {
  setLoopNavigation: (nextLoopNavigation: SettingsState["loopNavigation"]) => void
  setCacheMode: (nextCacheMode: SettingsState["cacheMode"]) => void
  setViewMode: (nextViewMode: SettingsState["viewMode"]) => void
  setSlideshowIntervalMs: (nextSlideshowIntervalMs: SettingsState["slideshowIntervalMs"]) => void
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
  menuBarHidden: false,
  alwaysOnTop: false,
  sortKey: "name",
  sortDescending: false,
  shuffle: false,
  includeSubfolders: false,
  skipBrokenFiles: false,
  shortcuts: { ...DEFAULT_SHORTCUTS },
  wheel: { ...DEFAULT_WHEEL },
  mouse: { ...DEFAULT_MOUSE }
}

export const DEFAULT_SETTINGS: SettingsState = { ...initialSettings }

const CACHE_MODES: readonly CacheMode[] = ["off", "nearby", "extended", "memory-1gb", "memory-2gb"]
const VIEW_MODES: readonly ViewMode[] = ["single", "left-to-right", "right-to-left", "webtoon"]
const VIEWER_BACKGROUNDS: readonly ViewerBackground[] = ["theme", "black", "white", "checker"]
const SORT_KEYS: readonly DirSortKey[] = ["name", "date", "size"]

function sanitizeEnum<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback
}

function sanitizeBoolean(value: unknown): boolean {
  return value === true
}

/** 저장된 설정이 손상됐어도 유효한 SettingsState로 복원한다. */
export function sanitizeSettings(value: unknown): SettingsState {
  const record =
    typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}
  const language = normalizeLanguage(record.language) ?? initialSettings.language
  const intervalRaw = record.slideshowIntervalMs
  const slideshowIntervalMs =
    typeof intervalRaw === "number" &&
    Number.isFinite(intervalRaw) &&
    intervalRaw >= 1000 &&
    intervalRaw <= 30000
      ? Math.round(intervalRaw)
      : initialSettings.slideshowIntervalMs
  return {
    language,
    loopNavigation: sanitizeBoolean(record.loopNavigation),
    cacheMode: sanitizeEnum(record.cacheMode, CACHE_MODES, initialSettings.cacheMode),
    viewMode: sanitizeEnum(record.viewMode, VIEW_MODES, initialSettings.viewMode),
    slideshowIntervalMs,
    autoOpenLastFile: sanitizeBoolean(record.autoOpenLastFile),
    recordRecentFiles:
      record.recordRecentFiles === undefined
        ? initialSettings.recordRecentFiles
        : sanitizeBoolean(record.recordRecentFiles),
    viewerBackground: sanitizeEnum(
      record.viewerBackground,
      VIEWER_BACKGROUNDS,
      initialSettings.viewerBackground
    ),
    autoHideUI: sanitizeBoolean(record.autoHideUI),
    menuBarHidden: sanitizeBoolean(record.menuBarHidden),
    alwaysOnTop: sanitizeBoolean(record.alwaysOnTop),
    sortKey: sanitizeEnum(record.sortKey, SORT_KEYS, initialSettings.sortKey),
    sortDescending: sanitizeBoolean(record.sortDescending),
    shuffle: sanitizeBoolean(record.shuffle),
    includeSubfolders: sanitizeBoolean(record.includeSubfolders),
    skipBrokenFiles: sanitizeBoolean(record.skipBrokenFiles),
    shortcuts: sanitizeShortcutMap(record.shortcuts),
    wheel: sanitizeWheelMap(record.wheel),
    mouse: sanitizeMouseMap(record.mouse)
  }
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

const BACKGROUND_ORDER: SettingsState["viewerBackground"][] = ["theme", "black", "white", "checker"]

/** B 키: 배경 모드 순환 (theme → black → white → checker) */
export const cycleViewerBackground = () => {
  const current = getSettings().viewerBackground
  const next = BACKGROUND_ORDER[(BACKGROUND_ORDER.indexOf(current) + 1) % BACKGROUND_ORDER.length]
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
    toast.error(i18n.t("toast.settings.saveFail"), {
      description: errorMessage(e)
    })
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
      normalizeLanguage((stored as { language?: unknown } | null)?.language) ?? null
    const language = storedLanguage ?? systemLanguage
    if (stored) {
      useSettingsStore.setState({
        ...sanitizeSettings(stored),
        language
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
