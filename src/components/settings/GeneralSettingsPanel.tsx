import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLegend,
  FieldSeparator,
  FieldSet
} from "@/components/ui/field"
import {
  updateSettings,
  useSettingsStore,
  type AppLanguage,
  type CacheMode,
  type DirSortKey,
  type SettingsState,
  type ViewerBackground,
  type ViewMode
} from "@/store/settingsStore"
import { applySortSettings } from "@/utils/directoryOptions"
import {
  ArrowUpDown,
  HardDriveIcon,
  ImagesIcon,
  Languages,
  NavigationIcon,
  PaletteIcon,
  PresentationIcon,
  Shuffle,
  TimerIcon
} from "lucide-react"
import type { ReactNode } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

export function GeneralSettingsPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      language: state.language,
      loopNavigation: state.loopNavigation,
      cacheMode: state.cacheMode,
      viewMode: state.viewMode,
      slideshowIntervalMs: state.slideshowIntervalMs,
      autoOpenLastFile: state.autoOpenLastFile,
      viewerBackground: state.viewerBackground,
      autoHideUI: state.autoHideUI,
      sortKey: state.sortKey,
      sortDescending: state.sortDescending,
      shuffle: state.shuffle,
      includeSubfolders: state.includeSubfolders
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  const handleSortChange = (
    next: Partial<
      Pick<
        SettingsState,
        "sortKey" | "sortDescending" | "shuffle" | "includeSubfolders"
      >
    >
  ) => {
    void applySortSettings(next)
  }

  return (
    <FieldGroup>
      <CustomFieldSet
        icon={<Languages />}
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
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<NavigationIcon />}
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
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<ImagesIcon />}
        title={t("settings.view.title")}
        description={t("settings.view.desc")}
      >
        <RadioGroup
          value={settings.viewMode}
          onValueChange={(value) =>
            handleSettingsChange({ viewMode: value as ViewMode })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="single" id="settings-view-single" />
            <Label htmlFor="settings-view-single">
              {t("settings.view.single")}
            </Label>
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
            <Label htmlFor="settings-view-webtoon">
              {t("settings.view.webtoon")}
            </Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<ArrowUpDown />}
        title={t("settings.sort.title")}
        description={t("settings.sort.desc")}
      >
        <RadioGroup
          value={settings.shuffle ? "shuffle" : settings.sortKey}
          onValueChange={(value) => {
            if (value === "shuffle") {
              handleSortChange({ shuffle: true })
            } else {
              handleSortChange({
                shuffle: false,
                sortKey: value as DirSortKey
              })
            }
          }}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="name" id="settings-sort-name" />
            <Label htmlFor="settings-sort-name">
              {t("settings.sort.name")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="date" id="settings-sort-date" />
            <Label htmlFor="settings-sort-date">
              {t("settings.sort.date")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="size" id="settings-sort-size" />
            <Label htmlFor="settings-sort-size">
              {t("settings.sort.size")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="shuffle" id="settings-sort-shuffle" />
            <Label htmlFor="settings-sort-shuffle">
              <span className="flex items-center gap-1">
                <Shuffle className="size-3.5" /> {t("settings.sort.shuffle")}
              </span>
            </Label>
          </Field>
        </RadioGroup>
        <RadioGroup
          value={settings.sortDescending ? "desc" : "asc"}
          onValueChange={(value) =>
            handleSortChange({ sortDescending: value === "desc" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="asc" id="settings-sort-asc" />
            <Label htmlFor="settings-sort-asc">{t("settings.sort.asc")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="desc" id="settings-sort-desc" />
            <Label htmlFor="settings-sort-desc">
              {t("settings.sort.descOrder")}
            </Label>
          </Field>
        </RadioGroup>
        <Field orientation="horizontal">
          <Switch
            id="settings-subfolders"
            checked={settings.includeSubfolders}
            onCheckedChange={(checked) =>
              handleSortChange({ includeSubfolders: checked === true })
            }
          />
          <Label htmlFor="settings-subfolders">
            {t("settings.sort.subfolders")}
          </Label>
        </Field>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<TimerIcon />}
        title={t("settings.slideshow.title")}
        description={t("settings.slideshow.desc", {
          sec: (settings.slideshowIntervalMs / 1000).toFixed(1)
        })}
      >
        <Slider
          aria-label={t("settings.slideshow.aria")}
          value={[settings.slideshowIntervalMs]}
          min={1000}
          max={30000}
          step={500}
          onValueChange={(value) => {
            const next = Array.isArray(value) ? value[0] : (value as number)
            if (typeof next === "number")
              handleSettingsChange({ slideshowIntervalMs: next })
          }}
        />
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<PaletteIcon />}
        title={t("settings.bg.title")}
        description={t("settings.bg.desc")}
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
            <Label htmlFor="settings-bg-checker">
              {t("settings.bg.checker")}
            </Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<PresentationIcon />}
        title={t("settings.present.title")}
        description={t("settings.present.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-autohide"
            checked={settings.autoHideUI}
            onCheckedChange={(checked) =>
              handleSettingsChange({ autoHideUI: checked === true })
            }
          />
          <Label htmlFor="settings-autohide">
            {t("settings.present.autohide")}
          </Label>
        </Field>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<HardDriveIcon />}
        title={t("settings.cache.title")}
        description={t("settings.cache.desc")}
      >
        <RadioGroup
          value={settings.cacheMode}
          onValueChange={(value) =>
            handleSettingsChange({ cacheMode: value as CacheMode })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-cache-off" />
            <Label htmlFor="settings-cache-off">
              {t("settings.cache.off")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="nearby" id="settings-cache-nearby" />
            <Label htmlFor="settings-cache-nearby">
              {t("settings.cache.nearby")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="extended" id="settings-cache-extended" />
            <Label htmlFor="settings-cache-extended">
              {t("settings.cache.extended")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-1gb" id="settings-cache-1gb" />
            <Label htmlFor="settings-cache-1gb">
              {t("settings.cache.gb1")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-2gb" id="settings-cache-2gb" />
            <Label htmlFor="settings-cache-2gb">
              {t("settings.cache.gb2")}
            </Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>
      <FieldSeparator />

      <CustomFieldSet
        icon={<NavigationIcon />}
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
      </CustomFieldSet>
    </FieldGroup>
  )
}

function CustomFieldSet(props: {
  icon: ReactNode
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <FieldSet>
      <FieldLegend className="flex items-center gap-2">
        {props.icon}
        {props.title}
      </FieldLegend>
      <FieldDescription>{props.description}</FieldDescription>
      {props.children}
    </FieldSet>
  )
}
