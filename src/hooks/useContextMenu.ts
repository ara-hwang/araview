import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu"
import { useCallback } from "react"

import type { CommandId } from "@/constants/commands"
import { toTauriAccelerator } from "@/constants/shortcuts"
import { isCoverPage } from "@/hooks/useComicCoverEdit"
import { runViewerCommand, type ViewerActionHandlers } from "@/hooks/viewerActions"
import i18n from "@/i18n"
import { useAppStore } from "@/store/appStore"
import { useGifStore } from "@/store/gifStore"
import { getSettings } from "@/store/settingsStore"

import type { DirectoryImages } from "../types"

export type ImageViewerContextMenuActions = ViewerActionHandlers

/** 우클릭한 페이지의 표지 지정 항목. 아카이브 안에서만 메뉴에 넣는다. */
export type CoverMenuTarget = {
  isCover: boolean
  onToggle: () => void
}

/**
 * 우클릭한 요소가 가리키는 페이지 인덱스. 양쪽 보기와 웹툰은 클릭한 쪽 페이지를,
 * 그 밖에는 현재 페이지를 돌려준다.
 */
export function contextPageIndex(target: EventTarget | null): number {
  const { dirImages } = useAppStore.getState()
  const el =
    target instanceof Element ? target.closest("[data-page-path], [data-webtoon-index]") : null
  if (el instanceof HTMLElement) {
    const index =
      el.dataset.pagePath !== undefined
        ? dirImages.images.indexOf(el.dataset.pagePath)
        : Number(el.dataset.webtoonIndex)
    if (Number.isInteger(index) && index >= 0 && index < dirImages.images.length) return index
  }
  return dirImages.current_index
}

export async function showImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions,
  cover?: CoverMenuTarget
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

  const coverItems = cover
    ? [
        PredefinedMenuItem.new({ item: "Separator" }),
        MenuItem.new({
          text: t(cover.isCover ? "menu.unsetCover" : "menu.setCover"),
          action: cover.onToggle
        })
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
    item(t("menu.copyPath"), "copyPath", s.copyPath),
    ...coverItems
  ])

  const menu = await Menu.new({ items: [openItem, closeItem, ...rest] })
  await menu.popup()
}

export function useImageViewerContextMenu(
  dirImages: DirectoryImages | null,
  actions: ImageViewerContextMenuActions,
  onToggleCover?: (index: number) => void
) {
  return useCallback(
    async (e: React.MouseEvent) => {
      e.preventDefault()
      const inArchive = useAppStore.getState().archivePath !== null
      const index = contextPageIndex(e.target)
      const cover =
        onToggleCover && inArchive
          ? { isCover: isCoverPage(index), onToggle: () => onToggleCover(index) }
          : undefined
      await showImageViewerContextMenu(dirImages, actions, cover)
    },
    [dirImages, actions, onToggleCover]
  )
}
