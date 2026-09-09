import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import {
  updateSettings,
  useSettingsStore,
  type AppLanguage,
  type SettingsState
} from "@/store/settingsStore"
import { House, NavigationArrow, Translate } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

export function GeneralTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      language: state.language,
      loopNavigation: state.loopNavigation,
      autoOpenLastFile: state.autoOpenLastFile
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<Translate className="size-6" />}
        title={t("settings.language.title")}
        description={t("settings.language.desc")}
      >
        <RadioGroup
          value={settings.language}
          onValueChange={(value) =>
            handleSettingsChange({ language: value as AppLanguage })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="ko" id="settings-lang-ko" />
            <Label htmlFor="settings-lang-ko">
              {t("settings.language.ko")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="en" id="settings-lang-en" />
            <Label htmlFor="settings-lang-en">
              {t("settings.language.en")}
            </Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<House className="size-6" />}
        title={t("settings.startup.title")}
        description={t("settings.startup.desc")}
      >
        <RadioGroup
          value={settings.autoOpenLastFile ? "on" : "off"}
          onValueChange={(value) =>
            handleSettingsChange({ autoOpenLastFile: value === "on" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-startup-off" />
            <Label htmlFor="settings-startup-off">
              {t("settings.startup.home")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="on" id="settings-startup-on" />
            <Label htmlFor="settings-startup-on">
              {t("settings.startup.last")}
            </Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<NavigationArrow className="size-6" />}
        title={t("settings.nav.title")}
        description={t("settings.nav.desc")}
      >
        <RadioGroup
          value={settings.loopNavigation ? "loop" : "stop"}
          onValueChange={(value) =>
            handleSettingsChange({ loopNavigation: value === "loop" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="stop" id="settings-nav-stop" />
            <Label htmlFor="settings-nav-stop">{t("settings.nav.stop")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="loop" id="settings-nav-loop" />
            <Label htmlFor="settings-nav-loop">{t("settings.nav.loop")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
