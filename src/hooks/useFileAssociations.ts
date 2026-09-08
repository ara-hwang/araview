import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { toast } from "sonner"
import type { FileAssociation } from "@/types"

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
      toast.error("확장자 연결 상태를 불러오지 못했습니다", {
        description: String(error)
      })
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!enabled) return
    void refresh()
  }, [enabled, refresh])

  const setAssociation = useCallback(
    async (extension: string, associate: boolean) => {
      setPendingExtension(extension)
      try {
        const next = await invoke<FileAssociation>("set_file_association", {
          extension,
          associate
        })
        setItems((prev) =>
          prev.map((item) => (item.extension === extension ? next : item))
        )
        if (associate && !next.associated) {
          toast.warning("Windows에서 기본 앱 확인이 필요합니다")
        }
      } catch (error) {
        toast.error("확장자 연결에 실패했습니다", {
          description: String(error)
        })
      } finally {
        setPendingExtension(null)
      }
    },
    []
  )

  const setAllAssociations = useCallback(async (associate: boolean) => {
    setPendingAll(true)
    try {
      const next = await invoke<FileAssociation[]>(
        "set_all_file_associations",
        {
          associate
        }
      )
      setItems(next)
      if (associate && hasBlockedAssociation(next)) {
        toast.warning(
          "일부 확장자는 Windows 기본 앱 설정에서 확인이 필요합니다"
        )
      }
    } catch (error) {
      toast.error("확장자 연결에 실패했습니다", { description: String(error) })
    } finally {
      setPendingAll(false)
    }
  }, [])

  const openDefaultAppsSettings = useCallback(async () => {
    try {
      await invoke("open_default_apps_settings")
    } catch (error) {
      toast.error("Windows 설정을 열지 못했습니다", {
        description: String(error)
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
