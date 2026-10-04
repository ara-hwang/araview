import { useEffect, useMemo } from "react"
import { useTranslation } from "react-i18next"
import { create } from "zustand"
import { useShallow } from "zustand/react/shallow"

import {
  COMMAND_DEFS,
  COMMAND_GROUP_ORDER,
  commandShortcutId,
  filterCommands,
  isCommandEnabled,
  type CommandContext,
  type CommandDef,
  type CommandId
} from "@/constants/commands"
import { eventToBinding, formatShortcutDisplay } from "@/constants/shortcuts"
import { useAlwaysOnTop } from "@/hooks/useAlwaysOnTop"
import { useCloseImage } from "@/hooks/useCloseImage"
import { useFullscreen } from "@/hooks/useFullscreen"
import { useImageLoader } from "@/hooks/useImageLoader"
import { runViewerCommand, type ViewerActionHandlers } from "@/hooks/viewerActions"
import { useAppStore } from "@/store/appStore"
import { useGifStore } from "@/store/gifStore"
import { usePaletteMruStore } from "@/store/paletteMruStore"
import { getSettings, useSettingsStore } from "@/store/settingsStore"
import { isEditableTarget } from "@/utils/editableTarget"

export { OPEN_SETTINGS_EVENT, requestOpenSettings } from "@/hooks/viewerActions"

/** 팔레트 호스트가 직접 처리하는 동작. 홈에서도 동작해야 해서 뷰어 등록과 분리한다. */
type PaletteHostHandlers = Pick<
  ViewerActionHandlers,
  "onOpenFile" | "onCloseImage" | "onToggleFullscreen" | "onToggleAlwaysOnTop"
>

/** 뷰어 페이지가 등록하는 이미지 의존 핸들러. 홈에서는 등록되지 않는다. */
export type PaletteViewerHandlers = Omit<ViewerActionHandlers, keyof PaletteHostHandlers>

const noop = () => {}

/** 뷰어 핸들러가 없을 때(홈)의 기본값. 이미지 의존 명령은 아무 일도 하지 않는다. */
const NOOP_VIEWER_HANDLERS: PaletteViewerHandlers = {
  onNavigatePrev: noop,
  onNavigateNext: noop,
  onJumpPrev10: noop,
  onJumpNext10: noop,
  onJumpFirst: noop,
  onJumpLast: noop,
  onToggleExif: noop,
  onCopyImage: noop,
  onTrashFile: noop,
  onRevealInExplorer: noop,
  onOpenExternal: noop,
  onRenameFile: noop,
  onCopyPath: noop,
  onToggleGrid: noop,
  onToggleDock: noop,
  onToggleComicCover: noop
}

let viewerHandlers: PaletteViewerHandlers | null = null

export function registerPaletteHandlers(handlers: PaletteViewerHandlers) {
  viewerHandlers = handlers
}

export function unregisterPaletteHandlers() {
  viewerHandlers = null
}

type PaletteStore = {
  open: boolean
  query: string
  activeIndex: number
  setOpen: (open: boolean) => void
  setQuery: (query: string) => void
  setActiveIndex: (index: number) => void
  toggle: () => void
}

export const usePaletteStore = create<PaletteStore>((set) => ({
  open: false,
  query: "",
  activeIndex: 0,
  setOpen: (open) => set(() => (open ? { open } : { open, query: "", activeIndex: 0 })),
  setQuery: (query) => set(() => ({ query, activeIndex: 0 })),
  setActiveIndex: (activeIndex) => set(() => ({ activeIndex })),
  toggle: () =>
    set((state) => (state.open ? { open: false, query: "", activeIndex: 0 } : { open: true }))
}))

export type ResolvedPaletteCommand = {
  def: CommandDef
  label: string
  shortcutLabel: string
  enabled: boolean
}

function runCommand(id: CommandId, host: PaletteHostHandlers) {
  runViewerCommand(id, { ...(viewerHandlers ?? NOOP_VIEWER_HANDLERS), ...host })
}

/**
 * __root에 마운트되는 호스트. 전역 토글 리스너와 명령 해결을 담당한다.
 * 토글은 설정된 바인딩을 그대로 따르므로 단축키 커스텀과 자동 연동된다.
 */
export function useCommandPaletteHost() {
  const { t } = useTranslation()
  const { open, query, activeIndex } = usePaletteStore(
    useShallow((s) => ({
      open: s.open,
      query: s.query,
      activeIndex: s.activeIndex
    }))
  )
  const shortcuts = useSettingsStore((s) => s.shortcuts)
  const hasImage = useAppStore((s) => s.imageInfo !== null)
  const imageCount = useAppStore((s) => s.dirImages.images.length)
  const isGif = useGifStore((s) => s.active && s.frameCount > 1)
  const inArchive = useAppStore((s) => s.archivePath !== null)
  const mruIds = usePaletteMruStore((s) => s.ids)

  const { handleOpenFile } = useImageLoader()
  const closeAndGoHome = useCloseImage()
  const fullscreen = useFullscreen()
  const { toggle: toggleAlwaysOnTop } = useAlwaysOnTop()

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const toggleBinding = getSettings().shortcuts.togglePalette
      if (!toggleBinding) return
      if (eventToBinding(e) !== toggleBinding) return
      if (!usePaletteStore.getState().open && isEditableTarget(e.target)) {
        return
      }
      e.preventDefault()
      usePaletteStore.getState().toggle()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => {
      window.removeEventListener("keydown", onKeyDown)
    }
  }, [])

  const ctx: CommandContext = useMemo(
    () => ({ hasImage, canNavigate: imageCount > 1, isGif, inArchive }),
    [hasImage, imageCount, isGif, inArchive]
  )

  const commands: ResolvedPaletteCommand[] = useMemo(() => {
    const tx = t as unknown as (key: string) => string
    const shortcutLabelOf = (def: CommandDef) => {
      const shortcutId = commandShortcutId(def)
      return shortcutId ? formatShortcutDisplay(shortcuts[shortcutId] ?? "") : ""
    }
    return COMMAND_DEFS.map((def) => ({
      def,
      label: tx(def.labelKey),
      shortcutLabel: shortcutLabelOf(def),
      enabled: isCommandEnabled(def, ctx)
    }))
  }, [t, shortcuts, ctx])

  const { filtered, recentCount } = useMemo(() => {
    const searched = filterCommands(
      commands.map((c) => ({
        ...c,
        keywords: c.def.keywords ?? []
      })),
      query
    )
    // 검색어가 비었을 때만 최근 사용을 상단에 올린다. 검색 중에는 관련도 순 유지.
    if (query.trim() !== "") return { filtered: searched, recentCount: 0 }
    const byId = new Map(searched.map((c) => [c.def.id, c]))
    const recent: typeof searched = []
    for (const id of mruIds) {
      const hit = byId.get(id)
      if (hit) {
        recent.push(hit)
        byId.delete(id)
      }
    }
    if (recent.length === 0) return { filtered: searched, recentCount: 0 }
    return {
      filtered: [...recent, ...byId.values()],
      recentCount: recent.length
    }
  }, [commands, query, mruIds])

  const safeActiveIndex = Math.min(activeIndex, Math.max(filtered.length - 1, 0))

  const run = (id: CommandId) => {
    runCommand(id, {
      onOpenFile: () => void handleOpenFile(),
      onCloseImage: closeAndGoHome,
      onToggleFullscreen: () => void fullscreen.toggle(),
      onToggleAlwaysOnTop: () => void toggleAlwaysOnTop()
    })
    if (id !== "togglePalette") void usePaletteMruStore.getState().push(id)
    usePaletteStore.getState().setOpen(false)
  }

  return {
    open,
    query,
    filtered,
    recentCount,
    activeIndex: safeActiveIndex,
    run
  }
}

export { COMMAND_GROUP_ORDER }
