import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { create } from "zustand"

import { toast } from "@/components/ui/toast"
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
import type { CacheStorageMode } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

export type { AppLanguage }
export type { CacheStorageMode } from "@/types"

export type CacheMode = "off" | "nearby" | "extended" | "memory-1gb" | "memory-2gb"

/** 표시 해상도 상한. original=원본, 4k=3840px, 1080p=1920px (긴 변 기준) */
export type MaxResolution = "original" | "4k" | "1080p"

/** 이미지 표시 스케일링. auto는 픽셀 아트 자동 감지 결과를 따른다. */
export type ImageScalingMode = "auto" | "smooth" | "pixelated"

export type ViewMode = "single" | "left-to-right" | "right-to-left" | "webtoon"

export type ViewerBackground = "theme" | "black" | "white" | "checker"

export type DirSortKey = "name" | "date" | "size"

export type FitMode = "width" | "height" | "screen" | "auto"

export type DockPosition = "top" | "bottom" | "left" | "right"

export type DockThumbSize = "s" | "m" | "l"

export type { MouseMap, ShortcutMap, WheelMap } from "@/constants/shortcuts"

export type SettingsState = {
  language: AppLanguage
  loopNavigation: boolean
  cacheMode: CacheMode
  /** 이미지 파생 캐시를 실행 중에만 유지하거나 다음 실행에도 재사용한다. */
  cacheStorageMode: CacheStorageMode
  /** 대용량 이미지를 긴 변 기준으로 축소해 표시한다 (렌더 메모리 절감) */
  maxResolution: MaxResolution
  /** 이미지 확대 시 보간 방식. auto는 픽셀 아트 자동 감지를 사용한다. */
  imageScalingMode: ImageScalingMode
  /** 자동 모드에서 픽셀 아트 감지를 적용한다. */
  autoDetectPixelArt: boolean
  viewMode: ViewMode
  /** 웹툰 이미지 사이 간격(px) */
  webtoonImageGap: number
  /** 웹툰 이미지 사이에 페이지 경계선 표시 */
  webtoonPageBoundaries: boolean
  /** 웹툰 이미지를 읽기 영역 너비까지 확대 */
  webtoonFitWidth: boolean
  /** 웹툰 읽기 위치와 전체 진행률 표시 */
  webtoonShowProgress: boolean
  /** 웹툰 진행 표시에서 썸네인 그리드 바로가기 */
  webtoonThumbnailJump: boolean
  autoOpenLastFile: boolean
  recordRecentFiles: boolean
  viewerBackground: ViewerBackground
  autoHideUI: boolean
  menuBarHidden: boolean
  alwaysOnTop: boolean
  sortKey: DirSortKey
  sortDescending: boolean
  includeSubfolders: boolean
  skipBrokenFiles: boolean
  /** 아카이브(만화) 재진입 시 마지막으로 본 페이지에서 이어본다 */
  resumeReading: boolean
  /** 양쪽 보기에서 첫 페이지(표지)를 단독으로 표시한다 */
  showCoverAlone: boolean
  /** 정보 패널에 CBZ/ZIP ComicInfo 섹션을 표시한다 */
  showComicInfo: boolean
  /** 아카이브(만화)를 열면 자동으로 양쪽 보기로 본다. ComicInfo의 Manga 방향을 따른다 */
  comicAutoDualView: boolean
  fitMode: FitMode
  dockPosition: DockPosition
  dockVisible: boolean
  dockThumbSize: DockThumbSize
  dockShowName: boolean
  dockShowIndex: boolean
  shortcuts: import("@/constants/shortcuts").ShortcutMap
  wheel: import("@/constants/shortcuts").WheelMap
  mouse: import("@/constants/shortcuts").MouseMap
}

type SettingsStoreActions = {
  setLoopNavigation: (nextLoopNavigation: SettingsState["loopNavigation"]) => void
  setCacheMode: (nextCacheMode: SettingsState["cacheMode"]) => void
  setViewMode: (nextViewMode: SettingsState["viewMode"]) => void
}

type SettingsStore = SettingsState & SettingsStoreActions

const initialSettings: SettingsState = {
  language: "ko",
  loopNavigation: false,
  cacheMode: "nearby",
  cacheStorageMode: "persistent",
  maxResolution: "original",
  imageScalingMode: "auto",
  autoDetectPixelArt: true,
  viewMode: "single",
  webtoonImageGap: 8,
  webtoonPageBoundaries: false,
  webtoonFitWidth: false,
  webtoonShowProgress: true,
  webtoonThumbnailJump: true,
  autoOpenLastFile: false,
  recordRecentFiles: true,
  viewerBackground: "theme",
  autoHideUI: false,
  menuBarHidden: false,
  alwaysOnTop: false,
  sortKey: "name",
  sortDescending: false,
  includeSubfolders: false,
  skipBrokenFiles: false,
  resumeReading: true,
  showCoverAlone: true,
  showComicInfo: true,
  comicAutoDualView: true,
  fitMode: "auto",
  dockPosition: "bottom",
  dockVisible: true,
  dockThumbSize: "s",
  dockShowName: false,
  dockShowIndex: false,
  shortcuts: { ...DEFAULT_SHORTCUTS },
  wheel: { ...DEFAULT_WHEEL },
  mouse: { ...DEFAULT_MOUSE }
}

export const DEFAULT_SETTINGS: SettingsState = { ...initialSettings }

const CACHE_MODES: readonly CacheMode[] = ["off", "nearby", "extended", "memory-1gb", "memory-2gb"]
const CACHE_STORAGE_MODES: readonly CacheStorageMode[] = ["temporary", "persistent"]
const MAX_RESOLUTIONS: readonly MaxResolution[] = ["original", "4k", "1080p"]
const IMAGE_SCALING_MODES: readonly ImageScalingMode[] = ["auto", "smooth", "pixelated"]
const VIEW_MODES: readonly ViewMode[] = ["single", "left-to-right", "right-to-left", "webtoon"]
const VIEWER_BACKGROUNDS: readonly ViewerBackground[] = ["theme", "black", "white", "checker"]
const SORT_KEYS: readonly DirSortKey[] = ["name", "date", "size"]
const FIT_MODES: readonly FitMode[] = ["width", "height", "screen", "auto"]
const DOCK_POSITIONS: readonly DockPosition[] = ["top", "bottom", "left", "right"]
const DOCK_THUMB_SIZES: readonly DockThumbSize[] = ["s", "m", "l"]

function sanitizeEnum<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback
}

function sanitizeBoolean(value: unknown): boolean {
  return value === true
}

function sanitizeBooleanWithDefault(value: unknown, fallback: boolean): boolean {
  return value === undefined ? fallback : sanitizeBoolean(value)
}

function sanitizeWebtoonImageGap(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return initialSettings.webtoonImageGap
  return Math.round(Math.min(Math.max(value, 0), 64))
}

/** 저장된 설정이 손상됐어도 유효한 SettingsState로 복원한다. */
export function sanitizeSettings(value: unknown): SettingsState {
  const record =
    typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}
  const language = normalizeLanguage(record.language) ?? initialSettings.language
  return {
    language,
    loopNavigation: sanitizeBoolean(record.loopNavigation),
    cacheMode: sanitizeEnum(record.cacheMode, CACHE_MODES, initialSettings.cacheMode),
    cacheStorageMode: sanitizeEnum(
      record.cacheStorageMode,
      CACHE_STORAGE_MODES,
      initialSettings.cacheStorageMode
    ),
    maxResolution: sanitizeEnum(
      record.maxResolution,
      MAX_RESOLUTIONS,
      initialSettings.maxResolution
    ),
    imageScalingMode: sanitizeEnum(
      record.imageScalingMode,
      IMAGE_SCALING_MODES,
      initialSettings.imageScalingMode
    ),
    autoDetectPixelArt: sanitizeBooleanWithDefault(
      record.autoDetectPixelArt,
      initialSettings.autoDetectPixelArt
    ),
    viewMode: sanitizeEnum(record.viewMode, VIEW_MODES, initialSettings.viewMode),
    webtoonImageGap: sanitizeWebtoonImageGap(record.webtoonImageGap),
    webtoonPageBoundaries: sanitizeBoolean(record.webtoonPageBoundaries),
    webtoonFitWidth: sanitizeBoolean(record.webtoonFitWidth),
    webtoonShowProgress: sanitizeBooleanWithDefault(
      record.webtoonShowProgress,
      initialSettings.webtoonShowProgress
    ),
    webtoonThumbnailJump: sanitizeBooleanWithDefault(
      record.webtoonThumbnailJump,
      initialSettings.webtoonThumbnailJump
    ),
    autoOpenLastFile: sanitizeBoolean(record.autoOpenLastFile),
    recordRecentFiles: sanitizeBooleanWithDefault(
      record.recordRecentFiles,
      initialSettings.recordRecentFiles
    ),
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
    includeSubfolders: sanitizeBoolean(record.includeSubfolders),
    skipBrokenFiles: sanitizeBoolean(record.skipBrokenFiles),
    resumeReading: sanitizeBooleanWithDefault(record.resumeReading, initialSettings.resumeReading),
    showCoverAlone: sanitizeBooleanWithDefault(
      record.showCoverAlone,
      initialSettings.showCoverAlone
    ),
    showComicInfo: sanitizeBooleanWithDefault(record.showComicInfo, initialSettings.showComicInfo),
    comicAutoDualView: sanitizeBooleanWithDefault(
      record.comicAutoDualView,
      initialSettings.comicAutoDualView
    ),
    fitMode: sanitizeEnum(record.fitMode, FIT_MODES, initialSettings.fitMode),
    dockPosition: sanitizeEnum(record.dockPosition, DOCK_POSITIONS, initialSettings.dockPosition),
    dockVisible: sanitizeBooleanWithDefault(record.dockVisible, initialSettings.dockVisible),
    dockThumbSize: sanitizeEnum(
      record.dockThumbSize,
      DOCK_THUMB_SIZES,
      initialSettings.dockThumbSize
    ),
    dockShowName: sanitizeBoolean(record.dockShowName),
    dockShowIndex: sanitizeBoolean(record.dockShowIndex),
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
      description: errorMessage(e),
      details: errorCopyDetails(e)
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
