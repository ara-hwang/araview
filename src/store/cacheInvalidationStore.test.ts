import { beforeEach, describe, expect, it } from "vitest"

import { useCacheInvalidationStore } from "@/store/cacheInvalidationStore"

beforeEach(() => {
  useCacheInvalidationStore.setState({ epoch: 0 })
})

describe("useCacheInvalidationStore", () => {
  it("무효화할 때마다 epoch를 증가시킨다", () => {
    const first = useCacheInvalidationStore.getState().epoch
    useCacheInvalidationStore.getState().invalidate()
    useCacheInvalidationStore.getState().invalidate()
    expect(useCacheInvalidationStore.getState().epoch).toBe(first + 2)
  })
})
