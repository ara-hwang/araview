import {
  BookOpenText,
  Images,
  ListBullets,
  Palette,
  Presentation,
  PushPin
} from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import { ShortcutBadge } from "@/components/settings/ShortcutBadge"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Switch } from "@/components/ui/switch"
import {
  updateSettings,
  useSettingsStore,
  type DockPosition,
  type DockThumbSize,
  type SettingsState,
  type ViewerBackground,
  type ViewMode
} from "@/store/settingsStore"

export function ViewTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      viewMode: state.viewMode,
      viewerBackground: state.viewerBackground,
      resumeReading: state.resumeReading,
      autoHideUI: state.autoHideUI,
      menuBarHidden: state.menuBarHidden,
      alwaysOnTop: state.alwaysOnTop,
      dockPosition: state.dockPosition,
      dockVisible: state.dockVisible,
      dockThumbSize: state.dockThumbSize,
      dockShowName: state.dockShowName,
      dockShowIndex: state.dockShowIndex
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<Images className="size-6" />}
        title={t("settings.view.title")}
        description={t("settings.view.desc")}
      >
        <RadioGroup
          value={settings.viewMode}
          onValueChange={(value) => handleSettingsChange({ viewMode: value as ViewMode })}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="single" id="settings-view-single" />
            <Label htmlFor="settings-view-single">{t("settings.view.single")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="left-to-right" id="settings-view-ltr" />
            <Label htmlFor="settings-view-ltr">{t("settings.view.ltr")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="right-to-left" id="settings-view-rtl" />
            <Label htmlFor="settings-view-rtl">{t("settings.view.rtl")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="webtoon" id="settings-view-webtoon" />
            <Label htmlFor="settings-view-webtoon">{t("settings.view.webtoon")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<BookOpenText className="size-6" />}
        title={t("settings.reading.title")}
        description={t("settings.reading.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-resume"
            checked={settings.resumeReading}
            onCheckedChange={(checked) => handleSettingsChange({ resumeReading: checked === true })}
          />
          <Label htmlFor="settings-reading-resume">{t("settings.reading.resume")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Palette className="size-6" />}
        title={t("settings.bg.title")}
        description={t("settings.bg.desc")}
        shortcutId="cycleBackground"
      >
        <RadioGroup
          value={settings.viewerBackground}
          onValueChange={(value) =>
            handleSettingsChange({
              viewerBackground: value as ViewerBackground
            })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="theme" id="settings-bg-theme" />
            <Label htmlFor="settings-bg-theme">{t("settings.bg.theme")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="black" id="settings-bg-black" />
            <Label htmlFor="settings-bg-black">{t("settings.bg.black")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="white" id="settings-bg-white" />
            <Label htmlFor="settings-bg-white">{t("settings.bg.white")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="checker" id="settings-bg-checker" />
            <Label htmlFor="settings-bg-checker">{t("settings.bg.checker")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<ListBullets className="size-6" />}
        title={t("settings.dock.title")}
        description={t("settings.dock.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-visible"
            checked={settings.dockVisible}
            onCheckedChange={(checked) => handleSettingsChange({ dockVisible: checked === true })}
          />
          <Label htmlFor="settings-dock-visible">{t("settings.dock.visible")}</Label>
        </Field>
        <RadioGroup
          value={settings.dockPosition}
          onValueChange={(value) => handleSettingsChange({ dockPosition: value as DockPosition })}
          aria-label={t("settings.dock.position")}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="top" id="settings-dock-top" />
            <Label htmlFor="settings-dock-top">{t("settings.dock.top")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="bottom" id="settings-dock-bottom" />
            <Label htmlFor="settings-dock-bottom">{t("settings.dock.bottom")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="left" id="settings-dock-left" />
            <Label htmlFor="settings-dock-left">{t("settings.dock.left")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="right" id="settings-dock-right" />
            <Label htmlFor="settings-dock-right">{t("settings.dock.right")}</Label>
          </Field>
        </RadioGroup>
        <RadioGroup
          value={settings.dockThumbSize}
          onValueChange={(value) => handleSettingsChange({ dockThumbSize: value as DockThumbSize })}
          aria-label={t("settings.dock.thumbSize")}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="s" id="settings-dock-s" />
            <Label htmlFor="settings-dock-s">{t("settings.dock.thumbS")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="m" id="settings-dock-m" />
            <Label htmlFor="settings-dock-m">{t("settings.dock.thumbM")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="l" id="settings-dock-l" />
            <Label htmlFor="settings-dock-l">{t("settings.dock.thumbL")}</Label>
          </Field>
        </RadioGroup>
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-showname"
            checked={settings.dockShowName}
            onCheckedChange={(checked) => handleSettingsChange({ dockShowName: checked === true })}
          />
          <Label htmlFor="settings-dock-showname">{t("settings.dock.showName")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-showindex"
            checked={settings.dockShowIndex}
            onCheckedChange={(checked) => handleSettingsChange({ dockShowIndex: checked === true })}
          />
          <Label htmlFor="settings-dock-showindex">{t("settings.dock.showIndex")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Presentation className="size-6" />}
        title={t("settings.present.title")}
        description={t("settings.present.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-autohide"
            checked={settings.autoHideUI}
            onCheckedChange={(checked) => handleSettingsChange({ autoHideUI: checked === true })}
          />
          <Label htmlFor="settings-autohide">{t("settings.present.autohide")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-hide-menubar"
            checked={settings.menuBarHidden}
            onCheckedChange={(checked) => handleSettingsChange({ menuBarHidden: checked === true })}
          />
          <Label htmlFor="settings-hide-menubar">{t("settings.present.hideMenuBar")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<PushPin className="size-6" />}
        title={t("settings.window.title")}
        description={t("settings.window.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-always-on-top"
            checked={settings.alwaysOnTop}
            onCheckedChange={(checked) => handleSettingsChange({ alwaysOnTop: checked === true })}
          />
          <Label htmlFor="settings-always-on-top">
            {t("settings.window.alwaysOnTop")}
            <ShortcutBadge actionId="toggleAlwaysOnTop" />
          </Label>
        </Field>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
