import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu"
import { useCallback } from "react"

import { toTauriAccelerator } from "@/constants/shortcuts"
import i18n from "@/i18n"
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
import { cycleViewerBackground, getSettings } from "@/store/settingsStore"

import type { DirectoryImages } from "../types"

export type ImageViewerContextMenuActions = {
  onOpenFile: () => void
  onCloseImage: () => void
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onToggleExif: () => void
  onToggleFullscreen: () => void
  onToggleAlwaysOnTop: () => void
  onCopyImage: () => void
  onTrashFile: () => void
  onRevealInExplorer: () => void
  onOpenExternal: () => void
  onRenameFile: () => void
  onCopyPath: () => void
  onSaveEdits: () => void
  onToggleGrid: () => void
}

export async function showImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions
): Promise<void> {
  const t = i18n.t.bind(i18n)
  const s = getSettings().shortcuts
  const acc = (binding: string) => toTauriAccelerator(binding)
  const navItems =
    dirImages && dirImages.images.length > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          MenuItem.new({
            text: t("menu.prev"),
            accelerator: acc(s.navigatePrev),
            action: () => actions.onNavigatePrev()
          }),
          MenuItem.new({
            text: t("menu.next"),
            accelerator: acc(s.navigateNext),
            action: () => actions.onNavigateNext()
          })
        ]
      : []

  const [openItem, closeItem, ...rest] = await Promise.all([
    MenuItem.new({
      text: t("menu.open"),
      accelerator: acc(s.openFile),
      action: () => actions.onOpenFile()
    }),
    MenuItem.new({
      text: t("menu.closeImage"),
      accelerator: acc(s.closeImage),
      action: () => actions.onCloseImage()
    }),
    ...navItems,
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.zoomIn"),
      accelerator: acc(s.zoomIn),
      action: () => zoomIn()
    }),
    MenuItem.new({
      text: t("menu.zoomOut"),
      accelerator: acc(s.zoomOut),
      action: () => zoomOut()
    }),
    MenuItem.new({
      text: t("menu.actualSize"),
      accelerator: acc(s.resetView),
      action: () => resetZoomPan()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.fitWidth"),
      accelerator: acc(s.fitWidth),
      action: () => setZoomToFit("width")
    }),
    MenuItem.new({
      text: t("menu.fitHeight"),
      accelerator: acc(s.fitHeight),
      action: () => setZoomToFit("height")
    }),
    MenuItem.new({
      text: t("menu.fitScreen"),
      accelerator: acc(s.fitScreen),
      action: () => setZoomToFit("screen")
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.rotateCw"),
      accelerator: acc(s.rotateCW),
      action: () => rotateCW()
    }),
    MenuItem.new({
      text: t("menu.rotateCcw"),
      accelerator: acc(s.rotateCCW),
      action: () => rotateCCW()
    }),
    MenuItem.new({
      text: t("menu.flipH"),
      accelerator: acc(s.flipH),
      action: () => flipHorizontal()
    }),
    MenuItem.new({
      text: t("menu.flipV"),
      accelerator: acc(s.flipV),
      action: () => flipVertical()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.toggleExif"),
      accelerator: acc(s.toggleExif),
      action: () => actions.onToggleExif()
    }),
    MenuItem.new({
      text: t("menu.toggleGrid"),
      accelerator: acc(s.toggleGrid),
      action: () => actions.onToggleGrid()
    }),
    MenuItem.new({
      text: t("menu.toggleFullscreen"),
      accelerator: acc(s.toggleFullscreen),
      action: () => actions.onToggleFullscreen()
    }),
    MenuItem.new({
      text: t("menu.toggleAlwaysOnTop"),
      accelerator: acc(s.toggleAlwaysOnTop),
      action: () => actions.onToggleAlwaysOnTop()
    }),
    MenuItem.new({
      text: t("menu.copyImage"),
      accelerator: acc(s.copyImage),
      action: () => actions.onCopyImage()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.cycleBg"),
      accelerator: acc(s.cycleBackground),
      action: () => cycleViewerBackground()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.trash"),
      accelerator: acc(s.trashFile),
      action: () => actions.onTrashFile()
    }),
    MenuItem.new({
      text: t("menu.reveal"),
      accelerator: acc(s.revealInExplorer),
      action: () => actions.onRevealInExplorer()
    }),
    MenuItem.new({
      text: t("menu.openExternal"),
      accelerator: acc(s.openExternal),
      action: () => actions.onOpenExternal()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.rename"),
      accelerator: acc(s.renameFile),
      action: () => actions.onRenameFile()
    }),
    MenuItem.new({
      text: t("menu.copyPath"),
      accelerator: acc(s.copyPath),
      action: () => actions.onCopyPath()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: t("menu.saveEdits"),
      accelerator: acc(s.saveEdits),
      action: () => actions.onSaveEdits()
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
