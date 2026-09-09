import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"

const MAX_FAVORITES = 50
const STORE_KEY = "favoriteFiles"

type FavoritesState = {
  files: string[]
  isFavorite: (filePath: string) => boolean
  add: (filePath: string) => Promise<void>
  toggle: (filePath: string) => Promise<boolean>
  remove: (filePath: string) => Promise<void>
  /** 이름 변경 시 순서 유지하며 경로 교체 */
  replace: (oldPath: string, newPath: string) => Promise<void>
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
    console.warn("[favorites] persist failed:", e)
  }
}

export const useFavoritesStore = create<FavoritesState>((set, get) => ({
  files: [],
  isFavorite: (filePath) => get().files.includes(filePath),
  add: async (filePath) => {
    const current = get().files.filter((p) => p !== filePath)
    const next = [filePath, ...current].slice(0, MAX_FAVORITES)
    set({ files: next })
    await persist(next)
  },
  toggle: async (filePath) => {
    if (get().files.includes(filePath)) {
      const next = get().files.filter((p) => p !== filePath)
      set({ files: next })
      await persist(next)
      return false
    }
    await get().add(filePath)
    return true
  },
  remove: async (filePath) => {
    const next = get().files.filter((p) => p !== filePath)
    set({ files: next })
    await persist(next)
  },
  replace: async (oldPath, newPath) => {
    const next = get().files.map((p) => (p === oldPath ? newPath : p))
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
      if (Array.isArray(stored)) set({ files: stored.slice(0, MAX_FAVORITES) })
    } catch (e) {
      console.warn("[favorites] load failed:", e)
    }
  }
}))
