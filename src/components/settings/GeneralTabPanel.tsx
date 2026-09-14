import { useState } from "react"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import {
  updateSettings,
  useSettingsStore,
  type AppLanguage,
  type SettingsState
} from "@/store/settingsStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { checkForUpdatesNow, useAppVersion } from "@/hooks/useUpdater"
import {
  ArrowClockwise,
  ClockCounterClockwise,
  House,
  NavigationArrow,
  SkipForward,
  Translate
} from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

export function GeneralTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      language: state.language,
      loopNavigation: state.loopNavigation,
      autoOpenLastFile: state.autoOpenLastFile,
      recordRecentFiles: state.recordRecentFiles,
      skipBrokenFiles: state.skipBrokenFiles
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  const handleRecentChange = (value: string) => {
    const enabled = value === "on"
    if (enabled) {
      handleSettingsChange({ recordRecentFiles: true })
      return
    }
    handleSettingsChange({ recordRecentFiles: false, autoOpenLastFile: false })
    void useRecentFilesStore.getState().clear()
  }

  const appVersion = useAppVersion()
  const [checkingUpdate, setCheckingUpdate] = useState(false)
  const handleCheckUpdate = () => {
    setCheckingUpdate(true)
    void checkForUpdatesNow().finally(() => setCheckingUpdate(false))
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
          disabled={!settings.recordRecentFiles}
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
        icon={<ClockCounterClockwise className="size-6" />}
        title={t("settings.recent.title")}
        description={t("settings.recent.desc")}
      >
        <RadioGroup
          value={settings.recordRecentFiles ? "on" : "off"}
          onValueChange={handleRecentChange}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="on" id="settings-recent-on" />
            <Label htmlFor="settings-recent-on">
              {t("settings.recent.on")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-recent-off" />
            <Label htmlFor="settings-recent-off">
              {t("settings.recent.off")}
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

      <FieldSeparator />

      <SettingsFieldSet
        icon={<SkipForward className="size-6" />}
        title={t("settings.skipBroken.title")}
        description={t("settings.skipBroken.desc")}
      >
        <RadioGroup
          value={settings.skipBrokenFiles ? "on" : "off"}
          onValueChange={(value) =>
            handleSettingsChange({ skipBrokenFiles: value === "on" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-skip-off" />
            <Label htmlFor="settings-skip-off">
              {t("settings.skipBroken.off")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="on" id="settings-skip-on" />
            <Label htmlFor="settings-skip-on">
              {t("settings.skipBroken.on")}
            </Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<ArrowClockwise className="size-6" />}
        title={t("settings.update.title")}
        description={t("settings.update.desc")}
      >
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={handleCheckUpdate} disabled={checkingUpdate}>
            {checkingUpdate
              ? t("settings.update.checking")
              : t("settings.update.check")}
          </Button>
          <span className="text-muted-foreground text-sm">
            {t("settings.update.current", {
              version: appVersion ?? t("settings.update.unknown")
            })}
          </span>
        </div>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
