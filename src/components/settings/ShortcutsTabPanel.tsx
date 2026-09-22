import { ArrowCounterClockwise, Keyboard, Mouse, X } from "@phosphor-icons/react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { toast } from "@/components/ui/toast"
import {
  CLICK_MOUSE_OPTIONS,
  DEFAULT_SHORTCUTS,
  LEFT_DRAG_OPTIONS,
  MOUSE_TRIGGERS,
  SHORTCUT_ACTION_IDS,
  WHEEL_SLOTS,
  eventToBinding,
  findBindingConflict,
  formatShortcutDisplay,
  isReservedBinding,
  type MouseAction,
  type ShortcutActionId,
  type WheelAction
} from "@/constants/shortcuts"
import { resetShortcutsToDefault, updateSettings, useSettingsStore } from "@/store/settingsStore"

const ACTION_LABEL_KEY: Record<ShortcutActionId, string> = {
  navigatePrev: "menu.prev",
  navigateNext: "menu.next",
  panLeft: "menu.panLeft",
  panRight: "menu.panRight",
  panUp: "menu.panUp",
  panDown: "menu.panDown",
  zoomIn: "menu.zoomIn",
  zoomOut: "menu.zoomOut",
  resetView: "menu.actualSize",
  fitWidth: "menu.fitWidth",
  fitHeight: "menu.fitHeight",
  fitScreen: "menu.fitScreen",
  openFile: "menu.open",
  closeImage: "menu.closeImage",
  toggleExif: "menu.toggleExif",
  rotateCW: "menu.rotateCw",
  rotateCCW: "menu.rotateCcw",
  flipH: "menu.flipH",
  flipV: "menu.flipV",
  toggleFullscreen: "menu.toggleFullscreen",
  toggleAlwaysOnTop: "menu.toggleAlwaysOnTop",
  copyImage: "menu.copyImage",
  trashFile: "menu.trash",
  revealInExplorer: "menu.reveal",
  openExternal: "menu.openExternal",
  cycleBackground: "menu.cycleBg",
  renameFile: "menu.rename",
  copyPath: "menu.copyPath",
  saveEdits: "menu.saveEdits",
  togglePalette: "palette.open",
  jumpPrev10: "menu.jumpPrev10",
  jumpNext10: "menu.jumpNext10",
  jumpFirst: "menu.jumpFirst",
  jumpLast: "menu.jumpLast",
  toggleGrid: "menu.toggleGrid",
  toggleGifPlayback: "menu.gifPlayPause",
  gifPrevFrame: "menu.gifPrevFrame",
  gifNextFrame: "menu.gifNextFrame"
}

type ConflictState = {
  actionId: ShortcutActionId
  binding: string
  conflictingActionId: ShortcutActionId
}

export function ShortcutsTabPanel() {
  const { t } = useTranslation()
  const shortcuts = useSettingsStore((s) => s.shortcuts)
  const wheel = useSettingsStore((s) => s.wheel)
  const mouse = useSettingsStore((s) => s.mouse)
  const [capturing, setCapturing] = useState<ShortcutActionId | null>(null)
  const [conflict, setConflict] = useState<ConflictState | null>(null)

  useEffect(() => {
    if (!capturing) return
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === "Escape") {
        setCapturing(null)
        return
      }
      const binding = eventToBinding(e)
      if (!binding) {
        toast.error(t("settings.shortcuts.invalid"))
        return
      }
      if (isReservedBinding(binding)) {
        toast.error(t("settings.shortcuts.reserved"))
        return
      }
      const current = useSettingsStore.getState().shortcuts
      const duplicates = findBindingConflict(current, capturing, binding)
      if (duplicates) {
        setConflict({
          actionId: capturing,
          binding,
          conflictingActionId: duplicates
        })
        return
      }
      void updateSettings({
        shortcuts: { ...current, [capturing]: binding }
      })
      setCapturing(null)
    }
    window.addEventListener("keydown", onKeyDown, true)
    return () => {
      window.removeEventListener("keydown", onKeyDown, true)
    }
  }, [capturing, t])

  const handleClear = (actionId: ShortcutActionId) => {
    void updateSettings({ shortcuts: { ...shortcuts, [actionId]: "" } })
    if (capturing === actionId) setCapturing(null)
  }

  const handleRestore = (actionId: ShortcutActionId) => {
    const current = useSettingsStore.getState().shortcuts
    const fallback = DEFAULT_SHORTCUTS[actionId]
    const duplicates = findBindingConflict(current, actionId, fallback)
    if (duplicates) {
      setConflict({
        actionId,
        binding: fallback,
        conflictingActionId: duplicates
      })
      return
    }
    void updateSettings({ shortcuts: { ...current, [actionId]: fallback } })
  }

  const handleConfirmConflict = () => {
    if (!conflict) return
    const current = useSettingsStore.getState().shortcuts
    void updateSettings({
      shortcuts: {
        ...current,
        [conflict.conflictingActionId]: "",
        [conflict.actionId]: conflict.binding
      }
    })
    setConflict(null)
    setCapturing(null)
  }

  const tx = t as unknown as (key: string, options?: Record<string, string>) => string

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<Keyboard className="size-6" />}
        title={t("settings.shortcuts.title")}
        description={t("settings.shortcuts.desc")}
      >
        <p className="text-xs text-muted-foreground" aria-live="polite">
          {capturing ? t("settings.shortcuts.capturing") : t("settings.shortcuts.captureHint")}
        </p>
        <ul className="divide-y divide-border rounded-lg border">
          {SHORTCUT_ACTION_IDS.map((actionId) => {
            const binding = shortcuts[actionId] ?? ""
            const isCapturing = capturing === actionId
            return (
              <li key={actionId} className="flex items-center justify-between gap-2 px-2 py-1.5">
                <span className="min-w-0 flex-1 truncate text-sm">
                  {tx(ACTION_LABEL_KEY[actionId])}
                </span>
                <span className="flex shrink-0 items-center gap-1">
                  <Button
                    variant={isCapturing ? "default" : "outline"}
                    size="sm"
                    className="min-w-28 font-mono tabular-nums"
                    aria-pressed={isCapturing}
                    aria-label={`${tx(ACTION_LABEL_KEY[actionId])}, ${binding ? formatShortcutDisplay(binding) : t("settings.shortcuts.unbound")}`}
                    onClick={() => setCapturing(isCapturing ? null : actionId)}
                  >
                    {isCapturing
                      ? t("settings.shortcuts.capturing")
                      : binding
                        ? formatShortcutDisplay(binding)
                        : t("settings.shortcuts.unbound")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title={t("settings.shortcuts.clear")}
                    aria-label={`${t("settings.shortcuts.clear")}, ${tx(ACTION_LABEL_KEY[actionId])}`}
                    disabled={binding === ""}
                    onClick={() => handleClear(actionId)}
                  >
                    <X />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title={t("settings.shortcuts.restore")}
                    aria-label={`${t("settings.shortcuts.restore")}, ${tx(ACTION_LABEL_KEY[actionId])}`}
                    disabled={binding === DEFAULT_SHORTCUTS[actionId]}
                    onClick={() => handleRestore(actionId)}
                  >
                    <ArrowCounterClockwise />
                  </Button>
                </span>
              </li>
            )
          })}
        </ul>

        {conflict && (
          <div
            role="alertdialog"
            aria-labelledby="shortcut-conflict-title"
            aria-describedby="shortcut-conflict-desc"
            className="rounded-lg border border-destructive/30 bg-destructive/5 p-3"
          >
            <p id="shortcut-conflict-title" className="text-sm font-medium">
              {t("settings.shortcuts.conflictTitle")}
            </p>
            <p id="shortcut-conflict-desc" className="mt-1 text-xs text-muted-foreground">
              {tx("settings.shortcuts.conflictDesc", {
                key: formatShortcutDisplay(conflict.binding),
                action: tx(ACTION_LABEL_KEY[conflict.conflictingActionId])
              })}
            </p>
            <div className="mt-2 flex justify-end gap-2">
              <Button variant="outline" size="sm" onClick={() => setConflict(null)}>
                {t("settings.shortcuts.cancel")}
              </Button>
              <Button size="sm" onClick={handleConfirmConflict}>
                {t("settings.shortcuts.replace")}
              </Button>
            </div>
          </div>
        )}
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Mouse className="size-6" />}
        title={t("settings.wheel.title")}
        description={t("settings.wheel.desc")}
      >
        {WHEEL_SLOTS.map((slot) => (
          <Field key={slot} orientation="horizontal">
            <label htmlFor={`settings-wheel-${slot}`} className="min-w-0 flex-1 text-sm">
              {tx(`settings.wheel.slot.${slot}`)}
            </label>
            <select
              id={`settings-wheel-${slot}`}
              value={wheel[slot]}
              onChange={(e) => {
                const next = e.target.value as WheelAction
                void updateSettings({ wheel: { ...wheel, [slot]: next } })
              }}
              className="h-8 rounded-lg border border-border bg-background px-2 text-sm"
            >
              {(["prev", "next", "zoomIn", "zoomOut", "none"] as WheelAction[]).map((action) => (
                <option key={action} value={action}>
                  {tx(`settings.wheel.action.${action}`)}
                </option>
              ))}
            </select>
          </Field>
        ))}
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Mouse className="size-6" />}
        title={t("settings.mouse.title")}
        description={t("settings.mouse.desc")}
      >
        {MOUSE_TRIGGERS.map((trigger) => {
          const options = trigger === "leftDrag" ? LEFT_DRAG_OPTIONS : CLICK_MOUSE_OPTIONS
          return (
            <Field key={trigger} orientation="horizontal">
              <label htmlFor={`settings-mouse-${trigger}`} className="min-w-0 flex-1 text-sm">
                {tx(`settings.mouse.trigger.${trigger}`)}
              </label>
              <select
                id={`settings-mouse-${trigger}`}
                value={mouse[trigger]}
                onChange={(e) => {
                  const next = e.target.value as MouseAction
                  void updateSettings({ mouse: { ...mouse, [trigger]: next } })
                }}
                className="h-8 rounded-lg border border-border bg-background px-2 text-sm"
              >
                {options.map((action) => (
                  <option key={action} value={action}>
                    {tx(`settings.mouse.action.${action}`)}
                  </option>
                ))}
              </select>
            </Field>
          )
        })}
        {mouse.rightClick !== "contextMenu" && (
          <p role="note" className="text-xs text-muted-foreground">
            {t("settings.mouse.rightClickWarning")}
          </p>
        )}
        <div className="flex justify-end">
          <Button variant="outline" size="sm" onClick={() => void resetShortcutsToDefault()}>
            {t("settings.shortcuts.resetAll")}
          </Button>
        </div>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
