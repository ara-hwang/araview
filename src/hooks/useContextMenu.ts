import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu"
import { useCallback } from "react"
import type { DirectoryImages } from "../types"
import {
  flipHorizontal,
  flipVertical,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  zoomIn,
  zoomOut
} from "@/store/appStore"
import { cycleViewerBackground } from "@/store/settingsStore"
import { toggleShuffleAndRefresh } from "@/utils/directoryOptions"
import i18n from "@/i18n"

export type ImageViewerContextMenuActions = {
  onOpenFile: () => void
  onCloseImage: () => void
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onToggleExif: () => void
  onToggleSlideshow: () => void
  onToggleFullscreen: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onSaveEdits: () => void
  onToggleFavorite: () => void
}

export async function showImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions
): Promise<void> {
  const t = i18n.t.bind(i18n)
  const navItems =
    dirImages && dirImages.images.length > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          MenuItem.new({
            text: t("menu.prev"),
            accelerator: "Left",
            action: () => actions.onNavigatePrev()
          }),
          MenuItem.new({
            text: t("menu.next"),
            accelerator: "Right",
            action: () => actions.onNavigateNext()
          })
        ]
      : []

  const [openItem, closeItem, ...rest] = await Promise.all([
    MenuItem.new({
      text: t("menu.open"),
      accelerator: "Ctrl+O",
      action: () => actions.onOpenFile()
    }),
    MenuItem.new({
      text: t("menu.closeImage"),
      accelerator: "Esc",
      action: () => actions.onCloseImage()
    }),
    ...navItems,
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.zoomIn"),
      accelerator: "=",
      action: () => zoomIn()
    }),
    MenuItem.new({
      text: t("menu.zoomOut"),
      accelerator: "-",
      action: () => zoomOut()
    }),
    MenuItem.new({
      text: t("menu.actualSize"),
      accelerator: "0",
      action: () => resetZoomPan()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.fitWidth"),
      accelerator: "1",
      action: () => setZoomToFit("width")
    }),
    MenuItem.new({
      text: t("menu.fitHeight"),
      accelerator: "2",
      action: () => setZoomToFit("height")
    }),
    MenuItem.new({
      text: t("menu.fitScreen"),
      accelerator: "3",
      action: () => setZoomToFit("screen")
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.rotateCw"),
      accelerator: "R",
      action: () => rotateCW()
    }),
    MenuItem.new({
      text: t("menu.rotateCcw"),
      accelerator: "Shift+R",
      action: () => rotateCCW()
    }),
    MenuItem.new({
      text: t("menu.flipH"),
      accelerator: "H",
      action: () => flipHorizontal()
    }),
    MenuItem.new({
      text: t("menu.flipV"),
      accelerator: "V",
      action: () => flipVertical()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.toggleExif"),
      accelerator: "I",
      action: () => actions.onToggleExif()
    }),
    MenuItem.new({
      text: t("menu.toggleSlideshow"),
      accelerator: "Space",
      action: () => actions.onToggleSlideshow()
    }),
    MenuItem.new({
      text: t("menu.toggleFullscreen"),
      accelerator: "F11",
      action: () => actions.onToggleFullscreen()
    }),
    MenuItem.new({
      text: t("menu.copyImage"),
      accelerator: "Ctrl+C",
      action: () => actions.onCopyImage()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.cycleBg"),
      accelerator: "B",
      action: () => cycleViewerBackground()
    }),
    MenuItem.new({
      text: t("menu.toggleShuffle"),
      accelerator: "S",
      action: () => toggleShuffleAndRefresh()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.trash"),
      accelerator: "Del",
      action: () => actions.onTrashFile()
    }),
    MenuItem.new({
      text: t("menu.reveal"),
      accelerator: "Ctrl+Shift+E",
      action: () => actions.onRevealInExplorer()
    }),
    MenuItem.new({
      text: t("menu.openExternal"),
      accelerator: "Ctrl+Shift+O",
      action: () => actions.onOpenExternal()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.rename"),
      accelerator: "F2",
      action: () => actions.onRenameFile()
    }),
    MenuItem.new({
      text: t("menu.copyPath"),
      accelerator: "Ctrl+Shift+C",
      action: () => actions.onCopyPath()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.saveEdits"),
      accelerator: "Ctrl+S",
      action: () => actions.onSaveEdits()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.toggleFavorite"),
      accelerator: "F",
      action: () => actions.onToggleFavorite()
    })
  ])

  const menu = await Menu.new({ items: [openItem, closeItem, ...rest] })
  await menu.popup()
}

export function useImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions
) {
  return useCallback(
    async (e: React.MouseEvent) => {
      e.preventDefault()
      await showImageViewerContextMenu(dirImages, actions)
    },
    [dirImages, actions]
  )
}
