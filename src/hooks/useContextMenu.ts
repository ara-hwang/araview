import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu"
import { useCallback } from "react"
import type { DirectoryImages } from "../types"

export type ImageViewerContextMenuActions = {
  onOpenFile: () => void
  onNavigatePrev: () => void
  onNavigateNext: () => void
  onZoomIn: () => void
  onZoomOut: () => void
  onResetZoom: () => void
  onFitWidth: () => void
  onFitHeight: () => void
  onFitScreen: () => void
}

export async function showImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions
): Promise<void> {
  const navItems =
    dirImages && dirImages.images.length > 1
      ? [
          PredefinedMenuItem.new({ item: "Separator" }),
          MenuItem.new({
            text: "Previous Image",
            accelerator: "Left",
            action: () => actions.onNavigatePrev()
          }),
          MenuItem.new({
            text: "Next Image",
            accelerator: "Right",
            action: () => actions.onNavigateNext()
          })
        ]
      : []

  const [openItem, ...rest] = await Promise.all([
    MenuItem.new({
      text: "Open File",
      accelerator: "CmdOrCtrl+O",
      action: () => actions.onOpenFile()
    }),
    ...navItems,
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: "Zoom In",
      accelerator: "=",
      action: () => actions.onZoomIn()
    }),
    MenuItem.new({
      text: "Zoom Out",
      accelerator: "-",
      action: () => actions.onZoomOut()
    }),
    MenuItem.new({
      text: "Actual Size",
      accelerator: "0",
      action: () => actions.onResetZoom()
    }),
    PredefinedMenuItem.new({ item: "Separator" }),
    MenuItem.new({
      text: "Fit Width",
      accelerator: "1",
      action: () => actions.onFitWidth()
    }),
    MenuItem.new({
      text: "Fit Height",
      accelerator: "2",
      action: () => actions.onFitHeight()
    }),
    MenuItem.new({
      text: "Fit to Screen",
      accelerator: "3",
      action: () => actions.onFitScreen()
    })
  ])

  const menu = await Menu.new({ items: [openItem, ...rest] })
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
