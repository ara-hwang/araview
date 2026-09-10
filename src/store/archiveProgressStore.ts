import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"

const MAX_ENTRIES = 100
const STORE_KEY = "archiveProgress"

function sanitizeProgress(value: unknown): Record<string, string> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return {}
  }
  const out: Record<string, string> = {}
  for (const [key, entry] of Object.entries(value)) {
    if (
      typeof key === "string" &&
      key !== "" &&
      typeof entry === "string" &&
      entry !== ""
    ) {
      out[key] = entry
    }
    if (Object.keys(out).length >= MAX_ENTRIES) break
  }
  return out
}

type ArchiveProgressState = {
  progress: Record<string, string>
  get: (archivePath: string) => string | null
  save: (archivePath: string, entryName: string) => Promise<void>
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

const persist = async (progress: Record<string, string>) => {
  try {
    const store = await getTauriStore()
    await store.set(STORE_KEY, progress)
    await store.save()
  } catch (e) {
    console.warn("[archiveProgress] persist failed:", e)
  }
}

/** LRU 비슷하게: 갱신된 항목을 맨 뒤로 보내고 상한을 넘기면 앞에서 제거 */
function upsert(
  progress: Record<string, string>,
  archivePath: string,
  entryName: string
): Record<string, string> {
  const next: Record<string, string> = { ...progress }
  delete next[archivePath]
  next[archivePath] = entryName
  const keys = Object.keys(next)
  while (keys.length > MAX_ENTRIES) {
    const oldest = keys.shift()
    if (oldest === undefined) break
    delete next[oldest]
  }
  return next
}

export const useArchiveProgressStore = create<ArchiveProgressState>(
  (set, get) => ({
    progress: {},
    get: (archivePath) => get().progress[archivePath] ?? null,
    save: async (archivePath, entryName) => {
      if (!archivePath || !entryName) return
      const next = upsert(get().progress, archivePath, entryName)
      set({ progress: next })
      await persist(next)
    },
    remove: async (archivePath) => {
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
  })
)
