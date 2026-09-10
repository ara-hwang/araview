import { useEffect, useMemo } from "react"
import { create } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { useTranslation } from "react-i18next"
import {
  flipHorizontal,
  flipVertical,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  useAppStore,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import {
  cycleViewerBackground,
  getSettings,
  useSettingsStore
} from "@/store/settingsStore"
import { eventToBinding, formatShortcutDisplay } from "@/constants/shortcuts"
import {
  COMMAND_DEFS,
  COMMAND_GROUP_ORDER,
  filterCommands,
  isCommandEnabled,
  type CommandContext,
  type CommandDef,
  type CommandId
} from "@/constants/commands"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useCloseImage } from "@/hooks/useCloseImage"
import { useFullscreen } from "@/hooks/useFullscreen"
import { useAlwaysOnTop } from "@/hooks/useAlwaysOnTop"
import { toggleShuffleAndRefresh } from "@/utils/directoryOptions"

export const OPEN_SETTINGS_EVENT = "tiv:open-settings"

export function requestOpenSettings() {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT))
}

/** 뷰어 페이지가 등록하는 이미지 의존 핸들러. 홈에서는 비활성이라 불필요. */
export type PaletteViewerHandlers = {
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onToggleExif: () => void
  onToggleSlideshow: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onSaveEdits: () => void
  onToggleFavorite: () => void
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
  setOpen: (open) =>
    set(() => (open ? { open } : { open, query: "", activeIndex: 0 })),
  setQuery: (query) => set(() => ({ query, activeIndex: 0 })),
  setActiveIndex: (activeIndex) => set(() => ({ activeIndex })),
  toggle: () =>
    set((state) =>
      state.open ? { open: false, query: "", activeIndex: 0 } : { open: true }
    )
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
    case "zoomIn":
      zoomIn()
      break
    case "zoomOut":
      zoomOut()
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
    case "toggleShuffle":
      void toggleShuffleAndRefresh()
      break
    case "saveEdits":
      viewerHandlers?.onSaveEdits()
      break
    case "toggleFavorite":
      viewerHandlers?.onToggleFavorite()
      break
    case "openSettings":
      requestOpenSettings()
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
      shortcutLabel: def.shortcutId
        ? formatShortcutDisplay(shortcuts[def.shortcutId] ?? "")
        : "",
      enabled: isCommandEnabled(def, ctx)
    }))
  }, [t, shortcuts, ctx])

  const filtered = useMemo(
    () =>
      filterCommands(
        commands.map((c) => ({
          ...c,
          keywords: c.def.keywords ?? []
        })),
        query
      ),
    [commands, query]
  )

  const safeActiveIndex = Math.min(
    activeIndex,
    Math.max(filtered.length - 1, 0)
  )

  const run = (id: CommandId) => {
    runCommand(id, {
      openFile: handleOpenFile,
      closeImage: closeAndGoHome,
      toggleFullscreen: () => void fullscreen.toggle(),
      toggleAlwaysOnTop: () => void toggleAlwaysOnTop()
    })
    usePaletteStore.getState().setOpen(false)
  }

  return { open, query, filtered, activeIndex: safeActiveIndex, run }
}

export { COMMAND_GROUP_ORDER }
