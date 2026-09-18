import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useCallback, useEffect, useState } from "react"

import { toast } from "@/components/ui/toast"
import i18n from "@/i18n"
import type { FileAssociation } from "@/types"
import { errorCopyDetails, errorMessage } from "@/utils/appError"

export function hasBlockedAssociation(items: FileAssociation[]): boolean {
  return items.some((item) => item.needs_os_confirmation && !item.associated)
}

export function useFileAssociations(enabled: boolean) {
  const [items, setItems] = useState<FileAssociation[]>([])
  const [loading, setLoading] = useState(false)
  const [pendingExtension, setPendingExtension] = useState<string | null>(null)
  const [pendingAll, setPendingAll] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const next = await invoke<FileAssociation[]>("get_file_associations")
      setItems(next)
    } catch (error) {
      toast.error(i18n.t("toast.assoc.loadFail"), {
        description: errorMessage(error),
        details: errorCopyDetails(error)
      })
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!enabled) return
    void refresh()

    let cancelled = false
    let unlisten: (() => void) | undefined
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) void refresh()
      })
      .then((fn) => {
        if (cancelled) {
          fn()
          return
        }
        unlisten = fn
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [enabled, refresh])

  const setAssociation = useCallback(async (extension: string, associate: boolean) => {
    setPendingExtension(extension)
    try {
      const next = await invoke<FileAssociation>("set_file_association", {
        extension,
        associate
      })
      setItems((prev) => prev.map((item) => (item.extension === extension ? next : item)))
    } catch (error) {
      toast.error(i18n.t("toast.assoc.openFail"), {
        description: errorMessage(error),
        details: errorCopyDetails(error, `*.${extension}`)
      })
    } finally {
      setPendingExtension(null)
    }
  }, [])

  const setAllAssociations = useCallback(async (associate: boolean) => {
    setPendingAll(true)
    try {
      const next = await invoke<FileAssociation[]>("set_all_file_associations", {
        associate
      })
      setItems(next)
      toast.info(i18n.t("toast.assoc.pickInfo"))
    } catch (error) {
      toast.error(i18n.t("toast.assoc.settingsFail"), {
        description: errorMessage(error),
        details: errorCopyDetails(error)
      })
    } finally {
      setPendingAll(false)
    }
  }, [])

  const openDefaultAppsSettings = useCallback(async () => {
    try {
      await invoke("open_default_apps_settings")
    } catch (error) {
      toast.error(i18n.t("toast.assoc.settingsFail"), {
        description: errorMessage(error),
        details: errorCopyDetails(error)
      })
    }
  }, [])

  return {
    items,
    loading,
    pendingExtension,
    pendingAll,
    setAssociation,
    setAllAssociations,
    openDefaultAppsSettings
  }
}
