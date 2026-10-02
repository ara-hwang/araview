import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu"
import { useCallback } from "react"

import type { CommandId } from "@/constants/commands"
import { toTauriAccelerator } from "@/constants/shortcuts"
import { runViewerCommand, type ViewerActionHandlers } from "@/hooks/viewerActions"
import i18n from "@/i18n"
import { useGifStore } from "@/store/gifStore"
import { getSettings } from "@/store/settingsStore"

import type { DirectoryImages } from "../types"

export type ImageViewerContextMenuActions = ViewerActionHandlers

export async function showImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions
): Promise<void> {
  const t = i18n.t.bind(i18n)
  const s = getSettings().shortcuts
  const acc = (binding: string) => toTauriAccelerator(binding)
  const item = (text: string, id: CommandId, binding?: string) =>
    MenuItem.new({
      text,
      accelerator: binding === undefined ? undefined : acc(binding),
      action: () => void runViewerCommand(id, actions)
    })
  const navItems =
    dirImages && dirImages.images.length > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          item(t("menu.prev"), "navigatePrev", s.navigatePrev),
          item(t("menu.next"), "navigateNext", s.navigateNext)
        ]
      : []

  const gif = useGifStore.getState()
  const gifItems =
    gif.active && gif.frameCount > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          item(t("menu.gifPlayPause"), "toggleGifPlayback", s.toggleGifPlayback),
          item(t("menu.gifPrevFrame"), "gifPrevFrame", s.gifPrevFrame),
          item(t("menu.gifNextFrame"), "gifNextFrame", s.gifNextFrame)
        ]
      : []

  const [openItem, closeItem, ...rest] = await Promise.all([
    item(t("menu.open"), "openFile", s.openFile),
    item(t("menu.closeImage"), "closeImage", s.closeImage),
    ...navItems,
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.zoomIn"), "zoomIn", s.zoomIn),
    item(t("menu.zoomOut"), "zoomOut", s.zoomOut),
    item(t("menu.actualSize"), "resetView", s.resetView),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.fitWidth"), "fitWidth", s.fitWidth),
    item(t("menu.fitHeight"), "fitHeight", s.fitHeight),
    item(t("menu.fitScreen"), "fitScreen", s.fitScreen),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.rotateCw"), "rotateCW", s.rotateCW),
    item(t("menu.rotateCcw"), "rotateCCW", s.rotateCCW),
    item(t("menu.flipH"), "flipH", s.flipH),
    item(t("menu.flipV"), "flipV", s.flipV),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.toggleExif"), "toggleExif", s.toggleExif),
    item(t("menu.toggleGrid"), "toggleGrid", s.toggleGrid),
    item(t("menu.toggleDock"), "toggleDock"),
    ...gifItems,
    item(t("menu.toggleFullscreen"), "toggleFullscreen", s.toggleFullscreen),
    item(t("menu.toggleAlwaysOnTop"), "toggleAlwaysOnTop", s.toggleAlwaysOnTop),
    item(t("menu.copyImage"), "copyImage", s.copyImage),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.cycleBg"), "cycleBackground", s.cycleBackground),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.trash"), "trashFile", s.trashFile),
    item(t("menu.reveal"), "revealInExplorer", s.revealInExplorer),
    item(t("menu.openExternal"), "openExternal", s.openExternal),
    PredefinedMenuItem.new({ item: "Separator" }),
    item(t("menu.rename"), "renameFile", s.renameFile),
    item(t("menu.copyPath"), "copyPath", s.copyPath)
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
