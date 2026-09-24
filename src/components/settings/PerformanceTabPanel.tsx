import { CornersOut, HardDrive } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  updateSettings,
  useSettingsStore,
  type CacheMode,
  type MaxResolution,
  type SettingsState
} from "@/store/settingsStore"

export function PerformanceTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      cacheMode: state.cacheMode,
      maxResolution: state.maxResolution
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<HardDrive className="size-6" />}
        title={t("settings.cache.title")}
        description={t("settings.cache.desc")}
      >
        <RadioGroup
          value={settings.cacheMode}
          onValueChange={(value) => handleSettingsChange({ cacheMode: value as CacheMode })}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-cache-off" />
            <Label htmlFor="settings-cache-off">{t("settings.cache.off")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="nearby" id="settings-cache-nearby" />
            <Label htmlFor="settings-cache-nearby">{t("settings.cache.nearby")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="extended" id="settings-cache-extended" />
            <Label htmlFor="settings-cache-extended">{t("settings.cache.extended")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-1gb" id="settings-cache-1gb" />
            <Label htmlFor="settings-cache-1gb">{t("settings.cache.gb1")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-2gb" id="settings-cache-2gb" />
            <Label htmlFor="settings-cache-2gb">{t("settings.cache.gb2")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<CornersOut className="size-6" />}
        title={t("settings.resolution.title")}
        description={t("settings.resolution.desc")}
      >
        <RadioGroup
          value={settings.maxResolution}
          onValueChange={(value) => handleSettingsChange({ maxResolution: value as MaxResolution })}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="original" id="settings-resolution-original" />
            <Label htmlFor="settings-resolution-original">
              {t("settings.resolution.original")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="4k" id="settings-resolution-4k" />
            <Label htmlFor="settings-resolution-4k">{t("settings.resolution.4k")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="1080p" id="settings-resolution-1080p" />
            <Label htmlFor="settings-resolution-1080p">{t("settings.resolution.1080p")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
