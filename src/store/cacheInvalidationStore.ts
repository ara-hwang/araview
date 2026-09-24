import { create } from "zustand"

type CacheInvalidationState = {
  epoch: number
}

type CacheInvalidationActions = {
  invalidate: () => void
}

export const useCacheInvalidationStore = create<CacheInvalidationState & CacheInvalidationActions>(
  (set) => ({
    epoch: 0,
    invalidate: () => set((state) => ({ epoch: state.epoch + 1 }))
  })
)
