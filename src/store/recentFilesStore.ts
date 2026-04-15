import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"

const MAX_RECENT = 20
const STORE_KEY = "recentFiles"

type RecentFilesState = {
  files: string[]
  add: (filePath: string) => Promise<void>
  remove: (filePath: string) => Promise<void>
  clear: () => Promise<void>
  init: () => Promise<void>
}

let tauriStorePromise: Promise<TauriStore> | null = null
const getTauriStore = (): Promise<TauriStore> => {
  if (!tauriStorePromise) {
    tauriStorePromise = TauriStore.load("settings.json")
  }
  return tauriStorePromise
}

const persist = async (files: string[]) => {
  try {
    const store = await getTauriStore()
    await store.set(STORE_KEY, files)
    await store.save()
  } catch (e) {
    console.warn("[recentFiles] persist failed:", e)
  }
}

export const useRecentFilesStore = create<RecentFilesState>((set, get) => ({
  files: [],
  add: async (filePath) => {
    const current = get().files.filter((p) => p !== filePath)
    const next = [filePath, ...current].slice(0, MAX_RECENT)
    set({ files: next })
    await persist(next)
  },
  remove: async (filePath) => {
    const next = get().files.filter((p) => p !== filePath)
    set({ files: next })
    await persist(next)
  },
  clear: async () => {
    set({ files: [] })
    await persist([])
  },
  init: async () => {
    try {
      const store = await getTauriStore()
      const stored = await store.get<string[]>(STORE_KEY)
      if (Array.isArray(stored)) set({ files: stored.slice(0, MAX_RECENT) })
    } catch (e) {
      console.warn("[recentFiles] load failed:", e)
    }
  }
}))
