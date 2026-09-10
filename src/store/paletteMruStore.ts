import { create } from "zustand"
import { Store as TauriStore } from "@tauri-apps/plugin-store"
import { COMMAND_DEFS, type CommandId } from "@/constants/commands"

export const MAX_PALETTE_MRU = 5
const STORE_KEY = "paletteMru"

const VALID_IDS: ReadonlySet<string> = new Set(
  COMMAND_DEFS.map((def) => def.id)
)

function sanitizeIds(value: unknown): CommandId[] {
  if (!Array.isArray(value)) return []
  const out: CommandId[] = []
  for (const entry of value) {
    if (typeof entry === "string" && VALID_IDS.has(entry)) {
      const id = entry as CommandId
      if (!out.includes(id)) out.push(id)
    }
    if (out.length >= MAX_PALETTE_MRU) break
  }
  return out
}

type PaletteMruState = {
  ids: CommandId[]
  push: (id: CommandId) => Promise<void>
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

const persist = async (ids: CommandId[]) => {
  try {
    const store = await getTauriStore()
    await store.set(STORE_KEY, ids)
    await store.save()
  } catch (e) {
    console.warn("[paletteMru] persist failed:", e)
  }
}

export const usePaletteMruStore = create<PaletteMruState>((set, get) => ({
  ids: [],
  push: async (id) => {
    if (!VALID_IDS.has(id)) return
    const next = [id, ...get().ids.filter((entry) => entry !== id)].slice(
      0,
      MAX_PALETTE_MRU
    )
    set({ ids: next })
    await persist(next)
  },
  clear: async () => {
    set({ ids: [] })
    await persist([])
  },
  init: async () => {
    try {
      const store = await getTauriStore()
      const stored = await store.get<unknown>(STORE_KEY)
      set({ ids: sanitizeIds(stored) })
    } catch (e) {
      console.warn("[paletteMru] load failed:", e)
    }
  }
}))
