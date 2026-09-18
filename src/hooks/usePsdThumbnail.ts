import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useState } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import type { PsdThumbStatus } from "@/types"
import { errorMessage } from "@/utils/appError"

export function usePsdThumbnail(enabled: boolean) {
  const [status, setStatus] = useState<PsdThumbStatus | null>(null)
  const [loading, setLoading] = useState(false)
  const [pending, setPending] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const next = await invoke<PsdThumbStatus>("get_psd_thumbnail_status")
      setStatus(next)
    } catch (error) {
      toast.error(i18n.t("toast.thumb.loadFail"), {
        description: errorMessage(error)
      })
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!enabled) return
    void refresh()
  }, [enabled, refresh])

  const setThumbnail = useCallback(async (register: boolean) => {
    setPending(true)
    try {
      const next = await invoke<PsdThumbStatus>(
        register ? "register_psd_thumbnail" : "unregister_psd_thumbnail"
      )
      setStatus(next)
      toast.success(i18n.t(register ? "toast.thumb.doneEnabled" : "toast.thumb.doneDisabled"))
    } catch (error) {
      toast.error(i18n.t(register ? "toast.thumb.registerFail" : "toast.thumb.unregisterFail"), {
        description: errorMessage(error)
      })
    } finally {
      setPending(false)
    }
  }, [])

  return { status, loading, pending, refresh, setThumbnail }
}
