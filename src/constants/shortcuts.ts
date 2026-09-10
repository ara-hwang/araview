export type ShortcutActionId =
  | "navigatePrev"
  | "navigateNext"
  | "zoomIn"
  | "zoomOut"
  | "resetView"
  | "fitWidth"
  | "fitHeight"
  | "fitScreen"
  | "openFile"
  | "closeImage"
  | "toggleExif"
  | "rotateCW"
  | "rotateCCW"
  | "flipH"
  | "flipV"
  | "toggleSlideshow"
  | "toggleFullscreen"
  | "toggleAlwaysOnTop"
  | "copyImage"
  | "trashFile"
  | "revealInExplorer"
  | "openExternal"
  | "cycleBackground"
  | "renameFile"
  | "copyPath"
  | "toggleShuffle"
  | "saveEdits"
  | "toggleFavorite"
  | "togglePalette"

export type ShortcutMap = Record<ShortcutActionId, string>

export type WheelSlot =
  | "wheelUp"
  | "wheelDown"
  | "ctrl+wheelUp"
  | "ctrl+wheelDown"
  | "shift+wheelUp"
  | "shift+wheelDown"
  | "alt+wheelUp"
  | "alt+wheelDown"

export type WheelAction = "prev" | "next" | "zoomIn" | "zoomOut" | "none"

export type WheelMap = Record<WheelSlot, WheelAction>

export type MouseTrigger =
  "leftDrag" | "middleClick" | "doubleClick" | "rightClick"

export type MouseAction =
  | "pan"
  | "prev"
  | "next"
  | "zoomIn"
  | "zoomOut"
  | "toggleFullscreen"
  | "contextMenu"
  | "none"

export type MouseMap = Record<MouseTrigger, MouseAction>

export const SHORTCUT_ACTION_IDS: ShortcutActionId[] = [
  "navigatePrev",
  "navigateNext",
  "zoomIn",
  "zoomOut",
  "resetView",
  "fitWidth",
  "fitHeight",
  "fitScreen",
  "openFile",
  "closeImage",
  "toggleExif",
  "rotateCW",
  "rotateCCW",
  "flipH",
  "flipV",
  "toggleSlideshow",
  "toggleFullscreen",
  "toggleAlwaysOnTop",
  "copyImage",
  "trashFile",
  "revealInExplorer",
  "openExternal",
  "cycleBackground",
  "renameFile",
  "copyPath",
  "toggleShuffle",
  "saveEdits",
  "toggleFavorite",
  "togglePalette"
]

export const WHEEL_SLOTS: WheelSlot[] = [
  "wheelUp",
  "wheelDown",
  "ctrl+wheelUp",
  "ctrl+wheelDown",
  "shift+wheelUp",
  "shift+wheelDown",
  "alt+wheelUp",
  "alt+wheelDown"
]

export const MOUSE_TRIGGERS: MouseTrigger[] = [
  "leftDrag",
  "middleClick",
  "doubleClick",
  "rightClick"
]

export const DEFAULT_SHORTCUTS: ShortcutMap = {
  navigatePrev: "ArrowLeft",
  navigateNext: "ArrowRight",
  zoomIn: "=",
  zoomOut: "-",
  resetView: "0",
  fitWidth: "1",
  fitHeight: "2",
  fitScreen: "3",
  openFile: "Ctrl+O",
  closeImage: "Escape",
  toggleExif: "I",
  rotateCW: "R",
  rotateCCW: "Shift+R",
  flipH: "H",
  flipV: "V",
  toggleSlideshow: "Space",
  toggleFullscreen: "F11",
  toggleAlwaysOnTop: "T",
  copyImage: "Ctrl+C",
  trashFile: "Delete",
  revealInExplorer: "Ctrl+Shift+E",
  openExternal: "Ctrl+Shift+O",
  cycleBackground: "B",
  renameFile: "F2",
  copyPath: "Ctrl+Shift+C",
  toggleShuffle: "S",
  saveEdits: "Ctrl+S",
  toggleFavorite: "F",
  togglePalette: "Ctrl+K"
}

export const DEFAULT_WHEEL: WheelMap = {
  wheelUp: "prev",
  wheelDown: "next",
  "ctrl+wheelUp": "zoomIn",
  "ctrl+wheelDown": "zoomOut",
  "shift+wheelUp": "none",
  "shift+wheelDown": "none",
  "alt+wheelUp": "none",
  "alt+wheelDown": "none"
}

export const DEFAULT_MOUSE: MouseMap = {
  leftDrag: "pan",
  middleClick: "none",
  doubleClick: "toggleFullscreen",
  rightClick: "contextMenu"
}

const LETTER_RE = /^[a-zA-Z]$/

function normalizePrimaryKey(rawKey: string): string | null {
  if (rawKey === " ") return "Space"
  if (rawKey === "Spacebar") return "Space"
  if (rawKey === "Esc") return "Escape"
  if (rawKey === "Del") return "Delete"
  if (rawKey === "Up") return "ArrowUp"
  if (rawKey === "Down") return "ArrowDown"
  if (rawKey === "Left") return "ArrowLeft"
  if (rawKey === "Right") return "ArrowRight"
  if (rawKey.length === 1) {
    if (LETTER_RE.test(rawKey)) return rawKey.toUpperCase()
    return rawKey
  }
  if (
    rawKey === "Control" ||
    rawKey === "Shift" ||
    rawKey === "Alt" ||
    rawKey === "Meta" ||
    rawKey === "Dead"
  ) {
    return null
  }
  return rawKey
}

export function normalizeBinding(value: unknown): string | null {
  if (typeof value !== "string") return null
  const trimmed = value.trim()
  if (trimmed === "") return ""
  if (trimmed === "+") return "+"
  let rawKey: string
  let rawModifiers: string[]
  if (trimmed.endsWith("+") && trimmed.length > 1) {
    rawKey = "+"
    rawModifiers = trimmed
      .slice(0, -1)
      .split("+")
      .map((part) => part.trim().toLowerCase())
      .filter(Boolean)
  } else {
    const parts = trimmed
      .split("+")
      .map((part) => part.trim())
      .filter(Boolean)
    if (parts.length === 0) return null
    rawKey = parts[parts.length - 1]
    rawModifiers = parts.slice(0, -1).map((mod) => mod.toLowerCase())
  }
  const modifiers = new Set<string>()
  for (const mod of rawModifiers) {
    if (mod === "ctrl" || mod === "control") modifiers.add("Ctrl")
    else if (mod === "shift") modifiers.add("Shift")
    else if (mod === "alt") modifiers.add("Alt")
    else if (mod === "meta" || mod === "cmd" || mod === "win")
      modifiers.add("Meta")
    else return null
  }
  const primary = normalizePrimaryKey(rawKey)
  if (!primary) return null
  if (modifiers.has("Meta")) return null
  const isLetter = primary.length === 1 && LETTER_RE.test(primary)
  const isNamed = primary.length > 1
  if (!isLetter && !isNamed) {
    modifiers.delete("Shift")
  }
  const ordered = ["Ctrl", "Shift", "Alt"].filter((mod) => modifiers.has(mod))
  return ordered.length > 0 ? `${ordered.join("+")}+${primary}` : primary
}

type KeyEventLike = {
  key: string
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  metaKey: boolean
}

export function eventToBinding(event: KeyEventLike): string | null {
  if (event.metaKey) return null
  const primary = normalizePrimaryKey(event.key)
  if (!primary) return null
  const modifiers: string[] = []
  if (event.ctrlKey) modifiers.push("Ctrl")
  const isLetter = primary.length === 1 && LETTER_RE.test(primary)
  const isNamed = primary.length > 1
  if (event.shiftKey && (isLetter || isNamed)) modifiers.push("Shift")
  if (event.altKey) modifiers.push("Alt")
  return modifiers.length > 0 ? `${modifiers.join("+")}+${primary}` : primary
}

const RESERVED_BINDINGS = new Set(["Tab"])

export function isReservedBinding(binding: string): boolean {
  return RESERVED_BINDINGS.has(binding)
}

export function isValidBinding(binding: string): boolean {
  if (binding === "") return true
  const normalized = normalizeBinding(binding)
  if (!normalized || normalized === "") return false
  if (normalized !== binding) return false
  if (isReservedBinding(normalized)) return false
  return true
}

export function findBindingConflict(
  shortcuts: ShortcutMap,
  actionId: ShortcutActionId,
  binding: string
): ShortcutActionId | null {
  if (binding === "") return null
  for (const id of SHORTCUT_ACTION_IDS) {
    if (id !== actionId && shortcuts[id] === binding) return id
  }
  return null
}

export function sanitizeShortcutMap(value: unknown): ShortcutMap {
  const result: ShortcutMap = { ...DEFAULT_SHORTCUTS }
  if (typeof value !== "object" || value === null) return result
  const record = value as Record<string, unknown>
  for (const id of SHORTCUT_ACTION_IDS) {
    const raw = record[id]
    if (typeof raw !== "string") continue
    if (raw === "") {
      result[id] = ""
      continue
    }
    const normalized = normalizeBinding(raw)
    if (normalized && normalized !== "" && !isReservedBinding(normalized)) {
      result[id] = normalized
    }
  }
  return result
}

const VALID_WHEEL_ACTIONS: WheelAction[] = [
  "prev",
  "next",
  "zoomIn",
  "zoomOut",
  "none"
]

export function sanitizeWheelMap(value: unknown): WheelMap {
  const result: WheelMap = { ...DEFAULT_WHEEL }
  if (typeof value !== "object" || value === null) return result
  const record = value as Record<string, unknown>
  for (const slot of WHEEL_SLOTS) {
    const raw = record[slot]
    if (
      typeof raw === "string" &&
      (VALID_WHEEL_ACTIONS as string[]).includes(raw)
    ) {
      result[slot] = raw as WheelAction
    }
  }
  return result
}

const VALID_MOUSE_ACTIONS: MouseAction[] = [
  "pan",
  "prev",
  "next",
  "zoomIn",
  "zoomOut",
  "toggleFullscreen",
  "contextMenu",
  "none"
]

export function sanitizeMouseMap(value: unknown): MouseMap {
  const result: MouseMap = { ...DEFAULT_MOUSE }
  if (typeof value !== "object" || value === null) return result
  const record = value as Record<string, unknown>
  for (const trigger of MOUSE_TRIGGERS) {
    const raw = record[trigger]
    if (
      typeof raw === "string" &&
      (VALID_MOUSE_ACTIONS as string[]).includes(raw)
    ) {
      result[trigger] = raw as MouseAction
    }
  }
  if (result.leftDrag !== "pan" && result.leftDrag !== "none") {
    result.leftDrag = "pan"
  }
  return result
}

export function formatShortcutDisplay(binding: string): string {
  if (binding === "") return ""
  return binding
}

export function toTauriAccelerator(binding: string): string | undefined {
  if (binding === "") return undefined
  const mapped = binding
    .replace("ArrowLeft", "Left")
    .replace("ArrowRight", "Right")
    .replace("ArrowUp", "Up")
    .replace("ArrowDown", "Down")
  if (mapped === "Delete") return "Del"
  if (mapped === "Escape") return "Esc"
  if (mapped === "Space") return "Space"
  return mapped
}

export const LEFT_DRAG_OPTIONS: MouseAction[] = ["pan", "none"]

export const CLICK_MOUSE_OPTIONS: MouseAction[] = [
  "prev",
  "next",
  "zoomIn",
  "zoomOut",
  "toggleFullscreen",
  "contextMenu",
  "none"
]
