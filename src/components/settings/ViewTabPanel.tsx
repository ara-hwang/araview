import { Images, Palette, Presentation, PushPin } from "@phosphor-icons/react"
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
      autoHideUI: state.autoHideUI,
      menuBarHidden: state.menuBarHidden,
      alwaysOnTop: state.alwaysOnTop
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
