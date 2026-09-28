import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { create } from "zustand"

const MAX_ENTRIES = 100
const STORE_KEY = "archiveProgress"

/** 아카이브(만화) 읽기 진도. index/total은 최근 파일 목록 표시용이다. */
export type ArchiveReadingProgress = {
  /** 마지막으로 본 엔트리 이름 (이어보기 기준) */
  entry: string
  /** 목록에서의 위치 (0-based). 알 수 없으면 -1 */
  index: number
  /** 전체 페이지 수. 알 수 없으면 0 */
  total: number
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value !== ""
}

function toCount(value: unknown, min: number): number | null {
  return typeof value === "number" && Number.isInteger(value) && value >= min ? value : null
}

/**
 * 저장값을 복원한다. 구버전(string)과 현재(object) 저장값을 모두 받는다.
 * 항목 수는 MAX_ENTRIES로 제한한다.
 */
export function sanitizeProgress(value: unknown): Record<string, ArchiveReadingProgress> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return {}
  }
  const out: Record<string, ArchiveReadingProgress> = {}
  for (const [key, raw] of Object.entries(value)) {
    if (key === "") continue
    let record: ArchiveReadingProgress | null = null
    if (isNonEmptyString(raw)) {
      // 구버전: 엔트리 이름만 저장했다.
      record = { entry: raw, index: -1, total: 0 }
    } else if (typeof raw === "object" && raw !== null && !Array.isArray(raw)) {
      const candidate = raw as Record<string, unknown>
      if (isNonEmptyString(candidate.entry)) {
        record = {
          entry: candidate.entry,
          index: toCount(candidate.index, 0) ?? -1,
          total: toCount(candidate.total, 1) ?? 0
        }
      }
    }
    if (record) out[key] = record
    if (Object.keys(out).length >= MAX_ENTRIES) break
  }
  return out
}

type ArchiveProgressState = {
  progress: Record<string, ArchiveReadingProgress>
  /** 이어보기용 엔트리 이름. 기록이 없으면 null */
  get: (archivePath: string) => string | null
  getProgress: (archivePath: string) => ArchiveReadingProgress | null
  save: (
    archivePath: string,
    entryName: string,
    meta?: { index?: number; total?: number }
  ) => Promise<void>
  remove: (archivePath: string) => Promise<void>
  init: () => Promise<void>
}

let tauriStorePromise: Promise<TauriStore> | null = null
const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json")
  }
  return tauriStorePromise
}

const persist = async (progress: Record<string, ArchiveReadingProgress>) => {
  try {
    const store = await getTauriStore()
    await store.set(STORE_KEY, progress)
    await store.save()
  } catch (e) {
    console.warn("[archiveProgress] persist failed:", e)
  }
}

/**
 * 디스크 쓰기 trailing 디바운스. 웹툰 연속 스크롤은 중앙 페이지가 바뀔 때마다
 * save를 발화하므로, 상태는 즉시 반영하되 persist는 아카이브별로 마지막 호출
 * 뒤 DEBOUNCE_MS가 지나면 한 번만 실행한다.
 */
const DEBOUNCE_MS = 600
const persistTimers = new Map<string, number>()

const persistDebounced = (
  archivePath: string,
  progress: Record<string, ArchiveReadingProgress>
) => {
  const existing = persistTimers.get(archivePath)
  if (existing !== undefined) window.clearTimeout(existing)
  const timer = window.setTimeout(() => {
    persistTimers.delete(archivePath)
    void persist(progress)
  }, DEBOUNCE_MS)
  persistTimers.set(archivePath, timer)
}

/** LRU 비슷하게: 갱신된 항목을 맨 뒤로 보내고 상한을 넘기면 앞에서 제거 */
function upsert(
  progress: Record<string, ArchiveReadingProgress>,
  archivePath: string,
  entry: ArchiveReadingProgress
): Record<string, ArchiveReadingProgress> {
  const next: Record<string, ArchiveReadingProgress> = { ...progress }
  delete next[archivePath]
  next[archivePath] = entry
  const keys = Object.keys(next)
  while (keys.length > MAX_ENTRIES) {
    const oldest = keys.shift()
    if (oldest === undefined) break
    delete next[oldest]
  }
  return next
}

export const useArchiveProgressStore = create<ArchiveProgressState>((set, get) => ({
  progress: {},
  get: (archivePath) => get().progress[archivePath]?.entry ?? null,
  getProgress: (archivePath) => get().progress[archivePath] ?? null,
  save: async (archivePath, entryName, meta) => {
    if (!archivePath || !entryName) return
    const previous = get().progress[archivePath]
    const index = toCount(meta?.index, 0) ?? previous?.index ?? -1
    const total = toCount(meta?.total, 1) ?? previous?.total ?? 0
    const next = upsert(get().progress, archivePath, { entry: entryName, index, total })
    set({ progress: next })
    persistDebounced(archivePath, next)
  },
  remove: async (archivePath) => {
    const pending = persistTimers.get(archivePath)
    if (pending !== undefined) {
      window.clearTimeout(pending)
      persistTimers.delete(archivePath)
    }
    if (!(archivePath in get().progress)) return
    const next = { ...get().progress }
    delete next[archivePath]
    set({ progress: next })
    await persist(next)
  },
  init: async () => {
    try {
      const store = await getTauriStore()
      const stored = await store.get<unknown>(STORE_KEY)
      set({ progress: sanitizeProgress(stored) })
    } catch (e) {
      console.warn("[archiveProgress] load failed:", e)
    }
  }
}))
