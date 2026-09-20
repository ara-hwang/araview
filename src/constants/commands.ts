import type { ShortcutActionId } from "@/constants/shortcuts"

export type CommandGroup = "file" | "navigate" | "view" | "display" | "system"

export type CommandId = ShortcutActionId | "openSettings" | "checkForUpdates"

export type CommandContext = {
  hasImage: boolean
  canNavigate: boolean
}

export type CommandDef = {
  id: CommandId
  group: CommandGroup
  labelKey: string
  shortcutId?: ShortcutActionId
  requiresImage?: boolean
  requiresNavigation?: boolean
  /** 현재 UI 언어와 무관하게 매칭되는 영문 별칭 (ko UI에서 영문 검색용) */
  keywords?: readonly string[]
}

export const COMMAND_GROUP_ORDER: readonly CommandGroup[] = [
  "file",
  "navigate",
  "view",
  "display",
  "system"
]

/** 팔레트에 표시되는 명령 정의. 순서가 그룹 내 표시 순서가 된다. */
export const COMMAND_DEFS: readonly CommandDef[] = [
  // 파일
  {
    id: "openFile",
    group: "file",
    labelKey: "menu.open",
    shortcutId: "openFile",
    keywords: ["open", "file"]
  },
  {
    id: "closeImage",
    group: "file",
    labelKey: "menu.closeImage",
    shortcutId: "closeImage",
    requiresImage: true,
    keywords: ["close", "home"]
  },
  {
    id: "saveEdits",
    group: "file",
    labelKey: "menu.saveEdits",
    shortcutId: "saveEdits",
    requiresImage: true,
    keywords: ["save", "edit"]
  },
  {
    id: "copyImage",
    group: "file",
    labelKey: "menu.copyImage",
    shortcutId: "copyImage",
    requiresImage: true,
    keywords: ["copy", "clipboard"]
  },
  {
    id: "trashFile",
    group: "file",
    labelKey: "menu.trash",
    shortcutId: "trashFile",
    requiresImage: true,
    keywords: ["trash", "delete"]
  },
  {
    id: "renameFile",
    group: "file",
    labelKey: "menu.rename",
    shortcutId: "renameFile",
    requiresImage: true,
    keywords: ["rename"]
  },
  {
    id: "copyPath",
    group: "file",
    labelKey: "menu.copyPath",
    shortcutId: "copyPath",
    requiresImage: true,
    keywords: ["path", "copy"]
  },
  {
    id: "revealInExplorer",
    group: "file",
    labelKey: "menu.reveal",
    shortcutId: "revealInExplorer",
    requiresImage: true,
    keywords: ["explorer", "folder", "reveal"]
  },
  {
    id: "openExternal",
    group: "file",
    labelKey: "menu.openExternal",
    shortcutId: "openExternal",
    requiresImage: true,
    keywords: ["external", "default app"]
  },
  // 이동
  {
    id: "navigatePrev",
    group: "navigate",
    labelKey: "menu.prev",
    shortcutId: "navigatePrev",
    requiresNavigation: true,
    keywords: ["prev", "previous"]
  },
  {
    id: "navigateNext",
    group: "navigate",
    labelKey: "menu.next",
    shortcutId: "navigateNext",
    requiresNavigation: true,
    keywords: ["next"]
  },
  {
    id: "jumpPrev10",
    group: "navigate",
    labelKey: "menu.jumpPrev10",
    shortcutId: "jumpPrev10",
    requiresNavigation: true,
    keywords: ["jump", "prev", "10", "pageup"]
  },
  {
    id: "jumpNext10",
    group: "navigate",
    labelKey: "menu.jumpNext10",
    shortcutId: "jumpNext10",
    requiresNavigation: true,
    keywords: ["jump", "next", "10", "pagedown"]
  },
  {
    id: "jumpFirst",
    group: "navigate",
    labelKey: "menu.jumpFirst",
    shortcutId: "jumpFirst",
    requiresNavigation: true,
    keywords: ["jump", "first", "home"]
  },
  {
    id: "jumpLast",
    group: "navigate",
    labelKey: "menu.jumpLast",
    shortcutId: "jumpLast",
    requiresNavigation: true,
    keywords: ["jump", "last", "end"]
  },
  // 보기
  {
    id: "zoomIn",
    group: "view",
    labelKey: "menu.zoomIn",
    shortcutId: "zoomIn",
    requiresImage: true,
    keywords: ["zoom in"]
  },
  {
    id: "zoomOut",
    group: "view",
    labelKey: "menu.zoomOut",
    shortcutId: "zoomOut",
    requiresImage: true,
    keywords: ["zoom out"]
  },
  {
    id: "panLeft",
    group: "view",
    labelKey: "menu.panLeft",
    shortcutId: "panLeft",
    requiresImage: true,
    keywords: ["pan", "left", "move"]
  },
  {
    id: "panRight",
    group: "view",
    labelKey: "menu.panRight",
    shortcutId: "panRight",
    requiresImage: true,
    keywords: ["pan", "right", "move"]
  },
  {
    id: "panUp",
    group: "view",
    labelKey: "menu.panUp",
    shortcutId: "panUp",
    requiresImage: true,
    keywords: ["pan", "up", "move"]
  },
  {
    id: "panDown",
    group: "view",
    labelKey: "menu.panDown",
    shortcutId: "panDown",
    requiresImage: true,
    keywords: ["pan", "down", "move"]
  },
  {
    id: "resetView",
    group: "view",
    labelKey: "menu.actualSize",
    shortcutId: "resetView",
    requiresImage: true,
    keywords: ["actual size", "reset", "100%"]
  },
  {
    id: "fitWidth",
    group: "view",
    labelKey: "menu.fitWidth",
    shortcutId: "fitWidth",
    requiresImage: true,
    keywords: ["fit", "width"]
  },
  {
    id: "fitHeight",
    group: "view",
    labelKey: "menu.fitHeight",
    shortcutId: "fitHeight",
    requiresImage: true,
    keywords: ["fit", "height"]
  },
  {
    id: "fitScreen",
    group: "view",
    labelKey: "menu.fitScreen",
    shortcutId: "fitScreen",
    requiresImage: true,
    keywords: ["fit", "screen"]
  },
  {
    id: "toggleGrid",
    group: "view",
    labelKey: "menu.toggleGrid",
    shortcutId: "toggleGrid",
    requiresImage: true,
    keywords: ["grid", "thumbnails", "overview", "contact sheet"]
  },
  {
    id: "rotateCW",
    group: "view",
    labelKey: "menu.rotateCw",
    shortcutId: "rotateCW",
    requiresImage: true,
    keywords: ["rotate", "clockwise"]
  },
  {
    id: "rotateCCW",
    group: "view",
    labelKey: "menu.rotateCcw",
    shortcutId: "rotateCCW",
    requiresImage: true,
    keywords: ["rotate", "counterclockwise"]
  },
  {
    id: "flipH",
    group: "view",
    labelKey: "menu.flipH",
    shortcutId: "flipH",
    requiresImage: true,
    keywords: ["flip", "horizontal", "mirror"]
  },
  {
    id: "flipV",
    group: "view",
    labelKey: "menu.flipV",
    shortcutId: "flipV",
    requiresImage: true,
    keywords: ["flip", "vertical"]
  },
  // 표시
  {
    id: "toggleExif",
    group: "display",
    labelKey: "menu.toggleExif",
    shortcutId: "toggleExif",
    requiresImage: true,
    keywords: ["exif", "info", "metadata"]
  },
  {
    id: "toggleFullscreen",
    group: "display",
    labelKey: "menu.toggleFullscreen",
    shortcutId: "toggleFullscreen",
    keywords: ["fullscreen"]
  },
  {
    id: "toggleAlwaysOnTop",
    group: "display",
    labelKey: "menu.toggleAlwaysOnTop",
    shortcutId: "toggleAlwaysOnTop",
    keywords: ["always on top", "pin"]
  },
  {
    id: "cycleBackground",
    group: "display",
    labelKey: "menu.cycleBg",
    shortcutId: "cycleBackground",
    requiresImage: true,
    keywords: ["background", "checker"]
  },
  // 시스템
  {
    id: "openSettings",
    group: "system",
    labelKey: "palette.settings",
    keywords: ["settings", "preferences", "설정"]
  },
  {
    id: "checkForUpdates",
    group: "system",
    labelKey: "palette.checkUpdates",
    keywords: ["update", "upgrade", "check", "version", "업데이트"]
  }
]

export function isCommandEnabled(def: CommandDef, ctx: CommandContext): boolean {
  if (def.requiresNavigation) return ctx.hasImage && ctx.canNavigate
  if (def.requiresImage) return ctx.hasImage
  return true
}

export type SearchableCommand = {
  id?: CommandId
  label: string
  keywords?: readonly string[]
}

/**
 * 공백 분리 토큰 AND 매칭. 라벨 시작 일치 > 라벨 포함 > id/별칭 일치 순으로 정렬.
 * 입력이 비면 들어온 순서 그대로 반환한다.
 */
export function filterCommands<T extends SearchableCommand>(
  commands: readonly T[],
  query: string
): T[] {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean)
  if (tokens.length === 0) return [...commands]

  const scored: { item: T; score: number }[] = []
  for (const item of commands) {
    const label = item.label.toLowerCase()
    const alias = `${item.id ?? ""} ${(item.keywords ?? []).join(" ")}`.toLowerCase()
    let score = 0
    let matched = true
    for (const token of tokens) {
      if (label.startsWith(token)) score += 3
      else if (label.includes(token)) score += 2
      else if (alias.includes(token)) score += 1
      else {
        matched = false
        break
      }
    }
    if (matched) scored.push({ item, score })
  }
  scored.sort((a, b) => b.score - a.score)
  return scored.map((entry) => entry.item)
}
