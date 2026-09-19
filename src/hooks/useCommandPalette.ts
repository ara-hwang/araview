import { useEffect, useMemo } from "react"
import { useTranslation } from "react-i18next"
import { create } from "zustand"
import { useShallow } from "zustand/react/shallow"

import {
  COMMAND_DEFS,
  COMMAND_GROUP_ORDER,
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
import { WEBTOON_KEY_SCROLL_PX, scrollWebtoonBy } from "@/hooks/useImageViewerHotkeys"
import { requestUpdateCheck } from "@/hooks/useUpdater"
import {
  flipHorizontal,
  flipVertical,
  panDown,
  panLeft,
  panRight,
  panUp,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  useAppStore,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import { usePaletteMruStore } from "@/store/paletteMruStore"
import { cycleViewerBackground, getSettings, useSettingsStore } from "@/store/settingsStore"

export const OPEN_SETTINGS_EVENT = "tiv:open-settings"

export function requestOpenSettings() {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT))
}

/** 뷰어 페이지가 등록하는 이미지 의존 핸들러. 홈에서는 비활성이라 불필요. */
export type PaletteViewerHandlers = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onJumpPrev10: () => void
  onJumpNext10: () => void
  onJumpFirst: () => void
  onJumpLast: () => void
  onToggleExif: () => void
  onToggleSlideshow: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onSaveEdits: () => void
  onToggleGrid: () => void
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

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true
  if (target.isContentEditable) return true
  return false
}

function runCommand(
  id: CommandId,
  host: {
    openFile: () => void
    closeImage: () => void
    toggleFullscreen: () => void
    toggleAlwaysOnTop: () => void
  }
) {
  switch (id) {
    case "openFile":
      host.openFile()
      break
    case "closeImage":
      host.closeImage()
      break
    case "navigatePrev":
      viewerHandlers?.onNavigatePrev()
      break
    case "navigateNext":
      viewerHandlers?.onNavigateNext()
      break
    case "jumpPrev10":
      viewerHandlers?.onJumpPrev10()
      break
    case "jumpNext10":
      viewerHandlers?.onJumpNext10()
      break
    case "jumpFirst":
      viewerHandlers?.onJumpFirst()
      break
    case "jumpLast":
      viewerHandlers?.onJumpLast()
      break
    case "zoomIn":
      zoomIn()
      break
    case "zoomOut":
      zoomOut()
      break
    case "panLeft":
      if (getSettings().viewMode === "webtoon") viewerHandlers?.onNavigatePrev()
      else panLeft()
      break
    case "panRight":
      if (getSettings().viewMode === "webtoon") viewerHandlers?.onNavigateNext()
      else panRight()
      break
    case "panUp":
      if (getSettings().viewMode === "webtoon") scrollWebtoonBy(-WEBTOON_KEY_SCROLL_PX)
      else panUp()
      break
    case "panDown":
      if (getSettings().viewMode === "webtoon") scrollWebtoonBy(WEBTOON_KEY_SCROLL_PX)
      else panDown()
      break
    case "resetView":
      resetZoomPan()
      break
    case "fitWidth":
      setZoomToFit("width")
      break
    case "fitHeight":
      setZoomToFit("height")
      break
    case "fitScreen":
      setZoomToFit("screen")
      break
    case "rotateCW":
      rotateCW()
      break
    case "rotateCCW":
      rotateCCW()
      break
    case "flipH":
      flipHorizontal()
      break
    case "flipV":
      flipVertical()
      break
    case "toggleExif":
      viewerHandlers?.onToggleExif()
      break
    case "toggleSlideshow":
      viewerHandlers?.onToggleSlideshow()
      break
    case "toggleFullscreen":
      void host.toggleFullscreen()
      break
    case "toggleAlwaysOnTop":
      void host.toggleAlwaysOnTop()
      break
    case "copyImage":
      viewerHandlers?.onCopyImage()
      break
    case "trashFile":
      viewerHandlers?.onTrashFile()
      break
    case "revealInExplorer":
      viewerHandlers?.onRevealInExplorer()
      break
    case "openExternal":
      viewerHandlers?.onOpenExternal()
      break
    case "cycleBackground":
      cycleViewerBackground()
      break
    case "renameFile":
      viewerHandlers?.onRenameFile()
      break
    case "copyPath":
      viewerHandlers?.onCopyPath()
      break
    case "saveEdits":
      viewerHandlers?.onSaveEdits()
      break
    case "toggleGrid":
      viewerHandlers?.onToggleGrid()
      break
    case "openSettings":
      requestOpenSettings()
      break
    case "checkForUpdates":
      requestUpdateCheck()
      break
    case "togglePalette":
      break
  }
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
    () => ({ hasImage, canNavigate: imageCount > 1 }),
    [hasImage, imageCount]
  )

  const commands: ResolvedPaletteCommand[] = useMemo(() => {
    const tx = t as unknown as (key: string) => string
    return COMMAND_DEFS.map((def) => ({
      def,
      label: tx(def.labelKey),
      shortcutLabel: def.shortcutId ? formatShortcutDisplay(shortcuts[def.shortcutId] ?? "") : "",
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
      openFile: handleOpenFile,
      closeImage: closeAndGoHome,
      toggleFullscreen: () => void fullscreen.toggle(),
      toggleAlwaysOnTop: () => void toggleAlwaysOnTop()
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
