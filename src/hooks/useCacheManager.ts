import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useRef, useState } from "react"

import { useCacheInvalidationStore } from "@/store/cacheInvalidationStore"
import type { CacheClearResult, CacheScope, CacheStats } from "@/types"
import { classifyError, type ClassifiedError } from "@/utils/appError"
import { clearImageMetaCache } from "@/utils/imageMetaCache"

export function useCacheManager() {
  const [stats, setStats] = useState<CacheStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [clearingScope, setClearingScope] = useState<CacheScope | null>(null)
  const [error, setError] = useState<ClassifiedError | null>(null)
  const requestIdRef = useRef(0)
  const mountedRef = useRef(true)

  const refresh = useCallback(async (): Promise<CacheStats | null> => {
    const requestId = ++requestIdRef.current
    setLoading(true)
    setError(null)
    try {
      const next = await invoke<CacheStats>("get_cache_stats")
      if (!mountedRef.current || requestId !== requestIdRef.current) return null
      setStats(next)
      return next
    } catch (e) {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setError(classifyError(e))
      }
      return null
    } finally {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setLoading(false)
      }
    }
  }, [])

  const clear = useCallback(async (scope: CacheScope): Promise<CacheClearResult> => {
    setClearingScope(scope)
    setError(null)
    try {
      const result = await invoke<CacheClearResult>("clear_cache", { scope })
      clearImageMetaCache()
      useCacheInvalidationStore.getState().invalidate()
      if (mountedRef.current) {
        setStats(result.stats)
      }
      return result
    } catch (e) {
      if (mountedRef.current) setError(classifyError(e))
      throw e
    } finally {
      if (mountedRef.current) setClearingScope(null)
    }
  }, [])

  useEffect(() => {
    mountedRef.current = true
    void refresh()
    return () => {
      mountedRef.current = false
      requestIdRef.current += 1
    }
  }, [refresh])

  return {
    stats,
    loading,
    clearingScope,
    error,
    refresh,
    clear
  }
}
